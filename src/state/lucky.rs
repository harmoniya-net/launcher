use gpui::Context;
use harmoniya_api::auth;
use harmoniya_api::services::lucky;

use super::{with_access_token, AppState};

impl AppState {
    pub fn fetch_lucky_profile(&mut self, cx: &mut Context<Self>) {
        if !self.session.signed_in() { return; }
        let store = self.session.clone();
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(with_access_token(store, |access| async move {
                let ygg = auth::fetch_yggdrasil_token(&access).await?;
                lucky::fetch_profile(&ygg).await
            }))
            .await;
            this.update(cx, |state, cx| match result {
                Ok(profile) => {
                    state.lucky_profile = Some(profile);
                    cx.notify();
                }
                Err(e) => tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "fetch_lucky_profile failed"),
            })
            .ok();
        })
        .detach();
    }
}
