// Don't pop a console window on Windows for the shipped (release) GUI build.
// Kept in debug so `tracing` console output is still visible during dev.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod banner;
mod gpui_http;
mod logo;
mod shell;
mod state;
mod theme;
mod views;
mod widgets;

use std::borrow::Cow;
use std::sync::Arc;

use futures::StreamExt;
use gpui::{px, size, App, AppContext, Application, Bounds, KeyBinding, WindowBounds, WindowOptions};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::app::Root;
use crate::shell::tray::TrayCmd;
use crate::shell::{desktop_integration, single_instance, tray};
use crate::state::{AppState, AppStateHandle, MainWindow};

// Ctrl/Cmd+R — manually refresh the catalog from the CMS.
gpui::actions!(harmoniya, [RefreshCatalog]);

fn main() {
    init_logging();

    // Single instance: a second launch just exits (the guard only does mutual
    // exclusion — it can't focus the running window). Handed to `hold` so it
    // lives for the process's lifetime; the updater releases it before re-exec.
    let instance = match single_instance::acquire() {
        single_instance::Instance::AlreadyRunning => {
            tracing::info!("another instance is already running; focusing it and exiting");
            single_instance::signal_focus();
            return;
        }
        single_instance::Instance::Primary(guard) => guard,
    };
    single_instance::hold(instance);

    Application::new()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
            // Plug reqwest into GPUI so img(url) loads remote textures.
            cx.set_http_client(Arc::new(gpui_http::GpuiHttpClient::new()));

            // Bundle Inter — the only font family the UI uses, so it renders
            // identically everywhere without depending on installed system fonts.
            let fonts: Vec<Cow<'static, [u8]>> = vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Medium.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Bold.ttf")),
            ];
            if let Err(e) = cx.text_system().add_fonts(fonts) {
                tracing::warn!("failed to load bundled Inter fonts: {e}");
            }

            let state = AppState::boot(cx);

            // Ctrl/Cmd+R refreshes the catalog from the CMS (forces a fetch,
            // bypassing the focus/select cooldown).
            cx.bind_keys([
                KeyBinding::new("ctrl-r", RefreshCatalog, None),
                KeyBinding::new("cmd-r", RefreshCatalog, None),
            ]);
            cx.on_action(|_: &RefreshCatalog, cx: &mut App| {
                let state = cx.global::<AppStateHandle>().0.clone();
                state.update(cx, |s, cx| s.reload_modpacks(cx));
            });

            // Install/refresh the .desktop entry + icons so the app shows our logo.
            desktop_integration::ensure();

            // Auto-update: check for a newer release; if one exists, take over the
            // window with an updating screen, download it, and relaunch.
            state.update(cx, |s, cx| s.check_for_update(cx));

            let bounds = Bounds::centered(None, size(px(1200.), px(760.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(gpui::TitlebarOptions {
                            title: Some("Harmoniya".into()),
                            ..Default::default()
                        }),
                        app_id: Some("net.harmoniya.launcher".into()),
                        // Below this the two-pane layout (448px sidebar + content)
                        // starts to crowd; keep the window usable.
                        window_min_size: Some(size(px(1280.), px(720.))),
                        ..Default::default()
                    },
                    move |window, cx| {
                        // Closing the window hides it to the tray when "close to
                        // tray" is on (the default), otherwise it quits the app.
                        window.on_window_should_close(cx, |window, cx| {
                            let close_to_tray = cx
                                .try_global::<AppStateHandle>()
                                .map(|h| h.0.read(cx).settings.close_to_tray)
                                .unwrap_or(true);
                            if close_to_tray {
                                crate::shell::window_ctl::hide(window, cx);
                            } else {
                                begin_quit(cx);
                            }
                            // Either way we never let GPUI close the window itself:
                            // closing the last window stops its loop, which would
                            // skip the graceful game shutdown that `begin_quit` runs.
                            false
                        });
                        // Pull fresh CMS data whenever the window regains focus —
                        // restored from the tray, alt-tabbed back, etc. (Rate-limited
                        // and a no-op until the catalog has first loaded.)
                        state.update(cx, |_, cx| {
                            cx.observe_window_activation(window, |s, window, cx| {
                                if window.is_window_active() {
                                    s.refresh_modpacks(cx);
                                }
                            })
                            .detach();
                        });
                        cx.new(|cx| Root::new(state, cx))
                    },
                )
                .expect("open window");
            cx.activate(true);

            // Stash the window handle so the launch flow and tray can hide/restore it.
            let handle = window.into();
            cx.set_global(MainWindow(handle));

            // System tray: toggle/restore the window (it hides on close or launch), or quit.
            start_tray(handle, cx);

            // Bring this window to the front when a duplicate launch pokes us.
            start_focus_listener(handle, cx);

            // Backstop: if the app quits some other way than tray Quit, at least
            // SIGTERM the game (the ~100ms quit budget rules out a full graceful wait).
            cx.on_app_quit(|_cx| {
                harmoniya_launch::game::sigterm_all();
                async {}
            })
            .detach();
        });
}

/// Spin up the tray and forward its commands onto the GPUI foreground loop.
fn start_tray(window: gpui::AnyWindowHandle, cx: &mut App) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<TrayCmd>();
    tray::spawn(tx.clone());
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        while let Some(cmd) = rx.next().await {
            match cmd {
                TrayCmd::Show => {
                    let _ = cx.update_window(window, |_, window, app| {
                        crate::shell::window_ctl::show(window, app)
                    });
                }
                TrayCmd::Toggle => {
                    let _ = cx.update_window(window, |_, window, app| {
                        crate::shell::window_ctl::toggle(window, app)
                    });
                }
                TrayCmd::Quit => graceful_quit().await,
            }
        }
    })
    .detach();
}

/// Listen for "focus me" pokes from duplicate launches and raise the window.
fn start_focus_listener(window: gpui::AnyWindowHandle, cx: &mut App) {
    let Some(mut rx) = single_instance::serve_focus() else { return };
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        while rx.next().await.is_some() {
            let _ = cx.update_window(window, |_, window, app| {
                crate::shell::window_ctl::show(window, app)
            });
        }
    })
    .detach();
}

/// Stop every running game gracefully (SIGTERM, ≤30s, SIGKILL), then exit. We
/// exit the process directly rather than via `cx.quit()`: GPUI's Wayland loop
/// doesn't reliably wake on `cx.quit()` from an idle, D-Bus-driven task. The
/// `-> !` return lets callers use it as a terminal expression.
async fn graceful_quit() -> ! {
    harmoniya_api::http::on_tokio(harmoniya_launch::game::stop_all()).await;
    std::process::exit(0);
}

/// Fire-and-forget [`graceful_quit`] from a synchronous context (the window
/// close handler). The window stays up until the process exits.
fn begin_quit(cx: &mut App) {
    cx.spawn(async move |_cx: &mut gpui::AsyncApp| graceful_quit().await)
        .detach();
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,harmoniya_launcher=debug"));
    let console = fmt::layer().with_target(false);
    let registry = tracing_subscriber::registry().with(filter).with(console);

    if let Ok(dir) = harmoniya_api::config::logs_dir() {
        let appender = tracing_appender::rolling::daily(dir, "harmoniya.log");
        let file_layer = fmt::layer()
            .with_target(true)
            .with_ansi(false)
            .with_writer(appender);
        let _ = registry.with(file_layer).try_init();
    } else {
        let _ = registry.try_init();
    }
}
