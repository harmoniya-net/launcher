//! Authentication: OAuth login, logout, and loading the current user.

use gpui::Context;
use harmoniya_api::auth;
use harmoniya_api::services::account::fetch_me;

use super::{is_auth_dead, with_access_token, AppEvent, AppState, LoginPhase, Route};

impl AppState {
    pub fn fetch_user(&mut self, cx: &mut Context<Self>) {
        if !self.session.signed_in() { return; }
        let store = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(
                with_access_token(store, |t| async move { fetch_me(&t).await })
            ).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok(user) => {
                        state.user = Some(user);
                        cx.emit(AppEvent::UserLoaded);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::warn!("fetch_me failed: {e}");
                        if is_auth_dead(&e) { state.logout(cx); } else { cx.notify(); }
                    }
                }
            }).ok();
        }).detach();
    }

    pub fn login(&mut self, provider: auth::Provider, cx: &mut Context<Self>) {
        // `Waiting` now covers only the brief browser-open step, so this guard
        // just debounces a rapid double-click — it does NOT block for the whole
        // auth. Once the page is open we drop back to `Idle` (below), so closing
        // the tab and clicking again starts a fresh flow instead of hanging.
        if matches!(self.login_phase, LoginPhase::Waiting) { return; }
        self.login_phase = LoginPhase::Waiting;
        cx.notify();
        let handle = cx.spawn(async move |this, cx| {
            // Step 1 — open the browser. Fast and non-blocking (no callback wait).
            let pending = match auth::begin_login_flow(provider) {
                Ok(p) => p,
                Err(e) => {
                    this.update(cx, |state, cx| {
                        state.pending_login_task = None;
                        state.login_phase = LoginPhase::Error(e.to_string());
                        cx.notify();
                    }).ok();
                    return;
                }
            };
            // Page is open — unblock the buttons immediately so the user can
            // retry or switch provider if they close the tab.
            if this.update(cx, |state, cx| {
                state.login_phase = LoginPhase::Idle;
                cx.notify();
            }).is_err() {
                return;
            }
            // Step 2 — await the redirect + token exchange in the background.
            let result = harmoniya_api::http::on_tokio(pending.finish()).await;
            this.update(cx, |state, cx| {
                state.pending_login_task = None;
                match result {
                    Ok(tokens) => {
                        state.session.set(tokens);
                        state.login_phase = LoginPhase::Idle;
                        state.set_route(Route::Account, cx);
                        cx.emit(AppEvent::AuthChanged);
                        // The browser stole focus during OAuth; pull the launcher
                        // back to the front (the success tab closes shortly after).
                        state.focus_window(cx);
                    }
                    Err(e) => {
                        // Only surface the failure if we're still signed out — a
                        // newer attempt or a completed login shouldn't be clobbered
                        // by a stale flow's timeout.
                        if !state.session.signed_in() {
                            state.login_phase = LoginPhase::Error(e.to_string());
                            cx.notify();
                        }
                    }
                }
            }).ok();
        });
        self.pending_login_task = Some(handle);
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        self.session.clear();
        self.user = None;
        self.lucky_profile = None;
        self.skin_profile = None;
        self.head_cache.clear();
        self.set_route(Route::Login, cx);
        cx.emit(AppEvent::AuthChanged);
    }
}
