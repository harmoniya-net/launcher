//! Game launch pipeline — install + spawn a modpack from its published opys
//! bundle.
//!
//! Resolve auth (username / uuid / yggdrasil token), read the bundle's head and
//! resolve the player's saved options against it, run the `opys_runtime`
//! install streaming progress, then spawn what the manifest says to run.
//! Progress and the terminal result are pushed back to the UI over an
//! unbounded channel so the launch modal can render them (phase label, percent
//! bar, per-file rows, error block).

use std::sync::{Arc, Mutex};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::channel::mpsc::UnboundedSender;
use indexmap::IndexMap;
pub use opys_runtime::{LaunchSpec, ManifestSource};
use opys_runtime::{ErrorReport, InstallError, InstallOptions, InstallProgress, LaunchOptions, ROOT_VAR};
/// Re-exported so the app can mint a token and cancel an in-flight install.
pub use opys_runtime::CancellationToken;

use harmoniya_api::auth::{self, SessionStore};
use harmoniya_api::services::account::fetch_me;

use crate::options::{self, ModpackOptions};

mod progress;
use progress::Tracker;

/// Install phases, as the runtime runs them, and the percent window each
/// occupies on the overall bar. The runtime also scans what is already on
/// disk between resolving and fetching; it reports nothing of its own there,
/// so that time shows as the end of `Resolve`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Reading the bundle's head, then downloading and opening the bundle.
    Resolve,
    /// Downloading the files the manifest lists that aren't on disk yet.
    Fetch,
    Verify,
    Extract,
    /// Removing what the manifest's `cleanup` rules name (e.g. stray mods).
    Cleanup,
}

impl Phase {
    fn range(self) -> (f32, f32) {
        match self {
            Phase::Resolve => (0., 5.),
            Phase::Fetch => (5., 85.),
            Phase::Verify => (85., 93.),
            Phase::Extract => (93., 99.),
            Phase::Cleanup => (99., 100.),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchFile {
    pub path: String,
    pub bytes: u64,
    pub total: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct LaunchProgress {
    pub phase: Phase,
    pub percent: u8,
    pub files: Vec<LaunchFile>,
}

/// Coarse error classification. Human-readable titles live in the app's
/// localization catalog (`i18n`), keeping this crate language-neutral.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErrorCode {
    /// A download was refused or failed.
    Network,
    /// Files on disk aren't the ones the manifest pins.
    Integrity,
    /// An archive could not be unpacked.
    Extract,
    /// The bundle (or its manifest) is one this launcher can't read.
    Manifest,
    /// The file system refused something (permissions, disk full, …).
    Io,
    /// The player closed the launch modal mid-install.
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct LaunchError {
    pub code: ErrorCode,
    pub message: String,
    pub phase: Option<Phase>,
    pub paths: Vec<String>,
}

/// UI-facing launch state, 1:1 with the web `LaunchState` union.
#[derive(Clone, Debug, Default)]
pub enum LaunchState {
    #[default]
    Idle,
    Starting,
    Progress(LaunchProgress),
    Error(LaunchError),
    Done { pid: u32 },
}

/// Messages streamed from the worker (running on the tokio runtime) back to the
/// GPUI entity.
pub enum LaunchMsg {
    Progress(LaunchProgress),
    Error(LaunchError),
    Done { pid: u32 },
}

/// Default game data directory, matching the web `defaultDataDir()`.
pub fn default_data_dir() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return format!("{appdata}\\Harmoniya");
        }
    }
    if let Some(dirs) = directories::BaseDirs::new() {
        #[cfg(target_os = "macos")]
        {
            return dirs
                .home_dir()
                .join("Library")
                .join("Application Support")
                .join("Harmoniya")
                .to_string_lossy()
                .into_owned();
        }
        #[cfg(target_os = "windows")]
        {
            return dirs.data_dir().join("Harmoniya").to_string_lossy().into_owned();
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            return dirs
                .home_dir()
                .join(".local")
                .join("share")
                .join("harmoniya")
                .to_string_lossy()
                .into_owned();
        }
    }
    "harmoniya".into()
}

/// `override` if non-empty, else the platform default. Mirrors `resolveDataDir`.
pub fn resolve_data_dir(override_dir: Option<&str>) -> String {
    match override_dir.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s.to_owned(),
        None => default_data_dir(),
    }
}

