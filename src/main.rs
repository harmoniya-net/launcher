mod app;
mod assets;
mod auth;
mod banner;
mod desktop_integration;
mod game;
mod gpui_http;
mod http;
mod logo;
mod persistence;
mod services;
mod single_instance;
mod skin_head;
mod skin_viewer;
mod state;
mod theme;
mod tray;
mod update;
mod window_ctl;
mod views;
mod widgets;

use std::borrow::Cow;
use std::net::TcpListener;
use std::sync::Arc;

use futures::StreamExt;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::app::Root;
use crate::state::{AppState, MainWindow};
use crate::tray::TrayCmd;

fn main() {
    init_logging();

    // Single instance: a second launch focuses the running window and exits.
    let listener = match single_instance::acquire() {
        single_instance::Instance::AlreadyRunning => {
            tracing::info!("another instance is already running; focused it");
            return;
        }
        single_instance::Instance::Primary(listener) => listener,
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
                    // Closing the window hides it to the tray instead of quitting.
                    window.on_window_should_close(cx, |window, cx| {
                        crate::window_ctl::hide(window, cx);
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
        // Also wires the single-instance listener so a second launch focuses us.
        start_tray(handle, listener, cx);

        // Backstop: if the app quits some other way than tray Quit, at least
        // SIGTERM the game (the ~100ms quit budget rules out a full graceful wait).
        cx.on_app_quit(|_cx| {
            crate::game::sigterm_all();
            async {}
        })
        .detach();
    });
}

/// Spin up the tray (and the single-instance focus listener) and forward their
/// commands onto the GPUI foreground loop.
fn start_tray(window: gpui::AnyWindowHandle, listener: Option<TcpListener>, cx: &mut App) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<TrayCmd>();
    tray::spawn(tx.clone());
    // A second instance pinging the loopback port asks us to surface the window.
    if let Some(listener) = listener {
        let tx = tx.clone();
        single_instance::serve(listener, move || {
            let _ = tx.unbounded_send(TrayCmd::Show);
        });
    }
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
                TrayCmd::Quit => {
                    // Stop the game gracefully (SIGTERM, ≤30s, SIGKILL) while the
                    // launcher is still alive, then exit. We exit the process
                    // directly: GPUI's Wayland loop doesn't reliably wake on
                    // `cx.quit()` when triggered from an idle, D-Bus-driven task.
                    crate::http::on_tokio(crate::game::stop_all()).await;
                    std::process::exit(0);
                }
            }
        }
    })
    .detach();
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,harmoniya_launcher=debug"));
    let console = fmt::layer().with_target(false);
    let registry = tracing_subscriber::registry().with(filter).with(console);

    if let Ok(dir) = persistence::logs_dir() {
        let appender = tracing_appender::rolling::daily(dir, "harmoniya.log");
        let file_layer = fmt::layer().with_target(true).with_ansi(false).with_writer(appender);
        let _ = registry.with(file_layer).try_init();
    } else {
        let _ = registry.try_init();
    }
}
