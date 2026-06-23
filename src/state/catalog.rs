//! Modpack catalog: list/groups, favourites, banner prefetch, and the
//! per-modpack option store.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Context, Image};
use harmoniya_api::config;
use harmoniya_api::services::modpacks::{fetch_all, group};

use super::{fetch_image_bytes, guess_format, AppEvent, AppState};

/// Don't refetch the catalog from the live CMS more than once per this window;
/// the select-server / regain-focus refreshes are best-effort freshness, not a
/// reason to hammer prod.
const CATALOG_REFRESH_COOLDOWN: Duration = Duration::from_secs(30);

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
        let changed = self.selection.selected_modpack_id != modpack_id;
        self.selection.selected_modpack_id = modpack_id;
        let _ = config::save_json(config::SELECTION_FILE, &self.selection);
        cx.notify();
        // Switching server is a natural moment to pull fresh CMS data.
        if changed {
            self.refresh_modpacks(cx);
        }
    }

    /// Initial catalog load: shows the loading state and surfaces errors.
    pub fn fetch_modpacks(&mut self, cx: &mut Context<Self>) {
        if self.modpacks_loading {
            return;
        }
        self.modpacks_error = None;
        self.spawn_catalog_fetch(false, cx);
    }

    /// Silently re-pull the catalog, updating it in place — no loading flash, and
    /// a transient failure keeps the current data. Triggered by switching server
    /// and regaining focus; a no-op until the initial load has populated the
    /// catalog, and rate-limited so we don't hammer the live CMS.
    pub fn refresh_modpacks(&mut self, cx: &mut Context<Self>) {
        if self.modpacks.is_empty() || self.modpacks_loading {
            return;
        }
        if self.last_catalog_fetch.is_some_and(|t| t.elapsed() < CATALOG_REFRESH_COOLDOWN) {
            return;
        }
        self.spawn_catalog_fetch(true, cx);
    }

    /// Manual refresh (Ctrl+R): refetch now, bypassing the background cooldown.
    /// An empty catalog does a visible load; otherwise it refreshes in place.
    pub fn reload_modpacks(&mut self, cx: &mut Context<Self>) {
        if self.modpacks_loading {
            return;
        }
        if self.modpacks.is_empty() {
            self.modpacks_error = None;
            self.spawn_catalog_fetch(false, cx);
        } else {
            self.spawn_catalog_fetch(true, cx);
        }
    }

    fn spawn_catalog_fetch(&mut self, silent: bool, cx: &mut Context<Self>) {
        self.modpacks_loading = true;
        self.last_catalog_fetch = Some(Instant::now());
        if !silent {
            cx.notify(); // surface the loading state for the initial load
        }
        cx.spawn(async move |this, cx| {
            let result = harmoniya_api::http::on_tokio(fetch_all()).await;
            this.update(cx, |state, cx| {
                state.modpacks_loading = false;
                match result {
                    Ok(items) => {
                        state.groups = group(&items);
                        state.modpacks = items;
                        state.prefetch_banners(cx);
                        state.prefetch_logos(cx);
                        state.fetch_lucky_profile(cx);
                        cx.emit(AppEvent::ModpacksLoaded);
                        cx.notify();
                    }
                    // Keep the stale-but-valid catalog on a background refresh.
                    Err(e) if silent => tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "modpack refresh failed"),
                    Err(e) => {
                        state.modpacks_error = Some(e.to_string());
                        cx.notify();
                    }
                }
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

    /// Fetch each project's logo, resize it to 40×40 with Lanczos3 (2× the 20 px
    /// display size for HiDPI), and cache the result. GPUI's built-in downscale
    /// is nearest-neighbour and looks pixelated at this size; pre-scaling here
    /// fixes that at the cost of a one-time CPU pass per logo.
    pub fn prefetch_logos(&mut self, cx: &mut Context<Self>) {
        let urls: Vec<String> = self
            .groups
            .iter()
            .filter_map(|g| g.project.logo.url.clone())
            .filter(|u| !self.logo_cache.contains_key(u))
            .collect();
        if urls.is_empty() { return; }

        cx.spawn(async move |this, cx| {
            let fetches = urls.into_iter().map(|url| async move {
                let bytes = fetch_image_bytes(&url).await.ok()?;
                // SVG logos are XML the `image` crate can't decode (it would
                // error out and the logo would silently never cache). Hand the
                // raw bytes to GPUI, which rasterizes them via resvg at render
                // time and scales cleanly on its own, so the Lanczos pre-pass
                // below doesn't apply.
                if is_svg(&bytes) {
                    return Some((url, Arc::new(Image::from_bytes(gpui::ImageFormat::Svg, bytes))));
                }
                let img = image::load_from_memory(&bytes).ok()?;
                let resized = img.resize_exact(40, 40, image::imageops::FilterType::Lanczos3);
                let mut png: Vec<u8> = Vec::new();
                resized
                    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
                    .ok()?;
                Some((url, Arc::new(Image::from_bytes(gpui::ImageFormat::Png, png))))
            });
            let results = harmoniya_api::http::on_tokio(futures::future::join_all(fetches)).await;
            this.update(cx, |state, cx| {
                let mut added = false;
                for r in results.into_iter().flatten() {
                    state.logo_cache.insert(r.0, r.1);
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

/// Sniff whether fetched image bytes are SVG. Logos come from the CMS without a
/// guaranteed file extension, so we look at the content: an `<svg` tag near the
/// start, allowing for a leading XML declaration / BOM / whitespace.
fn is_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head);
    text.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .starts_with("<?xml")
        || text.contains("<svg")
}
