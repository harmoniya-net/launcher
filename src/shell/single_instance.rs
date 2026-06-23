//! Single-instance guard backed by the `single-instance` crate. The first
//! launch holds an OS primitive — a named mutex on Windows, an abstract UNIX
//! domain socket on Linux, an `flock`'d file on macOS — and a second launch
//! sees it isn't unique and bows out. The OS releases the primitive when the
//! holding process dies, so there's no stale-lock problem (including after a
//! `process::exit`, which is how we quit).
//!
//! The crate only does mutual exclusion (no IPC), so "raise the already-running
//! window on a second launch" is layered on top via a tiny loopback poke: the
//! primary listens on a fixed loopback port ([`serve_focus`]); a duplicate
//! launch connects to it ([`signal_focus`]) to ask the primary to bring its
//! window forward, then exits.

use std::cell::RefCell;
use std::net::{TcpListener, TcpStream};

use futures::channel::mpsc::{unbounded, UnboundedReceiver};
use single_instance::SingleInstance;

/// Loopback port the primary listens on for "focus me" pokes from duplicate
/// launches. A one-off random pick in the dynamic/private range (rolled with
/// `shuf -i 49152-65535 -n 1`), baked in so the primary and a duplicate launch
/// agree on it. Loopback-only (127.0.0.1), so no firewall prompt; a clash with
/// unrelated software just disables activation (the OS lock still enforces
/// single-instance and the app still runs).
const FOCUS_PORT: u16 = 56729;

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
    /// We're the only instance. Pass the guard to [`hold`] to keep the lock for
    /// the process's lifetime; `None` means the guard couldn't be created for
    /// some unrelated reason — run anyway, just unguarded.
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
            tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "single-instance guard unavailable");
            Instance::Primary(None)
        }
    }
}

thread_local! {
    /// Holds the live guard for the process's lifetime — a thread-local rather
    /// than a `main` local so the auto-updater can release it at the exact moment
    /// it re-execs (see [`hold`]/[`release`]). Only ever touched on the main
    /// thread, where both the app loop and the update task run.
    static GUARD: RefCell<Option<SingleInstance>> = const { RefCell::new(None) };
}

/// Keep the primary's guard alive for the rest of the process.
pub fn hold(instance: Option<SingleInstance>) {
    GUARD.with(|g| *g.borrow_mut() = instance);
}

/// Drop the guard now, releasing the OS lock synchronously, so a child we're
/// about to spawn can claim it cleanly. Used by the updater right before re-exec
/// — this turns the relaunch into a deterministic hand-off instead of a race.
pub fn release() {
    GUARD.with(|g| drop(g.borrow_mut().take()));
}

/// Primary side: start listening for "focus me" pokes from duplicate launches.
/// Each inbound connection yields one `()` on the returned receiver — drive it
/// on the GPUI loop to raise the window. `None` if the port couldn't be bound
/// (activation just won't work; the app still runs).
pub fn serve_focus() -> Option<UnboundedReceiver<()>> {
    let listener = match TcpListener::bind(("127.0.0.1", FOCUS_PORT)) {
        Ok(l) => l,
        Err(e) => {
            tracing::warn!(port = FOCUS_PORT, error = %harmoniya_api::obs::cause_chain(&e), "focus listener bind failed");
            return None;
        }
    };
    let (tx, rx) = unbounded::<()>();
    std::thread::spawn(move || {
        // The connection itself is the poke — content is irrelevant.
        for stream in listener.incoming() {
            if stream.is_err() {
                continue;
            }
            if tx.unbounded_send(()).is_err() {
                break; // GPUI loop gone — stop listening.
            }
        }
    });
    Some(rx)
}

/// Secondary side: poke the already-running primary so it raises its window.
/// Best-effort — if nothing's listening (primary didn't bind, or a port clash),
/// it's a no-op and the duplicate launch just exits.
pub fn signal_focus() {
    let _ = TcpStream::connect(("127.0.0.1", FOCUS_PORT));
}
