use gpui::{App, AppContext, Context, Entity, EventEmitter, Global, Image, ImageFormat, Task};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::auth::{self, tokens::Tokens};
use crate::persistence;
use crate::services::{
    account::{User, fetch_me},
    launch::{self, LaunchMsg, LaunchState},
    modpacks::{Modpack, ProjectGroup, fetch_all, group},
    options::{self, ModpackOptions},
    yggdrasil::{SkinModel, SkinProfile, fetch_profile},
};
use futures::StreamExt;

#[derive(Clone, Debug)]
pub enum Route {
    Login,
    Account,
    Skin { tab: SkinTab },
}

impl Default for Route {
    fn default() -> Self { Route::Login }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkinTab { Skin, Launcher }

/// Modals are tracked at the app level so the overlay can be rendered at the
/// root view (covering the full window) instead of being scoped to whichever
/// pane triggered it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveModal { Launch, Settings, News }

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Selection {
    pub selected_modpack_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub data_dir: Option<String>,
    /// Per-modpack option choices (vars + enabled features), keyed by modpack id.
    #[serde(default)]
    pub modpack_options: HashMap<String, ModpackOptions>,
    /// Closing the window hides to the tray (default) instead of quitting.
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
}

fn default_close_to_tray() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            data_dir: None,
            modpack_options: HashMap::new(),
            close_to_tray: default_close_to_tray(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum LoginPhase {
    Idle,
    Waiting,
    Error(String),
}

pub struct AppState {
    pub route: Route,
    pub tokens: Option<Tokens>,
    pub user: Option<User>,
    pub modpacks: Vec<Modpack>,
    pub groups: Vec<ProjectGroup>,
    pub modpacks_loading: bool,
    pub modpacks_error: Option<String>,
    pub selection: Selection,
    /// Modpack ids the user pinned to the Favourites group.
    pub favourites: HashSet<String>,
    pub settings: Settings,
    pub login_phase: LoginPhase,
    pub skin_profile: Option<SkinProfile>,
    /// Locally overridden skin model for live preview in the viewer; cleared
    /// after the next successful skin upload sync.
    pub preview_skin_model: Option<SkinModel>,
    /// Raw bytes of a locally selected skin/cape file, used for live preview
    /// in the viewer before the user commits the upload. `Arc` so equality
    /// checks reduce to a pointer compare.
    pub preview_skin_bytes: Option<Arc<Vec<u8>>>,
    pub preview_cape_bytes: Option<Arc<Vec<u8>>>,
    pub pending_login_task: Option<Task<()>>,
    /// Current game install/launch state, driving the launch modal.
    pub launch_state: LaunchState,
    /// Keeps the progress-receiver loop alive while a launch is in flight.
    pub launch_task: Option<Task<()>>,
    /// Modpack ids whose game is currently running (mirrors `game`'s registry).
    pub running: HashSet<String>,
    /// Keeps the running-set watcher loop alive for the app's lifetime.
    pub running_task: Option<Task<()>>,
    pub active_modal: Option<ActiveModal>,
    pub news_modal_body: Option<String>,
    /// Pre-fetched banner bytes keyed by full request URL. When a card or hero
    /// renders, it looks up its URL here and passes the `Arc<Image>` to `img()`
    /// directly — bypasses the network on the hot path. Decode still happens
    /// on first paint, but that's <20ms vs ~200ms for the round trip.
    pub banner_cache: HashMap<String, Arc<Image>>,
    /// Pre-rendered Minecraft head texture keyed by source skin URL.
    pub head_cache: HashMap<String, Arc<Image>>,
}

pub enum AppEvent {
    Routed(Route),
    AuthChanged,
    ModpacksLoaded,
    UserLoaded,
    SkinProfileLoaded,
}

impl EventEmitter<AppEvent> for AppState {}

pub struct AppStateHandle(pub Entity<AppState>);

impl Global for AppStateHandle {}

/// Handle to the main window, set once at startup so non-window contexts (the
/// launch flow, the tray) can minimize/restore it.
pub struct MainWindow(pub gpui::AnyWindowHandle);

impl Global for MainWindow {}

impl AppState {
    pub fn boot(cx: &mut App) -> Entity<Self> {
        let selection: Selection = persistence::load_json("selection.json").unwrap_or_default();
        let favourites: HashSet<String> = persistence::load_json("favourites.json").unwrap_or_default();
        let settings: Settings = persistence::load_json("settings.json").unwrap_or_default();
        let tokens = auth::storage::load();

        let entity = cx.new(|_| AppState {
            route: if tokens.is_some() { Route::Account } else { Route::Login },
            tokens,
            user: None,
            modpacks: Vec::new(),
            groups: Vec::new(),
            modpacks_loading: false,
            modpacks_error: None,
            selection,
            favourites,
            settings,
            login_phase: LoginPhase::Idle,
            skin_profile: None,
            preview_skin_model: None,
            preview_skin_bytes: None,
            preview_cape_bytes: None,
            pending_login_task: None,
            launch_state: LaunchState::Idle,
            launch_task: None,
            running: HashSet::new(),
            running_task: None,
            active_modal: None,
            news_modal_body: None,
            banner_cache: HashMap::new(),
            head_cache: HashMap::new(),
        });

        cx.set_global(AppStateHandle(entity.clone()));

        // Mirror the game registry's running set into state so the UI can show a
        // "stop" button. `game` pings the channel whenever a game starts/exits.
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        crate::game::set_listener(tx);
        let watcher = cx.spawn({
            let entity = entity.clone();
            async move |cx: &mut gpui::AsyncApp| {
                while rx.next().await.is_some() {
                    let ids: HashSet<String> = crate::game::running_ids().into_iter().collect();
                    let _ = entity.update(cx, |state, cx| {
                        if state.running != ids {
                            state.running = ids;
                            cx.notify();
                        }
                    });
                }
            }
        });
        entity.update(cx, |state, _| state.running_task = Some(watcher));

        entity
    }

    pub fn is_running(&self, modpack_id: &str) -> bool {
        self.running.contains(modpack_id)
    }

    /// Stop the running game for a modpack (SIGTERM, ≤30s, SIGKILL). The running
    /// set updates via the game listener once it's torn down.
    pub fn stop_game(&mut self, modpack_id: String, cx: &mut Context<Self>) {
        cx.background_spawn(async move {
            crate::http::on_tokio(crate::game::stop(modpack_id)).await;
        })
        .detach();
    }

    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.route = route.clone();
        cx.emit(AppEvent::Routed(route));
        cx.notify();
    }

    pub fn is_favourite(&self, id: &str) -> bool {
        self.favourites.contains(id)
    }

    /// Pin/unpin a modpack to the Favourites group.
    pub fn toggle_favourite(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.favourites.remove(&id) {
            self.favourites.insert(id);
        }
        let _ = persistence::save_json("favourites.json", &self.favourites);
        cx.notify();
    }

    pub fn select_modpack(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.selection.selected_modpack_id = id;
        let _ = persistence::save_json("selection.json", &self.selection);
        cx.notify();
    }

    pub fn open_modal(&mut self, modal: ActiveModal, cx: &mut Context<Self>) {
        self.active_modal = Some(modal);
        cx.notify();
    }

    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        // Closing the launch modal abandons any in-flight install (the UI stops
        // updating; the download itself keeps running, matching the web cancel).
        if self.active_modal == Some(ActiveModal::Launch) {
            self.launch_task = None;
            self.launch_state = LaunchState::Idle;
        }
        self.active_modal = None;
        cx.notify();
    }

