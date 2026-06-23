//! System tray icon. Lets the user toggle the launcher window (it hides to the
//! tray on close or game launch) or quit.
//!
//! - **Linux:** StatusNotifierItem over D-Bus via `ksni` (no GTK loop, so it
//!   doesn't fight GPUI's Linux event loop).
//! - **macOS/Windows:** `tray-icon`, created on the main thread where GPUI pumps
//!   the native run loop, so its menu/click events fire.
//! - Other platforms: no-op.
//!
//! The menu's toggle label tracks [`crate::shell::window_ctl::is_visible`]; call
//! [`refresh`] after the window's visibility changes to re-render it.

use std::sync::atomic::{AtomicBool, Ordering};

use futures::channel::mpsc::UnboundedSender;

/// Set once a tray icon has successfully registered with the desktop's tray
/// host. Stays `false` when there's no host to register with — e.g. a bare
/// wlroots session with no StatusNotifierWatcher (`ServiceUnknown: The name is
/// not activatable`). Hiding the window to a tray that isn't there would strand
/// it with no way back, so the window controller consults [`is_available`] and
/// falls back to quitting (on close) or staying visible (after a launch).
static TRAY_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Whether a tray icon is actually present to restore the window from. `false`
/// until registration succeeds, and never flips true when there's no tray host.
/// On Linux this is set from the tray's background thread a beat after startup;
/// it's reliably settled by the time any user-driven close/hide can run.
pub fn is_available() -> bool {
    TRAY_AVAILABLE.load(Ordering::Relaxed)
}

fn mark_available() {
    TRAY_AVAILABLE.store(true, Ordering::Relaxed);
}

/// Commands the tray pushes back to the GPUI foreground loop.
#[derive(Clone, Copy, Debug)]
pub enum TrayCmd {
    /// Restore + focus the main window (icon left-click).
    Show,
    /// Flip between hidden and shown (menu toggle item).
    Toggle,
    /// Quit the application.
    Quit,
}

/// Label for the show/hide toggle item, given current visibility.
fn toggle_label(visible: bool) -> &'static str {
    let t = crate::i18n::t();
    if visible { t.tray_hide } else { t.tray_show }
}

/// Re-render the tray menu so the labels match current visibility and language.
pub fn refresh() {
    #[cfg(target_os = "linux")]
    linux::refresh();
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    desktop::refresh();
}

