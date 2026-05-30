mod app;
mod assets;
mod banner;
mod desktop_integration;
mod game;
mod gpui_http;
mod logo;
mod services;
mod single_instance;
mod state;
mod theme;
mod tray;
mod update;
mod window_ctl;
mod views;
mod widgets;

use std::borrow::Cow;
use std::sync::Arc;

use futures::StreamExt;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::app::Root;
use crate::state::{AppState, AppStateHandle, MainWindow};
use crate::tray::TrayCmd;

fn main() {
    init_logging();

    // Single instance: a second launch just exits (the guard only does mutual
    // exclusion — it can't focus the running window). Held for the process's
    // lifetime; dropping/exiting releases the OS lock.
    let _instance = match single_instance::acquire() {
        single_instance::Instance::AlreadyRunning => {
            tracing::info!("another instance is already running; exiting");
            return;
        }
        single_instance::Instance::Primary(guard) => guard,
    };

    Application::new().with_assets(assets::Assets).run(move |cx: &mut App| {
        // Plug reqwest into GPUI so img(url) loads remote textures.
        cx.set_http_client(Arc::new(gpui_http::GpuiHttpClient::new()));

        // Bundle Roboto so the UI looks the same on machines without it installed.
        let fonts: Vec<Cow<'static, [u8]>> = vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Regular.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Medium.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Bold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/Roboto-Italic.ttf")),
        ];
        if let Err(e) = cx.text_system().add_fonts(fonts) {
            tracing::warn!("failed to load bundled Roboto fonts: {e}");
        }

        let state = AppState::boot(cx);

        // Install/refresh the .desktop entry + icons so the app shows our logo.
        desktop_integration::ensure();

        // Self-update check, fire-and-forget on a blocking thread (self_update is sync).
        std::thread::spawn(|| {
            if let Err(e) = update::run_blocking() {
                tracing::warn!("self-update: {e}");
            }
        });

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
                            crate::window_ctl::hide(window, cx);
                        } else {
                            begin_quit(cx);
                        }
                        // Either way we never let GPUI close the window itself:
                        // closing the last window stops its loop, which would
                        // skip the graceful game shutdown that `begin_quit` runs.
                        false
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

        // Backstop: if the app quits some other way than tray Quit, at least
        // SIGTERM the game (the ~100ms quit budget rules out a full graceful wait).
        cx.on_app_quit(|_cx| {
            crate::game::sigterm_all();
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
                        crate::window_ctl::show(window, app)
                    });
                }
                TrayCmd::Toggle => {
                    let _ = cx.update_window(window, |_, window, app| {
                        crate::window_ctl::toggle(window, app)
                    });
                }
                TrayCmd::Quit => graceful_quit().await,
            }
        }
    })
    .detach();
}

/// Stop every running game gracefully (SIGTERM, ≤30s, SIGKILL), then exit. We
/// exit the process directly rather than via `cx.quit()`: GPUI's Wayland loop
/// doesn't reliably wake on `cx.quit()` from an idle, D-Bus-driven task. The
/// `-> !` return lets callers use it as a terminal expression.
async fn graceful_quit() -> ! {
    harmoniya_api::http::on_tokio(crate::game::stop_all()).await;
    std::process::exit(0);
}

/// Fire-and-forget [`graceful_quit`] from a synchronous context (the window
/// close handler). The window stays up until the process exits.
fn begin_quit(cx: &mut App) {
    cx.spawn(async move |_cx: &mut gpui::AsyncApp| graceful_quit().await).detach();
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,harmoniya_launcher=debug"));
    let console = fmt::layer().with_target(false);
    let registry = tracing_subscriber::registry().with(filter).with(console);

    if let Ok(dir) = harmoniya_api::config::logs_dir() {
        let appender = tracing_appender::rolling::daily(dir, "harmoniya.log");
        let file_layer = fmt::layer().with_target(true).with_ansi(false).with_writer(appender);
        let _ = registry.with(file_layer).try_init();
    } else {
        let _ = registry.try_init();
    }
}
