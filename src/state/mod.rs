//! The single application model. `AppState` is one GPUI `Entity` that the whole
//! UI observes; its behavior is split across this module by concern:
//! - [`session`] — sign-in/sign-out and the current user.
//! - [`catalog`] — modpack list, groups, favourites, banner prefetch, bundle
//!   heads (each modpack's options) and the player's option choices.
//! - [`launch_flow`] — the play/install/launch flow and window hide-to-tray.
//! - [`skin`] — skin profile, live preview, head-avatar prefetch.
//! - [`ui`] — routing, modals, and launcher settings.
//! - `discord` — derives the Discord Rich Presence activity shown for the
//!   current state; wired up as a global observer in [`AppState::boot`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use gpui::{App, AppContext, Entity, EventEmitter, Global, Image, ImageFormat, Task};
use serde::{Deserialize, Serialize};

use harmoniya_api::auth::{self, SessionStore};
use harmoniya_api::config;
use harmoniya_api::services::{
    account::User,
    lucky::LuckyProfile,
    modpacks::{Modpack, ProjectGroup},
    yggdrasil::{SkinModel, SkinProfile},
};
use harmoniya_launch::options::{Head, ModpackOptions};
use harmoniya_launch::pipeline::{CancellationToken, LaunchState};

mod catalog;
mod discord;
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
    /// Per-modpack option choices (vars + enabled features), keyed by modpack
    /// id and, within one, by option name. The options themselves are the
    /// bundle's (see [`BundleHead`]).
    #[serde(default)]
    pub modpack_options: HashMap<String, ModpackOptions>,
    /// Closing the window hides to the tray (default) instead of quitting.
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    /// UI language; Ukrainian unless the user switched to English.
    #[serde(default)]
    pub language: crate::i18n::Lang,
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
            language: crate::i18n::Lang::default(),
        }
    }
}

/// A modpack bundle's head as the settings form last read it. The head is
/// where a bundle says what a player may set, so the form is drawn from it. A
/// refresh keeps the last head that was read until a new one arrives.
#[derive(Clone, Debug, Default)]
pub struct BundleHead {
    pub head: Option<Arc<Head>>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum LoginPhase {
    Idle,
    /// Asking the account service for a sign-in code.
    Starting,
    /// Waiting for the user to approve `user_code` on the page at `url`.
    AwaitingApproval { user_code: String, url: String },
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
    /// When the last update check started; throttles checks on tray restore.
    pub last_update_check: Option<Instant>,
    /// The sole owner of the session token; see [`SessionStore`].
    pub session: Arc<SessionStore>,
    pub user: Option<User>,
    pub modpacks: Vec<Modpack>,
    pub groups: Vec<ProjectGroup>,
    pub modpacks_loading: bool,
    pub modpacks_error: Option<String>,
    /// Bundle heads keyed by bundle URL, read when a modpack's settings open.
    pub bundle_heads: HashMap<String, BundleHead>,
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
    /// The in-flight sign-in. Dropping it cancels the sign-in.
    pub pending_login_task: Option<Task<()>>,
    /// Current game install/launch state, driving the launch modal.
    pub launch_state: LaunchState,
    /// Keeps the progress-receiver loop alive while a launch is in flight.
    pub launch_task: Option<Task<()>>,
    /// Cancels the in-flight install when the launch modal is closed.
    pub launch_cancel: Option<CancellationToken>,
    /// Modpack ids whose game is currently running (mirrors `game`'s registry).
    pub running: HashSet<String>,
    /// When each currently-running modpack started (unix ms), so the Discord
    /// activity can show elapsed playtime. Entries are dropped once the game
    /// exits.
    pub running_since: HashMap<String, i64>,
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
        // Activate the persisted language before any view (or the tray) renders.
        crate::i18n::set(settings.language);
        let session = auth::SessionStore::load();

        let entity = cx.new(|_| AppState {
            route: if session.signed_in() { Route::Account } else { Route::Login },
            update: None,
            last_update_check: None,
            session,
            user: None,
            modpacks: Vec::new(),
            groups: Vec::new(),
            modpacks_loading: false,
            modpacks_error: None,
            bundle_heads: HashMap::new(),
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
            running_since: HashMap::new(),
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
                            let now = harmoniya_api::now_ms() as i64;
                            for id in &ids {
                                state.running_since.entry(id.clone()).or_insert(now);
                            }
                            state.running_since.retain(|id, _| ids.contains(id));
                            state.running = ids;
                            cx.notify();
                        }
                    });
                }
            }
        });
        entity.update(cx, |state, _| state.running_task = Some(watcher));

        // Discord Rich Presence: best-effort, so it lives entirely off the
        // GPUI thread (see `shell::discord`). Re-derive it from state on every
        // notification — cheap, and `discord::set` dedupes unchanged values
        // before it ever touches the IPC thread.
        crate::shell::discord::init();
        cx.observe(&entity, |entity, cx| {
            crate::shell::discord::set(entity.read(cx).discord_presence());
        })
        .detach();
        crate::shell::discord::set(entity.read(cx).discord_presence());

        entity
    }

    pub fn is_running(&self, modpack_id: &str) -> bool {
        self.running.contains(modpack_id)
    }

    pub fn selected_modpack(&self) -> Option<&Modpack> {
        let id = self.selection.selected_modpack_id.as_ref()?;
        self.modpacks.iter().find(|m| &m.id == id)
    }

}

// ── Shared helpers used across the concern submodules ───────────────────────

/// Run an authenticated request with the session token. Fails with an `auth:`
/// error when signed out.
async fn with_token<T, F, Fut>(store: Arc<SessionStore>, f: F) -> anyhow::Result<T>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    f(store.token()?).await
}

/// True when an error from [`with_token`] means the session is gone: we're
/// signed out, or the account service rejected the token (expired or revoked —
/// there is no refresh, so the user has to sign in again).
fn is_auth_dead(err: &anyhow::Error) -> bool {
    let msg = err.to_string();
    msg.starts_with("auth:") || msg.contains("401") || msg.contains("403")
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
