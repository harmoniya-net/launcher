//! Auto-update: on boot, check GitLab Releases for a newer build. If one exists,
//! take over the window with an updating screen while it downloads, then relaunch
//! into the new binary.

use std::time::Duration;

use futures::channel::oneshot;
use gpui::Context;

use super::{AppState, UpdatePhase};
use crate::shell::update;

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
    /// Check for a newer release in the background. If found, switch to the
    /// full-window updating screen, download it, and relaunch the app.
    pub fn check_for_update(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            // Look for a newer release off the UI thread.
            let version = match off_thread(update::check).await {
                Some(Ok(Some(v))) => v,
                Some(Ok(None)) => return, // already current
                Some(Err(e)) => {
                    tracing::warn!("update check: {e}");
                    return;
                }
                None => return,
            };

            // Take over the window, then download + install.
            if this
                .update(cx, |s, cx| {
                    s.update = Some(UpdatePhase::Downloading(version));
                    cx.notify();
                })
                .is_err()
            {
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
                        tracing::warn!("update install: {e}");
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
