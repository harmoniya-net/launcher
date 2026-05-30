//! Game launch pipeline — install + spawn a modpack from its CMS manifest URL.
//!
//! Mirrors the web launcher (`server/api/launch/[id].get.ts` + `useLauncher.ts`):
//! resolve auth (username / uuid / yggdrasil token), pick the data dir, run the
//! `opys_runtime` install pipeline streaming progress, then spawn the game with
//! `install: false`. Progress and the terminal result are pushed back to the UI
//! over an unbounded channel so the launch modal can render exactly like the web
//! `LaunchModal.vue` (phase label, percent bar, per-file rows, error block).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use futures::channel::mpsc::UnboundedSender;
use indexmap::IndexMap;
use opys_runtime::{InstallError, InstallOptions, InstallProgress, LaunchOptions, ManifestSource};

use harmoniya_api::auth::{self, tokens::Tokens};
use harmoniya_api::services::account::fetch_me;

/// Throttle interval for per-file byte updates, matching the web `BYTE_THROTTLE_MS`.
const BYTE_THROTTLE_MS: u64 = 250;

/// Install/launch phases and the percent window each occupies on the overall
/// bar. Mirrors `PHASE_RANGES` / `PHASE_LABELS` in the web handler.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Resolve,
    Download,
    Verify,
    Extract,
}

impl Phase {
    fn range(self) -> (f32, f32) {
        match self {
            Phase::Resolve => (0., 5.),
            Phase::Download => (5., 80.),
            Phase::Verify => (80., 90.),
            Phase::Extract => (90., 100.),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Resolve => "Завантаження маніфесту…",
            Phase::Download => "Завантаження файлів…",
            Phase::Verify => "Перевірка цілісності…",
            Phase::Extract => "Розпакування…",
        }
    }

