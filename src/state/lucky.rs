use gpui::Context;
use harmoniya_api::auth;
use harmoniya_api::services::lucky;

use super::AppState;

impl AppState {
    pub fn fetch_lucky_profile(&mut self, cx: &mut Context<Self>) {
        let Some(tokens) = self.tokens.clone() else { return; };
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(async move {
                let (tokens, _) = auth::ensure_fresh(tokens).await?;
                let ygg = auth::fetch_yggdrasil_token(&tokens.access_token).await?;
                lucky::fetch_profile(&ygg).await
            })
            .await;
            this.update(cx, |state, cx| match result {
                Ok(profile) => {
                    state.lucky_profile = Some(profile);
                    cx.notify();
                }
                Err(e) => tracing::warn!("fetch_lucky_profile: {e}"),
            })
            .ok();
        })
        .detach();
    }
}
