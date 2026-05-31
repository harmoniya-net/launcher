//! Authentication: OAuth login, logout, and loading the current user.

use gpui::Context;
use harmoniya_api::auth;
use harmoniya_api::services::account::fetch_me;

use super::{is_auth_dead, with_access_token, AppEvent, AppState, LoginPhase, Route};

impl AppState {
    pub fn fetch_user(&mut self, cx: &mut Context<Self>) {
        let Some(tokens) = self.tokens.clone() else { return; };
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(
                with_access_token(tokens, |t| async move { fetch_me(&t).await })
            ).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok((user, refreshed)) => {
                        state.adopt_tokens(refreshed);
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
        if matches!(self.login_phase, LoginPhase::Waiting) { return; }
        self.login_phase = LoginPhase::Waiting;
        cx.notify();
        let handle = cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(auth::run_login_flow(provider)).await;
            this.update(cx, |state, cx| {
                state.pending_login_task = None;
                match result {
                    Ok(tokens) => {
                        let _ = auth::storage::save(&tokens);
                        state.tokens = Some(tokens);
                        state.login_phase = LoginPhase::Idle;
                        state.set_route(Route::Account, cx);
                        cx.emit(AppEvent::AuthChanged);
                        // The browser stole focus during OAuth; pull the launcher
                        // back to the front (the success tab closes shortly after).
                        state.focus_window(cx);
                    }
                    Err(e) => {
                        state.login_phase = LoginPhase::Error(e.to_string());
                        cx.notify();
                    }
                }
            }).ok();
        });
        self.pending_login_task = Some(handle);
    }

    pub fn logout(&mut self, cx: &mut Context<Self>) {
        auth::storage::clear();
        self.tokens = None;
        self.user = None;
        self.skin_profile = None;
        self.head_cache.clear();
        self.set_route(Route::Login, cx);
        cx.emit(AppEvent::AuthChanged);
    }
}
