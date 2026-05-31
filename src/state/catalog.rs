//! Modpack catalog: list/groups, favourites, banner prefetch, and the
//! per-modpack option store.

use std::sync::Arc;

use gpui::{Context, Image};
use harmoniya_api::config;
use harmoniya_api::services::modpacks::{fetch_all, group};

use super::{fetch_image_bytes, guess_format, AppEvent, AppState};

impl AppState {
    pub fn is_favourite(&self, modpack_id: &str) -> bool {
        self.favourites.contains(modpack_id)
    }

    /// Pin/unpin a modpack to the Favourites group.
    pub fn toggle_favourite(&mut self, modpack_id: String, cx: &mut Context<Self>) {
        if !self.favourites.remove(&modpack_id) {
            self.favourites.insert(modpack_id);
        }
        let _ = config::save_json(config::FAVOURITES_FILE, &self.favourites);
        cx.notify();
    }

    pub fn select_modpack(&mut self, modpack_id: Option<String>, cx: &mut Context<Self>) {
        self.selection.selected_modpack_id = modpack_id;
        let _ = config::save_json(config::SELECTION_FILE, &self.selection);
        cx.notify();
    }

    pub fn fetch_modpacks(&mut self, cx: &mut Context<Self>) {
        if self.modpacks_loading { return; }
        self.modpacks_loading = true;
        self.modpacks_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(fetch_all()).await;
            this.update(cx, |state, cx| {
                state.modpacks_loading = false;
                match result {
                    Ok(items) => {
                        state.groups = group(&items);
                        state.modpacks = items;
                        state.prefetch_banners(cx);
                        cx.emit(AppEvent::ModpacksLoaded);
                    }
                    Err(e) => {
                        state.modpacks_error = Some(e.to_string());
                    }
                }
                cx.notify();
            }).ok();
        }).detach();
    }

    /// Kick off background downloads of every modpack's banner at every aspect
    /// we render (inactive card, active card, hero). Each completed download
    /// lands in `banner_cache` and `cx.notify` schedules the swap. Since each
    /// state has its own URL, we can use `ObjectFit::Fill` everywhere (which is
    /// what GPUI needs to round corners) without distortion — the image was
    /// already pre-cropped server-side to the exact aspect.
    pub fn prefetch_banners(&mut self, cx: &mut Context<Self>) {
        let urls: Vec<String> = self
            .modpacks
            .iter()
            .filter_map(|m| m.banner.as_ref().and_then(|b| b.url.as_deref()))
            .flat_map(|u| {
                [
                    crate::banner::at_size(u, 816, 400),  // card (active aspect; same URL inactive)
                    crate::banner::at_size(u, 2400, 440), // hero
                ]
            })
            .filter(|u| !self.banner_cache.contains_key(u))
            .collect();
        if urls.is_empty() { return; }

        cx.spawn(async move |this, cx| {
            // Run all fetches concurrently on the tokio runtime; insert each
            // result as it lands so the UI starts hitting cache as soon as
            // possible (don't wait for the slowest banner).
            let fetches = urls.into_iter().map(|url| async move {
                let bytes = fetch_image_bytes(&url).await.ok()?;
                let format = guess_format(&url);
                Some((url, Arc::new(Image::from_bytes(format, bytes))))
            });
            let results = harmoniya_api::http::on_tokio(futures::future::join_all(fetches)).await;
            this.update(cx, |state, cx| {
                let mut added = false;
                for r in results.into_iter().flatten() {
                    state.banner_cache.insert(r.0, r.1);
                    added = true;
                }
                if added { cx.notify(); }
            }).ok();
        }).detach();
    }

    // ── Per-modpack options ──────────────────────────────────────────────

    /// Saved value for a leaf option (var), if the user set one.
    pub fn option_value(&self, modpack_id: &str, name: &str) -> Option<String> {
        self.settings.modpack_options.get(modpack_id)?.vars.get(name).cloned()
    }

    /// Set (or clear, with `None`) a leaf option's value.
    pub fn set_option_value(&mut self, modpack_id: String, name: String, value: Option<String>, cx: &mut Context<Self>) {
        let entry = self.settings.modpack_options.entry(modpack_id).or_default();
        // Skip redundant writes: a slider drag calls this on every mouse-move,
        // but the stepped value only crosses a boundary occasionally.
        let changed = match &value {
            Some(v) => entry.vars.get(&name) != Some(v),
            None => entry.vars.contains_key(&name),
        };
        if !changed {
            return;
        }
        match value {
            Some(v) => { entry.vars.insert(name, v); }
            None => { entry.vars.remove(&name); }
        }
        let _ = config::save_json(config::SETTINGS_FILE, &self.settings);
        cx.notify();
    }

    /// Toggle a feature on/off for a modpack.
    pub fn set_feature(&mut self, modpack_id: String, name: String, enabled: bool, cx: &mut Context<Self>) {
        self.settings.modpack_options.entry(modpack_id).or_default().features.insert(name, enabled);
        let _ = config::save_json(config::SETTINGS_FILE, &self.settings);
        cx.notify();
    }
}
