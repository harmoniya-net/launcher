//! Best-effort GitHub-Releases self-update. The entry points below are
//! blocking and must be dispatched off the UI thread (see `state::update`).
//!
//! We never call GitHub's REST API: unauthenticated, it allows 60 requests an
//! hour per IP, and players behind a shared IP (carrier NAT, some ISPs) get a
//! 403 on every check, so they never update. Instead we read the newest tag
//! from the `releases/latest` web redirect and download the asset from its
//! plain `releases/download` URL; neither counts against that limit. CI
//! publishes one archive per target triple (see .github/workflows/release.yml).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

const REPO_URL: &str = "https://github.com/harmoniya-net/launcher";

/// The launcher's own path, captured at startup *before* the updater swaps the
/// binary. On Linux, once [`install`] replaces the executable in place,
/// `std::env::current_exe()` resolves `/proc/self/exe` to the now-unlinked old
/// inode and hands back a bogus `".../harmoniya-launcher (deleted)"` path —
/// exec'ing that fails, so the app never comes back up after an update. The
/// original path still holds the freshly-installed binary, so that's what we
/// re-exec. See [`record_exe_path`] / [`relaunch`].
static EXE_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Remember the running executable's path. Call once at startup, before any
/// update could replace the binary.
pub fn record_exe_path() {
    match std::env::current_exe() {
        Ok(path) => {
            let _ = EXE_PATH.set(path);
        }
        Err(e) => tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "relaunch: could not record exe path at startup"),
    }
}

/// Where the executable sits inside the release archive.
fn bin_path_in_archive() -> &'static str {
    if cfg!(target_os = "macos") {
        // The macOS archive holds the whole .app bundle.
        "Harmoniya.app/Contents/MacOS/harmoniya-launcher"
    } else if cfg!(windows) {
        "harmoniya-launcher.exe"
    } else {
        "harmoniya-launcher"
    }
}

/// The newest release's version (without the leading `v`), read from where
/// `releases/latest` redirects to: `.../releases/tag/v0.7.3`.
fn latest_version() -> Result<String> {
    harmoniya_api::http::handle().block_on(async {
        let client = reqwest::Client::builder()
            .user_agent(concat!("harmoniya-launcher/", env!("CARGO_PKG_VERSION")))
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .tls_built_in_webpki_certs(true)
            .tls_built_in_native_certs(true)
            .build()?;
        let resp = client
            .head(format!("{REPO_URL}/releases/latest"))
            .send()
            .await
            .map_err(|e| anyhow!("latest release: {}", harmoniya_api::obs::cause_chain(&e)))?;
        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("latest release: HTTP {} without a redirect", resp.status()))?;
        let tag = location
            .rsplit_once("/releases/tag/")
            .map(|(_, tag)| tag)
            .ok_or_else(|| anyhow!("latest release: unexpected redirect to {location}"))?;
        Ok(tag.trim_start_matches('v').to_string())
    })
}

/// Check GitHub Releases for a newer build. `Some(version)` if an update is
/// available, `None` if we're already current. Blocking (network).
pub fn check() -> Result<Option<String>> {
    let latest = latest_version()?;
    let newer = self_update::version::bump_is_greater(env!("CARGO_PKG_VERSION"), &latest)?;
    Ok(newer.then_some(latest))
}

/// Download release `version` (from [`check`]) and replace the running binary.
/// Blocking (network + disk).
pub fn install(version: &str) -> Result<()> {
    let asset = format!("harmoniya-launcher-{}.tar.gz", self_update::get_target());
    let url = format!("{REPO_URL}/releases/download/v{version}/{asset}");

    let dir = self_update::TempDir::new().context("temp dir for the update")?;
    let archive = dir.path().join(&asset);
    let file = std::fs::File::create(&archive).context("create the update archive")?;
    self_update::Download::from_url(&url)
        .download_to(file)
        .with_context(|| format!("download {url}"))?;

    self_update::Extract::from_source(&archive)
        .extract_file(dir.path(), bin_path_in_archive())
        .context("unpack the update")?;
    self_update::self_replace::self_replace(dir.path().join(bin_path_in_archive()))
        .context("replace the launcher executable")?;
    Ok(())
}

/// Re-exec the (freshly updated) binary and exit this process. Never returns.
///
/// Releasing the single-instance lock *before* spawning hands it off cleanly:
/// the child finds it free, so there's no race with our own (still-alive) process.
///
/// We re-exec the path recorded at startup, not `current_exe()`, which is stale
/// after an in-place update on Linux (see [`EXE_PATH`]).
pub fn relaunch() -> ! {
    super::single_instance::release();
    let exe = EXE_PATH.get().cloned().or_else(|| std::env::current_exe().ok());
    match exe {
        Some(exe) => {
            let spawned =
                std::process::Command::new(&exe).args(std::env::args_os().skip(1)).spawn();
            if let Err(e) = spawned {
                tracing::error!(exe = %exe.display(), error = %harmoniya_api::obs::cause_chain(&e), "relaunch: spawn failed");
            }
        }
        None => tracing::error!("relaunch: no executable path to spawn"),
    }
    std::process::exit(0);
}
