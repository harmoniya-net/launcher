//! The single application model. `AppState` is one GPUI `Entity` that the whole
//! UI observes; its behavior is split across this module by concern:
//! - [`session`] — login/logout, token refresh, the current user.
//! - [`catalog`] — modpack list, groups, favourites, banner prefetch, options.
//! - [`launch_flow`] — the play/install/launch flow and window hide-to-tray.
//! - [`skin`] — skin profile, live preview, head-avatar prefetch.
//! - [`ui`] — routing, modals, and launcher settings.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use gpui::{App, AppContext, Entity, EventEmitter, Global, Image, ImageFormat, Task};
use serde::{Deserialize, Serialize};

use harmoniya_api::auth::{self, tokens::Tokens};
use harmoniya_api::config;
use harmoniya_api::services::{
    account::User,
    lucky::LuckyProfile,
    modpacks::{Modpack, ProjectGroup},
    options::ModpackOptions,
    yggdrasil::{SkinModel, SkinProfile},
};
use harmoniya_launch::pipeline::{CancellationToken, LaunchState};

mod catalog;
mod launch_flow;
mod lucky;
mod session;
mod skin;
mod ui;
mod update;

#[derive(Clone, Debug, Default)]
pub enum Route {
    #[default]
    Login,
    Account,
    Skin { tab: SkinTab },
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

/// Auto-update progress. While set, `Root` renders a full-window updating screen
/// in place of the normal UI as a new release downloads and the app relaunches.
#[derive(Clone, Debug)]
pub enum UpdatePhase {
    /// A newer release was found and is downloading (carries its version).
    Downloading(String),
    /// Download finished; the app is about to relaunch into the new binary.
    Restarting,
}

pub struct AppState {
    pub route: Route,
    /// When set, the auto-updater has taken over the window (see [`UpdatePhase`]).
    pub update: Option<UpdatePhase>,
    pub tokens: Option<Tokens>,
    pub user: Option<User>,
    pub modpacks: Vec<Modpack>,
    pub groups: Vec<ProjectGroup>,
    pub modpacks_loading: bool,
    pub modpacks_error: Option<String>,
    /// When the catalog was last (re)fetched, to rate-limit background refreshes.
    pub last_catalog_fetch: Option<Instant>,
    pub selection: Selection,
    /// Modpack ids the user pinned to the Favourites group.
    pub favourites: HashSet<String>,
    pub settings: Settings,
    pub login_phase: LoginPhase,
    pub lucky_profile: Option<LuckyProfile>,
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
    /// Cancels the in-flight install when the launch modal is closed.
    pub launch_cancel: Option<CancellationToken>,
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
    /// Pre-fetched + Lanczos3-resized project logo textures keyed by URL.
    /// GPUI's nearest-neighbour downscale makes the raw CDN images look pixelated
    /// at 20 px; pre-scaling to 40×40 (2× for HiDPI) via the `image` crate fixes that.
    pub logo_cache: HashMap<String, Arc<Image>>,
}

pub enum AppEvent {
    Routed,
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
        let selection: Selection = config::load_json(config::SELECTION_FILE).unwrap_or_default();
        let favourites: HashSet<String> = config::load_json(config::FAVOURITES_FILE).unwrap_or_default();
        let settings: Settings = config::load_json(config::SETTINGS_FILE).unwrap_or_default();
        let tokens = auth::storage::load();

        let entity = cx.new(|_| AppState {
            route: if tokens.is_some() { Route::Account } else { Route::Login },
            update: None,
            tokens,
            user: None,
            modpacks: Vec::new(),
            groups: Vec::new(),
            modpacks_loading: false,
            modpacks_error: None,
            last_catalog_fetch: None,
            selection,
            favourites,
            settings,
            login_phase: LoginPhase::Idle,
            lucky_profile: None,
            skin_profile: None,
            preview_skin_model: None,
            preview_skin_bytes: None,
            preview_cape_bytes: None,
            pending_login_task: None,
            launch_state: LaunchState::Idle,
            launch_task: None,
            launch_cancel: None,
            running: HashSet::new(),
            running_task: None,
            active_modal: None,
            news_modal_body: None,
            banner_cache: HashMap::new(),
            head_cache: HashMap::new(),
            logo_cache: HashMap::new(),
        });

        cx.set_global(AppStateHandle(entity.clone()));

        // Mirror the game registry's running set into state so the UI can show a
        // "stop" button. `game` pings the channel whenever a game starts/exits.
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<()>();
        harmoniya_launch::game::set_listener(tx);
        let watcher = cx.spawn({
            let entity = entity.clone();
            async move |cx: &mut gpui::AsyncApp| {
                while rx.next().await.is_some() {
                    let ids: HashSet<String> = harmoniya_launch::game::running_ids().into_iter().collect();
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

    pub fn selected_modpack(&self) -> Option<&Modpack> {
        let id = self.selection.selected_modpack_id.as_ref()?;
        self.modpacks.iter().find(|m| &m.id == id)
    }

    /// Adopt + persist rotated tokens returned by a refresh, if any. The single
    /// home for the "save the new tokens" tail every auth'd flow shares.
    pub(crate) fn adopt_tokens(&mut self, refreshed: Option<Tokens>) {
        if let Some(t) = refreshed {
            let _ = auth::storage::save(&t);
            self.tokens = Some(t);
        }
    }
}

// ── Shared helpers used across the concern submodules ───────────────────────

/// Ensure the access token is fresh, run the async closure, and return its
/// result alongside any rotated tokens the caller should persist.
async fn with_access_token<T, F, Fut>(tokens: Tokens, f: F) -> anyhow::Result<(T, Option<Tokens>)>
where
    F: Fn(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    // Refresh up front if stale (shared with launch/skin via `auth::ensure_fresh`).
    let (tokens, mut refreshed) = auth::ensure_fresh(tokens).await?;
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

/// True when an error from [`with_access_token`] means the session can't be
/// salvaged: either the refresh-token endpoint rejected us, or a protected
/// endpoint returned 401/403 even with a fresh access token.
fn is_auth_dead(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.starts_with("refresh:") || msg.contains("401") || msg.contains("403")
}

async fn fetch_image_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let bytes = harmoniya_api::http::client()
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
