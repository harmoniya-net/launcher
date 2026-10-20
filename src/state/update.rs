//! Auto-update: check GitHub Releases for a newer build on boot, and again when
//! the window comes back from the tray (a launcher left in the tray for days
//! would otherwise never update). If one exists, take over the window with an
//! updating screen while it downloads, then relaunch into the new binary.

use std::time::{Duration, Instant};

use futures::channel::oneshot;
use gpui::Context;

use harmoniya_launch::pipeline::LaunchState;

use super::{AppState, LoginPhase, ManualUpdate, UpdatePhase};
use crate::shell::update;

/// Minimum gap between checks on tray restore.
const RECHECK_AFTER: Duration = Duration::from_secs(60 * 60);

/// Retrying a failed automatic check (no network yet at boot, a blip): wait
/// this long first, doubling up to [`RETRY_MAX`].
const RETRY_FIRST: Duration = Duration::from_secs(60);
const RETRY_MAX: Duration = Duration::from_secs(30 * 60);

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
    /// Whether relaunching now would cut off a running game or an
    /// install/launch in progress. Never overridden, not even by the button.
    fn update_would_stop_game(&self) -> bool {
        !self.running.is_empty()
            || matches!(self.launch_state, LaunchState::Starting | LaunchState::Progress(_))
    }

    /// Automatic checks also hold off while a sign-in waits for approval; the
    /// user asking for an update overrides that.
    fn update_would_interrupt(&self) -> bool {
        self.update_would_stop_game()
            || matches!(self.login_phase, LoginPhase::Starting | LoginPhase::AwaitingApproval { .. })
    }

    /// The "Update launcher" button: update now if there's a newer release,
    /// skipping the hourly throttle. Reports the outcome in `manual_update`.
    pub fn update_now(&mut self, cx: &mut Context<Self>) {
        if self.update.is_some() || matches!(self.manual_update, ManualUpdate::Checking) {
            return;
        }
        self.manual_update = if self.update_would_stop_game() {
            ManualUpdate::Busy
        } else {
            self.run_update(true, cx);
            ManualUpdate::Checking
        };
        cx.notify();
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
        self.run_update(false, cx);
    }

    /// A check failed. Let the next tray restore check again right away, and
    /// retry on our own with a growing delay. The retry goes through the
    /// restore path, so it skips itself if another check ran in the meantime
    /// or a game is running (the next restore then picks it up).
    fn retry_update_check(&mut self, cx: &mut Context<Self>) {
        self.last_update_check = None;
        let delay = (self.update_retry_delay * 2).clamp(RETRY_FIRST, RETRY_MAX);
        self.update_retry_delay = delay;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let _ = this.update(cx, |s, cx| s.check_for_update_on_restore(cx));
        })
        .detach();
    }

    /// `manual`: the user asked (button) — report the outcome, and don't wait
    /// for a pending sign-in.
    fn run_update(&mut self, manual: bool, cx: &mut Context<Self>) {
        self.last_update_check = Some(Instant::now());
        cx.spawn(async move |this, cx| {
            let report = |outcome: ManualUpdate, cx: &mut gpui::AsyncApp| {
                if manual {
                    let _ = this.update(cx, |s, cx| {
                        s.manual_update = outcome;
                        cx.notify();
                    });
                }
            };
            // Look for a newer release off the UI thread.
            let version = match off_thread(update::check).await {
                Some(Ok(Some(v))) => v,
                Some(Ok(None)) => return report(ManualUpdate::UpToDate, cx),
                Some(Err(e)) => {
                    tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "update check failed");
                    let _ = this.update(cx, |s, cx| s.retry_update_check(cx));
                    return report(ManualUpdate::Failed, cx);
                }
                None => return report(ManualUpdate::Failed, cx),
            };
            let _ = this.update(cx, |s, _| s.update_retry_delay = Duration::ZERO);

            // Take over the window, then download + install — unless the user
            // started something while we were checking; the next restore retries.
            let started = this.update(cx, |s, cx| {
                let blocked = if manual { s.update_would_stop_game() } else { s.update_would_interrupt() };
                if blocked {
                    return false;
                }
                s.update = Some(UpdatePhase::Downloading(version.clone()));
                s.manual_update = ManualUpdate::Idle;
                cx.notify();
                true
            });
            if !matches!(started, Ok(true)) {
                return report(ManualUpdate::Busy, cx);
            }

            let install_version = version.clone();
            match off_thread(move || update::install(&install_version)).await {
                Some(Ok(())) => {
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
                    // It failed: drop the screen, carry on.
                    let _ = this.update(cx, |s, cx| {
                        s.update = None;
                        if manual {
                            s.manual_update = ManualUpdate::Failed;
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}
