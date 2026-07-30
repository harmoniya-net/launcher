//! Best-effort GitHub-Releases self-update. `self_update` is sync, so the three
//! entry points below are blocking and must be dispatched off the UI thread (see
//! `state::update`). The GitHub repo is public, so release assets download
//! without auth; CI publishes one asset per target triple, which `self_update`
//! matches against the running binary's target.

use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::Result;
use self_update::update::ReleaseUpdate;

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

/// Build the self-updater for this binary.
fn updater() -> Result<Box<dyn ReleaseUpdate>> {
    let mut builder = self_update::backends::github::Update::configure();
    builder
        .repo_owner("harmoniya-net")
        .repo_name("launcher")
        .bin_name("harmoniya-launcher")
        .current_version(env!("CARGO_PKG_VERSION"))
        .show_download_progress(false)
        .no_confirm(true);
    // On macOS the release archive holds a whole .app bundle (see
    // .github/workflows/release.yml), not a bare binary at the archive root —
    // point the extractor at the executable's exact path inside it.
    #[cfg(target_os = "macos")]
    builder.bin_path_in_archive("Harmoniya.app/Contents/MacOS/{{ bin }}");
    Ok(builder.build()?)
}

/// Check GitHub Releases for a newer build. `Some(version)` if an update is
/// available, `None` if we're already current. Blocking (network).
pub fn check() -> Result<Option<String>> {
    let latest = updater()?.get_latest_release()?;
    let newer = self_update::version::bump_is_greater(env!("CARGO_PKG_VERSION"), &latest.version)?;
    Ok(newer.then_some(latest.version))
}

/// Download the latest release and replace the running binary, returning whether
/// anything was installed. Blocking (network + disk); call after [`check`].
pub fn install() -> Result<bool> {
    Ok(updater()?.update()?.updated())
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
