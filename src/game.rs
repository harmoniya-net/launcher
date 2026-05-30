//! Tracks launched game processes by modpack id so the UI can show a "stop"
//! affordance on a running modpack and stop it, and so all games are stopped
//! when the launcher quits.
//!
//! Stopping sends SIGTERM, waits up to 30s, then SIGKILL. On quit this must run
//! *before* `process::exit`, since GPUI's `on_app_quit` only gets ~100ms.

use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use tokio::process::Child;

/// How long a game gets to exit after SIGTERM before we force-kill it.
const GRACE: Duration = Duration::from_secs(30);

/// Running games, keyed by modpack id. `Mutex::new`/`Vec::new` are const, so no
/// lazy init; the list is tiny (usually 0–1), so linear scans are fine.
static GAMES: Mutex<Vec<(String, Child)>> = Mutex::new(Vec::new());
/// Pinged whenever the running set changes, so the UI can refresh.
static LISTENER: Mutex<Option<UnboundedSender<()>>> = Mutex::new(None);
static REAPER: Once = Once::new();

fn notify() {
    if let Some(tx) = LISTENER.lock().unwrap().as_ref() {
        let _ = tx.unbounded_send(());
    }
}

/// Register a sink that's pinged whenever the running set changes.
pub fn set_listener(tx: UnboundedSender<()>) {
    *LISTENER.lock().unwrap() = Some(tx);
}

/// Modpack ids with a currently-running game.
pub fn running_ids() -> Vec<String> {
    GAMES.lock().unwrap().iter().map(|(id, _)| id.clone()).collect()
}

/// Track a freshly-spawned game process and start the exit watcher once.
pub fn register(modpack_id: String, child: Child) {
    {
        let mut games = GAMES.lock().unwrap();
        games.retain(|(id, _)| id != &modpack_id);
        games.push((modpack_id, child));
    }
    notify();
    REAPER.call_once(|| {
        harmoniya_api::http::handle().spawn(async {
            let mut tick = tokio::time::interval(Duration::from_secs(2));
            loop {
                tick.tick().await;
                let mut games = GAMES.lock().unwrap();
                let before = games.len();
                games.retain_mut(|(_, c)| !matches!(c.try_wait(), Ok(Some(_))));
                let changed = games.len() != before;
                drop(games);
                if changed {
                    notify();
                }
            }
        });
    });
}

/// Stop one modpack's game (if running): SIGTERM, wait up to [`GRACE`], SIGKILL.
/// Removes it from the running set immediately so the UI reverts at once.
pub async fn stop(modpack_id: String) {
    let child = {
        let mut games = GAMES.lock().unwrap();
        games.iter().position(|(id, _)| id == &modpack_id).map(|i| games.remove(i).1)
    };
    notify();
    if let Some(child) = child {
        terminate(child).await;
    }
}

/// Immediate best-effort SIGTERM to every tracked game. Returns at once — used
/// as a fast backstop from `on_app_quit` for quit paths that bypass [`stop_all`].
#[cfg(unix)]
pub fn sigterm_all() {
    for (_, c) in GAMES.lock().unwrap().iter() {
        if let Some(pid) = c.id() {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
    }
}

#[cfg(not(unix))]
pub fn sigterm_all() {}

/// Stop every tracked game and wait for it to die. Unix: SIGTERM all up front so
/// the grace period is shared, then SIGKILL stragglers. Runs on the tokio runtime.
pub async fn stop_all() {
    let games: Vec<(String, Child)> = std::mem::take(&mut *GAMES.lock().unwrap());
    notify();
    if games.is_empty() {
        return;
    }
    tracing::info!("stopping {} game process(es)", games.len());

    #[cfg(unix)]
    {
        for (_, c) in &games {
            if let Some(pid) = c.id() {
                unsafe { libc::kill(pid as i32, libc::SIGTERM) };
            }
        }
        let deadline = Instant::now() + GRACE;
        for (_, mut child) in games {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if tokio::time::timeout(remaining, child.wait()).await.is_err() {
                tracing::warn!("game ignored SIGTERM after {}s; killing", GRACE.as_secs());
                let _ = child.start_kill();
                let _ = child.wait().await;
            }
        }
    }

    #[cfg(not(unix))]
    for (_, mut child) in games {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}

/// SIGTERM, wait up to [`GRACE`], then SIGKILL a single child.
async fn terminate(mut child: Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = child.id() {
            unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        }
        if tokio::time::timeout(GRACE, child.wait()).await.is_err() {
            tracing::warn!("game ignored SIGTERM after {}s; killing", GRACE.as_secs());
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
}
