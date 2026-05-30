//! Single-instance guard. The first launch binds a fixed loopback port and
//! listens on it; a second launch fails to bind, connects to ping "focus", and
//! exits — so the already-running window is brought to front instead of a
//! duplicate opening.
//!
//! Loopback TCP (rather than a Unix socket / Windows named pipe) is portable
//! with no extra dependencies and has no stale-endpoint problem: the OS frees
//! the port the moment the holder exits, so a failed bind reliably means a live
//! instance owns it.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

/// Fixed loopback endpoint for the single-instance channel.
const ADDR: &str = "127.0.0.1:47823";
const PING: &[u8] = b"focus";

pub enum Instance {
    /// We're the only instance. Holds the listener for focus pings (`None` if we
    /// couldn't bind for some unrelated reason — run anyway, just unguarded).
    Primary(Option<TcpListener>),
    /// Another instance is already running; it has been pinged to focus.
    AlreadyRunning,
}

/// Try to become the primary instance, or hand off to a running one.
pub fn acquire() -> Instance {
    match TcpListener::bind(ADDR) {
        Ok(listener) => Instance::Primary(Some(listener)),
        Err(_) => {
            // Port busy — ask whoever is listening to focus, then bow out.
            if let Ok(mut stream) = TcpStream::connect(ADDR) {
                let _ = stream.write_all(PING);
                let _ = stream.flush();
                Instance::AlreadyRunning
            } else {
                // Couldn't bind and nobody answered (transient): don't block startup.
                Instance::Primary(None)
            }
        }
    }
}

/// Run the accept loop on a background thread, invoking `on_focus` for each
/// valid focus ping from a second instance.
pub fn serve(listener: TcpListener, on_focus: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
            let mut buf = [0u8; 16];
            match stream.read(&mut buf) {
                Ok(n) if buf[..n].starts_with(PING) => on_focus(),
                _ => {}
            }
        }
    });
}
