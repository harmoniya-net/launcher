//! Skin profile, live preview overrides, and head-avatar prefetch.

use std::sync::Arc;

use gpui::{Context, Image, ImageFormat};
use harmoniya_api::services::yggdrasil::{fetch_profile, SkinModel};

use super::{fetch_image_bytes, is_auth_dead, with_access_token, AppEvent, AppState};

impl AppState {
    pub fn current_skin_model(&self) -> SkinModel {
        self.preview_skin_model
            .or_else(|| self.skin_profile.as_ref().map(|p| p.skin_model))
            .unwrap_or_default()
    }

    pub fn set_preview_skin_model(&mut self, model: SkinModel, cx: &mut Context<Self>) {
        if self.preview_skin_model != Some(model) {
            self.preview_skin_model = Some(model);
            cx.notify();
        }
    }

    pub fn set_preview_skin_bytes(&mut self, bytes: Option<Arc<Vec<u8>>>, cx: &mut Context<Self>) {
        self.preview_skin_bytes = bytes;
        cx.notify();
    }

    pub fn set_preview_cape_bytes(&mut self, bytes: Option<Arc<Vec<u8>>>, cx: &mut Context<Self>) {
        self.preview_cape_bytes = bytes;
        cx.notify();
    }

    pub fn fetch_skin_profile(&mut self, cx: &mut Context<Self>) {
        let Some(tokens) = self.tokens.clone() else { return; };
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(
                with_access_token(tokens, |t| async move {
                    fetch_profile(&t).await.map(|opt| opt.unwrap_or_default())
                })
            ).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok((profile, refreshed)) => {
                        state.adopt_tokens(refreshed);
                        state.skin_profile = Some(profile);
                        state.prefetch_head(cx);
                        cx.emit(AppEvent::SkinProfileLoaded);
                        cx.notify();
                    }
                    Err(e) => {
                        tracing::warn!("fetch_skin_profile failed: {e}");
                        if is_auth_dead(&e) { state.logout(cx); }
                    }
                }
            }).ok();
        }).detach();
    }

    /// Download the skin texture (if any) and pre-render the head face so the
    /// UserBar can swap from initial-letter placeholder to the real avatar.
    pub fn prefetch_head(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.skin_profile.as_ref().and_then(|p| p.skin_url.clone()) else { return; };
        if self.head_cache.contains_key(&url) { return; }
        cx.spawn(async move |this, cx| {
            let fetch_url = url.clone();
            let bytes = match harmoniya_api::http::on_tokio(async move { fetch_image_bytes(&fetch_url).await }).await {
                Ok(b) => b,
                Err(_) => return,
            };
            let head_png = match mc_skin::head::render(&bytes) {
                Ok(b) => b,
                Err(_) => return,
            };
            let image = Arc::new(Image::from_bytes(ImageFormat::Png, head_png));
            this.update(cx, |state, cx| {
                state.head_cache.insert(url, image);
                cx.notify();
            }).ok();
        }).detach();
    }
}