/// Cropped Harmoniya logo as straight-alpha RGBA8 bytes at `size`×`size`.
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn render_rgba(size: u32) -> Vec<u8> {
    crate::logo::rgba(size)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::sync::OnceLock;

    // Use the blocking + async-io backend, not ksni's default tokio backend:
    // the tokio backend panics in zbus (`Handle::current`) when a menu item is
    // clicked from a thread that isn't inside a tokio runtime.
    use ksni::blocking::{Handle, TrayMethods};
    use ksni::{Icon, MenuItem, Tray, menu::StandardItem};

    use super::{TrayCmd, UnboundedSender, render_rgba, toggle_label};

    /// Stored so [`refresh`] can ask ksni to re-render the (cached) menu.
    static HANDLE: OnceLock<Handle<LauncherTray>> = OnceLock::new();

    /// ksni wants ARGB32 in network byte order; convert from our RGBA8.
    fn icon_at(size: i32) -> Icon {
        let rgba = render_rgba(size as u32);
        let mut data = Vec::with_capacity(rgba.len());
        for px in rgba.chunks_exact(4) {
            data.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
        }
        Icon { width: size, height: size, data }
    }

    struct LauncherTray {
        tx: UnboundedSender<TrayCmd>,
    }

    impl LauncherTray {
        fn send(&self, cmd: TrayCmd) {
            let _ = self.tx.unbounded_send(cmd);
        }
    }

    impl Tray for LauncherTray {
        fn id(&self) -> String {
            "net.harmoniya.launcher".into()
        }

        fn title(&self) -> String {
            "Harmoniya".into()
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            vec![icon_at(32), icon_at(64)]
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            self.send(TrayCmd::Show);
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                StandardItem {
                    label: toggle_label(crate::shell::window_ctl::is_visible()).into(),
                    activate: Box::new(|t: &mut Self| t.send(TrayCmd::Toggle)),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: crate::i18n::t().tray_quit.into(),
                    activate: Box::new(|t: &mut Self| t.send(TrayCmd::Quit)),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    pub fn spawn(tx: UnboundedSender<TrayCmd>) {
        // `spawn` blocks briefly to set up the D-Bus connection, then the service
        // runs on ksni's own thread — so do the setup off the UI thread. The
        // stored Handle keeps the service alive after this thread exits.
        std::thread::spawn(move || match (LauncherTray { tx }).spawn() {
            Ok(handle) => {
                let _ = HANDLE.set(handle);
                super::mark_available();
            }
            Err(e) => tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "tray unavailable"),
        });
    }

    pub fn refresh() {
        let Some(handle) = HANDLE.get() else { return };
        let handle = handle.clone();
        // `update` re-renders the cached menu (reading is_visible() for the label).
        // It blocks on a D-Bus round-trip, so run it off the UI thread.
        std::thread::spawn(move || {
            handle.update(|_| {});
        });
    }
}

#[cfg(target_os = "linux")]
pub use linux::spawn;

#[cfg(any(target_os = "windows", target_os = "macos"))]
mod desktop {
    use std::cell::RefCell;

    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    };

    use super::{TrayCmd, UnboundedSender, render_rgba, toggle_label};

    thread_local! {
        // TrayIcon is !Send and must outlive `spawn`; park it on the main thread.
        // The toggle/quit MenuItems are kept so `refresh` can relabel them.
        static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
        static TOGGLE: RefCell<Option<MenuItem>> = const { RefCell::new(None) };
        static QUIT: RefCell<Option<MenuItem>> = const { RefCell::new(None) };
    }

    pub fn spawn(tx: UnboundedSender<TrayCmd>) {
        let menu = Menu::new();
        let toggle = MenuItem::new(toggle_label(crate::shell::window_ctl::is_visible()), true, None);
        let quit = MenuItem::new(crate::i18n::t().tray_quit, true, None);
        if let Err(e) = menu
            .append(&toggle)
            .and_then(|_| menu.append(&PredefinedMenuItem::separator()))
            .and_then(|_| menu.append(&quit))
        {
            tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "tray menu");
            return;
        }
        let toggle_id = toggle.id().clone();
        let quit_id = quit.id().clone();

        let icon = match Icon::from_rgba(render_rgba(64), 64, 64) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "tray icon");
                return;
            }
        };

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Harmoniya")
            .with_icon(icon)
            .build();
        match tray {
            Ok(t) => {
                TRAY.with(|c| *c.borrow_mut() = Some(t));
                TOGGLE.with(|c| *c.borrow_mut() = Some(toggle));
                QUIT.with(|c| *c.borrow_mut() = Some(quit));
                super::mark_available();
            }
            Err(e) => {
                tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "tray unavailable");
                return;
            }
        }

        // Menu clicks → Toggle/Quit. Handlers fire during the native run loop.
        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if e.id == toggle_id {
                let _ = menu_tx.unbounded_send(TrayCmd::Toggle);
            } else if e.id == quit_id {
                let _ = menu_tx.unbounded_send(TrayCmd::Quit);
            }
        }));

        // Left-click on the icon → Show.
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = e
            {
                let _ = tx.unbounded_send(TrayCmd::Show);
            }
        }));
    }

    pub fn refresh() {
        // Runs on the main thread (tray command loop / deferred app closures).
        let label = toggle_label(crate::shell::window_ctl::is_visible());
        TOGGLE.with(|c| {
            if let Some(item) = c.borrow().as_ref() {
                item.set_text(label);
            }
        });
        QUIT.with(|c| {
            if let Some(item) = c.borrow().as_ref() {
                item.set_text(crate::i18n::t().tray_quit);
            }
        });
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub use desktop::spawn;

#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
pub fn spawn(_tx: UnboundedSender<TrayCmd>) {
    // No tray on this platform.
}
