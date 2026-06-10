//! The play → install → launch flow, plus hiding the window to the tray once a
//! game starts.

use std::time::Duration;

use futures::StreamExt;
use gpui::{AppContext, Context};
use harmoniya_api::services::options;
use harmoniya_launch::pipeline::{self as launch, LaunchMsg, LaunchState};

use super::{ActiveModal, AppState, MainWindow};

impl AppState {
    /// Stop the running game for a modpack (SIGTERM, ≤30s, SIGKILL). The running
    /// set updates via the game listener once it's torn down.
    pub fn stop_game(&mut self, modpack_id: String, cx: &mut Context<Self>) {
        cx.background_spawn(async move {
            harmoniya_api::http::on_tokio(harmoniya_launch::game::stop(modpack_id)).await;
        })
        .detach();
    }

    /// Hide the main window to the tray via the global handle, deferred so it
    /// runs once the current update cycle unwinds.
    pub fn hide_window(&self, cx: &mut Context<Self>) {
        let Some(MainWindow(handle)) = cx.try_global::<MainWindow>() else { return };
        let handle = *handle;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, app| crate::shell::window_ctl::hide(window, app));
        });
    }

    /// Bring the main window to the foreground and focus it — e.g. after the
    /// browser OAuth flow completes, so the user lands back on the launcher
    /// instead of the (auto-closing) success tab.
    pub fn focus_window(&self, cx: &mut Context<Self>) {
        let Some(MainWindow(handle)) = cx.try_global::<MainWindow>() else { return };
        let handle = *handle;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, app| crate::shell::window_ctl::show(window, app));
        });
    }

    /// Open the launch modal and start installing/launching the selected modpack.
    pub fn play(&mut self, cx: &mut Context<Self>) {
        self.open_modal(ActiveModal::Launch, cx);
        self.start_launch(cx);
    }

    /// Begin (or restart) the launch flow for the currently selected modpack.
    pub fn start_launch(&mut self, cx: &mut Context<Self>) {
        if matches!(self.launch_state, LaunchState::Starting | LaunchState::Progress(_)) {
            return;
        }
        let Some(tokens) = self.tokens.clone() else {
            self.launch_state = LaunchState::Error(launch::LaunchError {
                code: launch::ErrorCode::Unknown,
                message: "Увійдіть в акаунт, щоб запустити гру.".into(),
                phase: None,
                paths: Vec::new(),
            });
            cx.notify();
            return;
        };
        let Some(modpack) = self.selected_modpack().cloned() else { return; };
        if modpack.manifest_url.trim().is_empty() {
            self.launch_state = LaunchState::Error(launch::LaunchError {
                code: launch::ErrorCode::Unknown,
                message: "Для цього модпаку не налаштовано маніфест.".into(),
                phase: None,
                paths: Vec::new(),
            });
            cx.notify();
            return;
        }

        let data_dir = launch::resolve_data_dir(self.settings.data_dir.as_deref());
        self.launch_state = LaunchState::Starting;
        cx.notify();

        let (tx, mut rx) = futures::channel::mpsc::unbounded::<LaunchMsg>();
        let manifest_url = modpack.manifest_url.clone();
        let modpack_id = modpack.id.clone();

        // Resolve the modpack's options (from the CMS) into extra vars + enabled features.
        let saved = self.settings.modpack_options.get(&modpack.id).cloned().unwrap_or_default();
        let (extra_vars, features) = options::resolve(&modpack.options, &saved);

        // Cancellation handle so closing the launch modal aborts the install.
        let cancel = launch::CancellationToken::new();
        self.launch_cancel = Some(cancel.clone());

        // Worker runs the whole pipeline on the tokio runtime and reports via tx.
        cx.background_spawn(async move {
            harmoniya_api::http::on_tokio(launch::run(
                tokens,
                modpack_id,
                manifest_url,
                data_dir,
                extra_vars,
                features,
                cancel,
                tx,
            ))
            .await;
        })
        .detach();

        // Receiver loop applies updates on the UI thread until the channel closes.
        let handle = cx.spawn(async move |this, cx| {
            while let Some(msg) = rx.next().await {
                let stop = this
                    .update(cx, |state, cx| state.apply_launch_msg(msg, cx))
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
        });
        self.launch_task = Some(handle);
    }

    /// Apply one streamed launch message. Returns `true` when the stream should
    /// stop (terminal state reached).
    fn apply_launch_msg(&mut self, msg: LaunchMsg, cx: &mut Context<Self>) -> bool {
        match msg {
            LaunchMsg::TokensRefreshed(t) => {
                self.adopt_tokens(Some(t));
                false
            }
            LaunchMsg::Progress(p) => {
                self.launch_state = LaunchState::Progress(p);
                cx.notify();
                false
            }
            LaunchMsg::Error(e) => {
                tracing::warn!("launch failed: {} ({:?})", e.message, e.code);
                self.launch_state = LaunchState::Error(e);
                cx.notify();
                true
            }
            LaunchMsg::Done { pid } => {
                tracing::info!("game launched (pid {pid})");
                self.launch_state = LaunchState::Done { pid };
                // Keep the modal up on its "launching" view for now (don't close
                // yet) — the game window takes a few seconds to spin up, and
                // hiding instantly would drop the user onto a blank desktop.
                cx.notify();
                self.hide_after_launch(cx);
                true
            }
        }
    }

    /// Linger on the post-launch "launching" view for a beat, then close the
    /// launch modal and hide the launcher to the tray. Detached and guarded:
    /// if the user dismissed the modal or kicked off another launch in the
    /// meantime, it quietly no-ops.
    fn hide_after_launch(&self, cx: &mut Context<Self>) {
        const LINGER: Duration = Duration::from_secs(5);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(LINGER).await;
            let _ = this.update(cx, |state, cx| {
                if !matches!(state.launch_state, LaunchState::Done { .. }) {
                    return;
                }
                if state.active_modal == Some(ActiveModal::Launch) {
                    state.active_modal = None;
                }
                state.hide_window(cx);
                cx.notify();
            });
        })
        .detach();
    }
}
