//! Auto-update: check GitHub Releases for a newer build on boot, and again when
//! the window comes back from the tray (a launcher left in the tray for days
//! would otherwise never update). If one exists, take over the window with an
//! updating screen while it downloads, then relaunch into the new binary.

use std::time::{Duration, Instant};

use futures::channel::oneshot;
use gpui::Context;

use harmoniya_launch::pipeline::LaunchState;

use super::{AppState, LoginPhase, UpdatePhase};
use crate::shell::update;

/// Minimum gap between checks. The updater asks GitHub's API unauthenticated,
/// which allows 60 requests an hour per IP (shared by a whole household).
const RECHECK_AFTER: Duration = Duration::from_secs(60 * 60);

/// Run a blocking closure on a throwaway thread and await its result, so the
/// (sync) `self_update` network/disk work never blocks the UI or executor pools.
/// `None` if the worker thread vanished before sending.
async fn off_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.await.ok()
}

impl AppState {
    /// Whether relaunching now would cut something off: a running game, an
    /// install/launch in progress, or a sign-in waiting for browser approval.
    fn update_would_interrupt(&self) -> bool {
        !self.running.is_empty()
            || matches!(self.launch_state, LaunchState::Starting | LaunchState::Progress(_))
            || matches!(self.login_phase, LoginPhase::Starting | LoginPhase::AwaitingApproval { .. })
    }

    /// The window came back from the tray: check for an update unless that
    /// would interrupt something or we checked recently.
    pub fn check_for_update_on_restore(&mut self, cx: &mut Context<Self>) {
        let recently = self.last_update_check.is_some_and(|at| at.elapsed() < RECHECK_AFTER);
        if self.update.is_some() || recently || self.update_would_interrupt() {
            return;
        }
        self.check_for_update(cx);
    }

    /// Check for a newer release in the background. If found, switch to the
    /// full-window updating screen, download it, and relaunch the app.
    pub fn check_for_update(&mut self, cx: &mut Context<Self>) {
        self.last_update_check = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            // Look for a newer release off the UI thread.
            let version = match off_thread(update::check).await {
                Some(Ok(Some(v))) => v,
                Some(Ok(None)) => return, // already current
                Some(Err(e)) => {
                    tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "update check failed");
                    return;
                }
                None => return,
            };

            // Take over the window, then download + install — unless the user
            // started something while we were checking; the next restore retries.
            let started = this.update(cx, |s, cx| {
                if s.update_would_interrupt() {
                    return false;
                }
                s.update = Some(UpdatePhase::Downloading(version));
                cx.notify();
                true
            });
            if !matches!(started, Ok(true)) {
                return;
            }

            match off_thread(update::install).await {
                Some(Ok(true)) => {
                    let _ = this.update(cx, |s, cx| {
                        s.update = Some(UpdatePhase::Restarting);
                        cx.notify();
                    });
                    // Let the "restarting" frame paint, stop any running game
                    // gracefully, then re-exec the new binary.
                    cx.background_executor().timer(Duration::from_millis(700)).await;
                    harmoniya_api::http::on_tokio(harmoniya_launch::game::stop_all()).await;
                    update::relaunch();
                }
                other => {
                    if let Some(Err(e)) = other {
                        tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "update install failed");
                    }
                    // Nothing installed (or it failed) — drop the screen, carry on.
                    let _ = this.update(cx, |s, cx| {
                        s.update = None;
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}