    /// Short label used in the error footer (`PHASE_LABELS` in `LaunchModal.vue`).
    pub fn short(self) -> &'static str {
        match self {
            Phase::Resolve => "отримання маніфесту",
            Phase::Download => "завантаження",
            Phase::Verify => "перевірка цілісності",
            Phase::Extract => "розпакування",
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ErrorCode {
    Network,
    Integrity,
    Extract,
    Unknown,
}

impl ErrorCode {
    pub fn label(self) -> &'static str {
        match self {
            ErrorCode::Network => "Помилка мережі",
            ErrorCode::Integrity => "Файл пошкоджено",
            ErrorCode::Extract => "Помилка розпакування",
            ErrorCode::Unknown => "Невідома помилка",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchError {
    pub code: ErrorCode,
    pub message: String,
    pub phase: Option<Phase>,
    pub paths: Vec<String>,
}

/// UI-facing launch state, 1:1 with the web `LaunchState` union.
#[derive(Clone, Debug)]
pub enum LaunchState {
    Idle,
    Starting,
    Progress(LaunchProgress),
    Error(LaunchError),
    Done { pid: u32 },
}

impl Default for LaunchState {
    fn default() -> Self {
        LaunchState::Idle
    }
}

/// Messages streamed from the worker (running on the tokio runtime) back to the
/// GPUI entity.
pub enum LaunchMsg {
    /// Tokens were refreshed mid-flow; persist + adopt them.
    TokensRefreshed(Tokens),
    Progress(LaunchProgress),
    Error(LaunchError),
    Done { pid: u32 },
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
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

/// Accumulates raw `InstallProgress` callbacks into a `LaunchProgress` snapshot,
/// porting the bookkeeping from the web SSE handler.
#[derive(Default)]
struct Tracker {
    files: HashMap<String, FileState>,
    download_fetched: u32,
    download_total: u32,
    phase: Option<Phase>,
}

struct FileState {
    bytes: u64,
    total: Option<u64>,
    last_emit: u64,
}

impl Tracker {
    fn percent(&self) -> u8 {
        let Some(phase) = self.phase else { return 0 };
        let (lo, hi) = phase.range();
        let pct = if phase != Phase::Download {
            lo
        } else if self.download_total == 0 {
            hi
        } else {
            lo + (hi - lo) * (self.download_fetched as f32 / self.download_total as f32)
        };
        pct.round().clamp(0., 100.) as u8
    }

    fn snapshot(&self) -> LaunchProgress {
        let mut files: Vec<LaunchFile> = self
            .files
            .iter()
            .map(|(path, s)| LaunchFile { path: path.clone(), bytes: s.bytes, total: s.total })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        LaunchProgress { phase: self.phase.unwrap_or(Phase::Resolve), percent: self.percent(), files }
    }

    /// Apply one raw progress event. Returns `Some(snapshot)` when the UI should
    /// re-render (mirrors which events the web handler emits on).
    fn apply(&mut self, p: InstallProgress) -> Option<LaunchProgress> {
        match p {
            InstallProgress::Resolve | InstallProgress::Pointer { .. } => {
                self.phase = Some(Phase::Resolve);
                Some(self.snapshot())
            }
            InstallProgress::Download { fetched, total, .. } => {
                self.phase = Some(Phase::Download);
                self.download_fetched = fetched;
                self.download_total = total;
                Some(self.snapshot())
            }
            InstallProgress::DownloadStart { path, total } => {
                self.phase = Some(Phase::Download);
                self.files.insert(
                    path,
                    FileState { bytes: 0, total: (total > 0).then_some(total), last_emit: 0 },
                );
                Some(self.snapshot())
            }
            InstallProgress::DownloadBytes { path, bytes } => {
                let now = now_ms();
                let s = self.files.get_mut(&path)?;
                s.bytes = bytes;
                if now.saturating_sub(s.last_emit) < BYTE_THROTTLE_MS {
                    return None;
                }
                s.last_emit = now;
                Some(self.snapshot())
            }
            InstallProgress::DownloadDone { path } => {
                self.files.remove(&path);
                None
            }
            InstallProgress::Verify => {
                self.phase = Some(Phase::Verify);
                self.files.clear();
                Some(self.snapshot())
            }
            InstallProgress::Extract { .. } => {
                self.phase = Some(Phase::Extract);
                Some(self.snapshot())
            }
            InstallProgress::Sweep { .. } => None,
        }
    }
}

fn classify(err: &InstallError, phase: Option<Phase>) -> LaunchError {
    let (code, paths) = match err {
        InstallError::Network { .. } => (ErrorCode::Network, Vec::new()),
        InstallError::Integrity { paths } => (ErrorCode::Integrity, paths.clone()),
        InstallError::Extraction { .. } => (ErrorCode::Extract, Vec::new()),
        _ => (ErrorCode::Unknown, Vec::new()),
    };
    LaunchError { code, message: err.to_string(), phase, paths }
}

/// Build the interpolation vars the manifest expects (`username`, `uuid`,
/// `token`, `root`). Same keys the web handler injects.
fn launch_vars(username: &str, uuid: &str, token: &str, root: &str) -> IndexMap<String, String> {
    let mut vars = IndexMap::new();
    vars.insert("username".to_string(), username.to_string());
    vars.insert("uuid".to_string(), uuid.to_string());
    vars.insert("token".to_string(), token.to_string());
    vars.insert("root".to_string(), root.to_string());
    vars
}

/// Drive the full launch: resolve auth, install, spawn. Runs on the tokio
/// runtime (call via `harmoniya_api::http::on_tokio`). Reports everything over `tx`.
pub async fn run(
    tokens: Tokens,
    modpack_id: String,
    manifest_url: String,
    data_dir: String,
    extra_vars: IndexMap<String, String>,
    features: Vec<String>,
    tx: UnboundedSender<LaunchMsg>,
) {
    // The progress callback fires on the tokio runtime and must be `Fn`, so the
    // tracker lives behind a mutex and pushes snapshots into the same channel.
    let tracker = Arc::new(Mutex::new(Tracker::default()));
    let phase_of = {
        let tracker = Arc::clone(&tracker);
        move || tracker.lock().unwrap().phase
    };

    let result =
        run_inner(tokens, modpack_id, &manifest_url, &data_dir, extra_vars, features, &tx, Arc::clone(&tracker))
            .await;
    let msg = match result {
        Ok(pid) => LaunchMsg::Done { pid },
        Err(e) => LaunchMsg::Error(classify(&e, phase_of())),
    };
    let _ = tx.unbounded_send(msg);
}

async fn run_inner(
    mut tokens: Tokens,
    modpack_id: String,
    manifest_url: &str,
    data_dir: &str,
    extra_vars: IndexMap<String, String>,
    features: Vec<String>,
    tx: &UnboundedSender<LaunchMsg>,
    tracker: Arc<Mutex<Tracker>>,
) -> Result<u32, InstallError> {
    // Refresh the access token up front if it's expired, then adopt + persist it.
    if tokens.is_access_expired() {
        if let Some(refresh) = tokens.refresh_token.clone() {
            let refreshed = auth::coordinated_refresh(&refresh)
                .await
                .map_err(|e| InstallError::other(format!("token refresh: {e}")))?;
            tokens = refreshed.clone();
            let _ = tx.unbounded_send(LaunchMsg::TokensRefreshed(refreshed));
        }
    }
    let access = tokens.access_token.clone();

    let user = fetch_me(&access).await.map_err(|e| InstallError::other(e.to_string()))?;
    let ygg_token = auth::get_yggdrasil_token(&access)
        .await
        .map_err(|e| InstallError::other(e.to_string()))?;
    let uuid = uuid_from_ygg(&ygg_token).map_err(|e| InstallError::other(e.to_string()))?;

    tokio::fs::create_dir_all(data_dir).await.map_err(|source| InstallError::Io {
        path: data_dir.to_owned(),
        source,
    })?;

    // Start from the user's modpack options, then layer the base vars on top so
    // the critical ones (username/uuid/token/root) can never be overridden.
    let mut vars = extra_vars;
    for (k, v) in launch_vars(&user.username, &uuid, &ygg_token, data_dir) {
        vars.insert(k, v);
    }

    // Install: stream progress through the tracker into the channel.
    let on_progress: Arc<dyn Fn(InstallProgress) + Send + Sync> = {
        let tracker = Arc::clone(&tracker);
        let tx = tx.clone();
        Arc::new(move |p| {
            if let Some(snapshot) = tracker.lock().unwrap().apply(p) {
                let _ = tx.unbounded_send(LaunchMsg::Progress(snapshot));
            }
        })
    };
    let mut install_opts = InstallOptions::new();
    install_opts.vars = Some(vars.clone());
    install_opts.features = features.clone();
    install_opts.on_progress = Some(on_progress);
    opys_runtime::install(ManifestSource::Url(manifest_url), install_opts).await?;

    // Build the spec and spawn the game without re-installing.
    let mut launch_opts = LaunchOptions::new();
    launch_opts.vars = Some(vars);
    launch_opts.features = features;
    launch_opts.do_install = false;
    let child = opys_runtime::launch(ManifestSource::Url(manifest_url), launch_opts).await?;

    let pid = child.id().unwrap_or(0);
    // Track the process by modpack so the UI can stop it, it's stopped on quit,
    // and a background watcher clears it when the game exits on its own.
    crate::game::register(modpack_id, child);
    Ok(pid)
}