/// Extract the (dash-stripped) Minecraft UUID from a yggdrasil JWT payload.
fn uuid_from_ygg(ygg_token: &str) -> anyhow::Result<String> {
    let payload_b64 = ygg_token
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("malformed yggdrasil token"))?;
    let payload = URL_SAFE_NO_PAD.decode(payload_b64.trim_end_matches('='))?;
    let claims: serde_json::Value = serde_json::from_slice(&payload)?;
    let uuid = claims
        .get("uuid")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("no uuid in yggdrasil token"))?;
    Ok(uuid.replace('-', ""))
}

/// Map the runtime's error kind ([`ErrorReport`], its `code` as data) onto the
/// launcher's. Nothing here reads the wording of a message.
fn classify(err: &InstallError, phase: Option<Phase>) -> LaunchError {
    let (code, paths) = match err.report() {
        ErrorReport::Network { .. } => (ErrorCode::Network, Vec::new()),
        ErrorReport::Integrity { paths, .. } => (ErrorCode::Integrity, paths),
        ErrorReport::Extraction { artifact_path, .. } => (ErrorCode::Extract, vec![artifact_path]),
        ErrorReport::Manifest { .. } => (ErrorCode::Manifest, Vec::new()),
        ErrorReport::Io { path, .. } => {
            (ErrorCode::Io, (!path.is_empty()).then_some(path).into_iter().collect())
        }
        ErrorReport::Cancelled { .. } => (ErrorCode::Cancelled, Vec::new()),
        ErrorReport::Other { .. } => (ErrorCode::Unknown, Vec::new()),
    };
    LaunchError { code, message: err.to_string(), phase, paths }
}

/// Who is playing: what the launcher always tells a manifest, as the vars
/// `username`, `uuid` and `token` (yggdrasil).
#[derive(Clone, Debug)]
pub struct Player {
    pub username: String,
    pub uuid: String,
    pub token: String,
}

/// The vars every launch is given (`username`, `uuid`, `token`, `root`), with
/// the player's option values layered on top — they may override any of them,
/// e.g. a text option that fills `username` behind a feature. `root` is the one
/// var the runtime reads by name, and the caller's always wins over the
/// manifest's.
fn launch_vars(player: &Player, root: &str, options: IndexMap<String, String>) -> IndexMap<String, String> {
    let mut vars = IndexMap::new();
    vars.insert("username".to_string(), player.username.clone());
    vars.insert("uuid".to_string(), player.uuid.clone());
    vars.insert("token".to_string(), player.token.clone());
    vars.insert(ROOT_VAR.to_string(), root.to_string());
    vars.extend(options);
    vars
}

/// Drive the full launch: resolve auth, install, spawn. Runs on the tokio
/// runtime (call via `harmoniya_api::http::on_tokio`). Reports everything over
/// `tx`. Cancelling `cancel` aborts an in-flight install (downloads included).
pub async fn run(
    store: Arc<SessionStore>,
    modpack_id: String,
    bundle_url: String,
    data_dir: String,
    saved: ModpackOptions,
    cancel: CancellationToken,
    tx: UnboundedSender<LaunchMsg>,
) {
    // The progress callback fires on the tokio runtime and must be `Fn`, so the
    // tracker lives behind a mutex and pushes snapshots into the same channel.
    let tracker = Arc::new(Mutex::new(Tracker::default()));
    let on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync> = {
        let tracker = Arc::clone(&tracker);
        let tx = tx.clone();
        Arc::new(move |p| {
            if let Some(snapshot) = tracker.lock().unwrap().apply(p) {
                let _ = tx.unbounded_send(LaunchMsg::Progress(snapshot));
            }
        })
    };
    let mut install = InstallOptions::new();
    install.on_progress = Some(on_progress);
    install.cancel = cancel;

    let result = run_inner(store, modpack_id, bundle_url, &data_dir, &saved, install).await;
    let msg = match result {
        Ok(pid) => LaunchMsg::Done { pid },
        Err(e) => LaunchMsg::Error(classify(&e, tracker.lock().unwrap().phase)),
    };
    let _ = tx.unbounded_send(msg);
}