    /// Hide the main window to the tray via the global handle, deferred so it
    /// runs once the current update cycle unwinds.
    pub fn hide_window(&self, cx: &mut Context<Self>) {
        let Some(MainWindow(handle)) = cx.try_global::<MainWindow>() else { return };
        let handle = *handle;
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, app| crate::window_ctl::hide(window, app));
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

        // Worker runs the whole pipeline on the tokio runtime and reports via tx.
        cx.background_spawn(async move {
            crate::http::on_tokio(launch::run(
                tokens,
                modpack_id,
                manifest_url,
                data_dir,
                extra_vars,
                features,
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
                let _ = auth::storage::save(&t);
                self.tokens = Some(t);
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
                // Match the web: a successful launch closes the modal.
                self.active_modal = None;
                cx.notify();
                // Hide the launcher out of the way now the game is starting.
                self.hide_window(cx);
                true
            }
        }
    }

    pub fn open_news(&mut self, body: String, cx: &mut Context<Self>) {
        self.news_modal_body = Some(body);
        self.open_modal(ActiveModal::News, cx);
    }

    pub fn set_data_dir(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.settings.data_dir = path.map(|p| p.trim().to_string()).filter(|s| !s.is_empty());
        let _ = persistence::save_json("settings.json", &self.settings);
        cx.notify();
    }

    /// Toggle whether closing the window hides to the tray or quits the app.
    /// Read by the window's close handler (via the global handle) in main.rs.
    pub fn set_close_to_tray(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.close_to_tray = enabled;
        let _ = persistence::save_json("settings.json", &self.settings);
        cx.notify();
    }

    // ── Per-modpack options ──────────────────────────────────────────────

    /// Saved value for a leaf option (var), if the user set one.
    pub fn option_value(&self, modpack_id: &str, name: &str) -> Option<String> {
        self.settings.modpack_options.get(modpack_id)?.vars.get(name).cloned()
    }

    /// Saved on/off for a feature, if the user toggled it (else use the default).
    pub fn feature_enabled(&self, modpack_id: &str, name: &str) -> Option<bool> {
        self.settings.modpack_options.get(modpack_id)?.features.get(name).copied()
    }

    /// Set (or clear, with `None`) a leaf option's value.
    pub fn set_option_value(&mut self, modpack_id: String, name: String, value: Option<String>, cx: &mut Context<Self>) {
        let entry = self.settings.modpack_options.entry(modpack_id).or_default();
        match value {
            Some(v) => { entry.vars.insert(name, v); }
            None => { entry.vars.remove(&name); }
        }
        let _ = persistence::save_json("settings.json", &self.settings);
        cx.notify();
    }

    /// Toggle a feature on/off for a modpack.
    pub fn set_feature(&mut self, modpack_id: String, name: String, enabled: bool, cx: &mut Context<Self>) {
        self.settings.modpack_options.entry(modpack_id).or_default().features.insert(name, enabled);
        let _ = persistence::save_json("settings.json", &self.settings);
        cx.notify();
    }

    pub fn selected_modpack(&self) -> Option<&Modpack> {
        let id = self.selection.selected_modpack_id.as_ref()?;
        self.modpacks.iter().find(|m| &m.id == id)
    }

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

    pub fn fetch_user(&mut self, cx: &mut Context<Self>) {
        let Some(tokens) = self.tokens.clone() else { return; };
        cx.spawn(async move |this, cx| {
            let result = crate::http::on_tokio(
                with_access_token(tokens, |t| async move { fetch_me(&t).await })
            ).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok((user, refreshed)) => {
                        if let Some(t) = refreshed { state.tokens = Some(t.clone()); let _ = auth::storage::save(&t); }
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

    pub fn fetch_modpacks(&mut self, cx: &mut Context<Self>) {
        if self.modpacks_loading { return; }
        self.modpacks_loading = true;
        self.modpacks_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = crate::http::on_tokio(fetch_all()).await;
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
            let results = crate::http::on_tokio(futures::future::join_all(fetches)).await;
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

    pub fn fetch_skin_profile(&mut self, cx: &mut Context<Self>) {
        let Some(tokens) = self.tokens.clone() else { return; };
        cx.spawn(async move |this, cx| {
            let result = crate::http::on_tokio(
                with_access_token(tokens, |t| async move {
                    fetch_profile(&t).await.map(|opt| opt.unwrap_or_default())
                })
            ).await;
            this.update(cx, |state, cx| {
                match result {
                    Ok((profile, refreshed)) => {
                        if let Some(t) = refreshed { state.tokens = Some(t.clone()); let _ = auth::storage::save(&t); }
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
            let bytes = match crate::http::on_tokio(async move { fetch_image_bytes(&fetch_url).await }).await {
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

    pub fn login(&mut self, provider: auth::Provider, cx: &mut Context<Self>) {
        if matches!(self.login_phase, LoginPhase::Waiting) { return; }
        self.login_phase = LoginPhase::Waiting;
        cx.notify();
        let handle = cx.spawn(async move |this, cx| {
            let result = crate::http::on_tokio(auth::run_login_flow(provider)).await;
            this.update(cx, |state, cx| {
                state.pending_login_task = None;
                match result {
                    Ok(tokens) => {
                        let _ = auth::storage::save(&tokens);
                        state.tokens = Some(tokens);
                        state.login_phase = LoginPhase::Idle;
                        state.set_route(Route::Account, cx);
                        cx.emit(AppEvent::AuthChanged);
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

/// Helper: ensure access token is fresh, run the async closure, return (result, refreshed-tokens-if-any).
async fn with_access_token<T, F, Fut>(mut tokens: Tokens, f: F) -> anyhow::Result<(T, Option<Tokens>)>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let mut refreshed = None;
    if tokens.is_access_expired() {
        if let Some(refresh) = tokens.refresh_token.clone() {
            let new = auth::coordinated_refresh(&refresh).await?;
            tokens = new.clone();
            refreshed = Some(new);
        }
    }
    let value = match f(tokens.access_token.clone()).await {
        Ok(v) => v,
        Err(e) => {
            // Try one refresh on failure, in case the server invalidated mid-call.
            if let Some(refresh) = tokens.refresh_token.clone() {
                let new = auth::coordinated_refresh(&refresh).await?;
                let v = f(new.access_token.clone()).await?;
                refreshed = Some(new);
                v
            } else {
                return Err(e);
            }
        }
    };
    Ok((value, refreshed))
}

/// True when an error from `with_access_token` means the session can't be
/// salvaged: either the refresh-token endpoint rejected us, or a protected
/// endpoint returned 401/403 even with a fresh access token.
fn is_auth_dead(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.starts_with("refresh:") || msg.contains("401") || msg.contains("403")
}

async fn fetch_image_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = crate::http::client()
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec();
    Ok(bytes)
}

fn guess_format(url: &str) -> ImageFormat {
    let lower = url.to_lowercase();
    if lower.contains("webp") { ImageFormat::Webp }
    else if lower.contains(".png") { ImageFormat::Png }
    else if lower.contains(".gif") { ImageFormat::Gif }
    else { ImageFormat::Jpeg }
}
