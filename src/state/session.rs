//! Authentication: device-flow sign-in, sign-out, and loading the current user.

use gpui::Context;
use harmoniya_api::auth::{self, LoginError};
use harmoniya_api::http::{on_tokio, on_tokio_abortable};
use harmoniya_api::services::account::fetch_me;

use super::{is_auth_dead, with_token, AppEvent, AppState, LoginPhase, Route};

impl AppState {
    pub fn fetch_user(&mut self, cx: &mut Context<Self>) {
        if !self.session.signed_in() { return; }
        let store = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = on_tokio(with_token(store, |t| async move { fetch_me(&t).await })).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok(user) => {
                        state.user = Some(user);
                        cx.emit(AppEvent::UserLoaded);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "fetch_me failed");
                        if is_auth_dead(&e) { state.logout(cx); } else { cx.notify(); }
                    }
                }
            }).ok();
        }).detach();
    }

    /// Start a device-flow sign-in: get a code, open the approval page, and
    /// poll in the background until the user approves it there.
    pub fn login(&mut self, provider: auth::Provider, cx: &mut Context<Self>) {
        // Debounce a double-click while the code is requested. Clicking again
        // while waiting for approval starts over: replacing
        // `pending_login_task` below drops (cancels) the previous attempt.
        if matches!(self.login_phase, LoginPhase::Starting) { return; }
        self.login_phase = LoginPhase::Starting;
        cx.notify();
        let handle = cx.spawn(async move |this, cx| {
            let login = match on_tokio(auth::start_login(provider)).await {
                Ok(login) => login,
                Err(e) => {
                    this.update(cx, |state, cx| {
                        state.pending_login_task = None;
                        state.login_phase = LoginPhase::Error(e.to_string());
                        cx.notify();
                    }).ok();
                    return;
                }
            };
            // The code and a "open again" link stay on screen, so a browser
            // that didn't open isn't fatal.
            if let Err(e) = login.open_browser() {
                tracing::warn!(error = %e, "failed to open the sign-in page");
            }
            if this.update(cx, |state, cx| {
                state.login_phase = LoginPhase::AwaitingApproval {
                    user_code: login.user_code.clone(),
                    url: login.verification_url.clone(),
                };
                cx.notify();
            }).is_err() {
                return;
            }

            // Abortable, so cancelling the sign-in stops the polling too.
            let result = on_tokio_abortable(login.wait()).await;
            this.update(cx, |state, cx| {
                state.pending_login_task = None;
                match result {
                    Ok(token) => {
                        state.session.set(token);
                        state.login_phase = LoginPhase::Idle;
                        state.set_route(Route::Account, cx);
                        cx.emit(AppEvent::AuthChanged);
                        // The user approved in the browser; bring the launcher back.
                        state.focus_window(cx);
                    }
                    Err(e) => {
                        let t = crate::i18n::t();
                        let msg = match e {
                            LoginError::Denied => t.login_denied.to_string(),
                            LoginError::Expired => t.login_expired.to_string(),
                            LoginError::Other(e) => e.to_string(),
                        };
                        state.login_phase = LoginPhase::Error(msg);
                        cx.notify();
                    }
                }
            }).ok();
        });
        self.pending_login_task = Some(handle);
    }

    /// Abandon the sign-in that is waiting for approval.
    pub fn cancel_login(&mut self, cx: &mut Context<Self>) {
        self.pending_login_task = None;
        self.login_phase = LoginPhase::Idle;
        cx.notify();
    }

    /// Open the approval page again, e.g. after the user closed the tab.
    pub fn reopen_login_page(&self) {
        if let LoginPhase::AwaitingApproval { url, .. } = &self.login_phase {
            if let Err(e) = open::that(url) {
                tracing::warn!(error = %e, "failed to open the sign-in page");
            }
        }
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        if let Some(token) = self.session.take() {
            // Best effort: the local session is already gone either way.
            harmoniya_api::http::handle().spawn(async move {
                if let Err(e) = auth::sign_out(&token).await {
                    tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "sign-out failed");
                }
            });
        }
        self.user = None;
        self.lucky_profile = None;
        self.skin_profile = None;
        self.head_cache.clear();
        self.set_route(Route::Login, cx);
        cx.emit(AppEvent::AuthChanged);
    }
}
