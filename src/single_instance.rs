//! Single-instance guard backed by the `single-instance` crate. The first
//! launch holds an OS primitive — a named mutex on Windows, an abstract UNIX
//! domain socket on Linux, an `flock`'d file on macOS — and a second launch
//! sees it isn't unique and bows out. The OS releases the primitive when the
//! holding process dies, so there's no stale-lock problem (including after a
//! `process::exit`, which is how we quit).
//!
//! Unlike the previous hand-rolled loopback-TCP scheme, this does *not* focus
//! the already-running window on a second launch — the crate only does mutual
//! exclusion, no IPC. A duplicate launch simply exits; use the tray icon to
//! restore a window that's hidden to tray.

use single_instance::SingleInstance;

/// Identifier for the lock. On macOS the crate treats the name as a filesystem
/// path to `flock`, so give it an absolute path under the temp dir there; on
/// Windows/Linux it's a mutex / abstract-socket name, where reverse-DNS is fine.
fn lock_name() -> String {
    #[cfg(target_os = "macos")]
    let name = std::env::temp_dir()
        .join("net.harmoniya.launcher.lock")
        .to_string_lossy()
        .into_owned();
    #[cfg(not(target_os = "macos"))]
    let name = "net.harmoniya.launcher".to_string();
    name
}

pub enum Instance {
    /// We're the only instance. The caller must keep the guard alive for the
    /// process's lifetime to hold the lock; `None` means the guard couldn't be
    /// created for some unrelated reason — run anyway, just unguarded.
    Primary(Option<SingleInstance>),
    /// Another instance is already running.
    AlreadyRunning,
}

/// Try to become the primary instance.
pub fn acquire() -> Instance {
    match SingleInstance::new(&lock_name()) {
        Ok(instance) if instance.is_single() => Instance::Primary(Some(instance)),
        Ok(_) => Instance::AlreadyRunning,
        Err(e) => {
            tracing::warn!("single-instance guard unavailable: {e}");
            Instance::Primary(None)
        }
    }
}