async fn run_inner(
    store: Arc<SessionStore>,
    modpack_id: String,
    bundle_url: String,
    data_dir: &str,
    saved: &ModpackOptions,
    install: InstallOptions,
) -> Result<u32, InstallError> {
    if let Some(cb) = &install.on_progress {
        cb(InstallProgress::Resolve);
    }
    let token = store.token().map_err(|e| InstallError::other(format!("{e:#}")))?;

    let user = fetch_me(&token).await.map_err(|e| InstallError::other(format!("{e:#}")))?;
    let ygg_token = auth::fetch_yggdrasil_token(&token)
        .await
        .map_err(|e| InstallError::other(format!("{e:#}")))?;
    let uuid = uuid_from_ygg(&ygg_token).map_err(|e| InstallError::other(format!("{e:#}")))?;
    let player = Player { username: user.username, uuid, token: ygg_token };

    // `root` is the shared data dir: every pack's manifest keeps its game
    // directory in a subdirectory of its own and shares `libraries`/`assets`.
    let spec = prepare(ManifestSource::Url(bundle_url), data_dir, &player, saved, install).await?;

    let mut command = tokio::process::Command::new(&spec.command);
    command.args(&spec.args).current_dir(&spec.workdir).envs(&spec.envs);
    let child = command.spawn().map_err(|source| InstallError::Io {
        path: spec.command.clone(),
        source,
    })?;

    let pid = child.id().unwrap_or(0);
    // Track the process by modpack so the UI can stop it, it's stopped on quit,
    // and a background watcher clears it when the game exits on its own.
    crate::game::register(modpack_id, child);
    Ok(pid)
}

/// Install the bundle `source` names into `root` and say what to run: its
/// head's options resolved against the player's `saved` choices, layered over
/// the player's vars. Everything of a launch but auth and the spawn.
pub async fn prepare(
    source: ManifestSource,
    root: &str,
    player: &Player,
    saved: &ModpackOptions,
    install: InstallOptions,
) -> Result<LaunchSpec, InstallError> {
    // The options are the bundle's own, read off its head: whatever the UI
    // last showed, this is the schema the bundle being installed was built with.
    let head = options::head_of(&source).await?;
    let (option_vars, features) = options::resolve(&head.options, saved);

    tokio::fs::create_dir_all(root).await.map_err(|source| InstallError::Io {
        path: root.to_owned(),
        source,
    })?;
    let mut launch = LaunchOptions::new();
    launch.vars = Some(launch_vars(player, root, option_vars));
    launch.features = features;
    launch.install = Some(install);
    // One resolve for both: the bundle is downloaded and opened once, installed
    // from, and asked what to run.
    opys_runtime::prepare(source, launch).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_layer_over_the_base_vars_except_where_they_dont_say() {
        let mut options = IndexMap::new();
        options.insert("username".to_string(), "Custom".to_string());
        options.insert("xmx".to_string(), "4096".to_string());
        let player = Player { username: "Steve".into(), uuid: "u".into(), token: "t".into() };
        let vars = launch_vars(&player, "/r", options);
        assert_eq!(vars["username"], "Custom");
        assert_eq!(vars["uuid"], "u");
        assert_eq!(vars[ROOT_VAR], "/r");
        assert_eq!(vars["xmx"], "4096");
    }

    #[test]
    fn errors_are_told_apart_by_kind_not_wording() {
        let e = classify(&InstallError::Manifest("bad".into()), Some(Phase::Resolve));
        assert_eq!(e.code, ErrorCode::Manifest);
        let e = classify(&InstallError::Cancelled, Some(Phase::Fetch));
        assert_eq!(e.code, ErrorCode::Cancelled);
        let e = classify(&InstallError::Integrity { paths: vec!["a".into()] }, None);
        assert_eq!((e.code, e.paths), (ErrorCode::Integrity, vec!["a".to_string()]));
    }
}
