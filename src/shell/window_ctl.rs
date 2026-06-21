//! Cross-platform show/hide for the main window, plus the shared visibility
//! state the tray's toggle label reflects. With a tray present, "hide" means
//! hide-to-tray (gone from the taskbar/dock), not minimize.
//!
//! GPUI has no portable hide, and the per-platform primitives differ:
//! - **Windows:** `App::hide()` is a no-op and `minimize()` only parks the
//!   window in the taskbar, so we go to the raw `HWND` and `ShowWindow(SW_HIDE)`.
//! - **macOS:** `App::hide()` hides the app (status item stays in the menu bar);
//!   `App::activate(true)` unhides + focuses it.
//! - **Linux (Wayland):** no protocol lets a client hide its own toplevel, and
//!   closing the window would quit GPUI (it stops the loop on the last window).
//!   `minimize` is unreliable — wlroots compositors (e.g. Hyprland) ignore it —
//!   so we instead unmap the surface (attach a null buffer) via a vendored-gpui
//!   `set_window_hidden`, which hides the window on every compositor and re-maps
//!   on show. See `vendor/gpui/HARMONIYA_PATCH.md`.

use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{App, Window};

/// Whether the main window is currently shown. Read by the tray to label its
/// toggle item. Starts `true` (window opens visible).
static WINDOW_VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn is_visible() -> bool {
    WINDOW_VISIBLE.load(Ordering::Relaxed)
}

fn set_visible_state(visible: bool) {
    WINDOW_VISIBLE.store(visible, Ordering::Relaxed);
    // Keep the tray's show/hide toggle label in sync.
    crate::shell::tray::refresh();
}

/// Hide the window out of sight; restored via the tray.
pub fn hide(window: &mut Window, cx: &mut App) {
    platform_hide(window, cx);
    set_visible_state(false);
}

/// Bring the window back and focus it.
pub fn show(window: &mut Window, cx: &mut App) {
    platform_show(window, cx);
    set_visible_state(true);
}

/// Flip between hidden and shown.
pub fn toggle(window: &mut Window, cx: &mut App) {
    if is_visible() {
        hide(window, cx);
    } else {
        show(window, cx);
    }
}

#[allow(unused_variables)]
fn platform_hide(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "windows")]
    set_visible_win32(window, false);
    #[cfg(target_os = "macos")]
    cx.hide();
    #[cfg(target_os = "linux")]
    linux::hide(window);
}

#[allow(unused_variables)]
fn platform_show(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "windows")]
    set_visible_win32(window, true);
    #[cfg(target_os = "macos")]
    cx.activate(true);
    #[cfg(target_os = "linux")]
    linux::show(window);
}

#[cfg(target_os = "windows")]
fn set_visible_win32(window: &Window, visible: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };

    // Call the `HasWindowHandle` trait method explicitly: GPUI's inherent
    // `Window::window_handle()` shadows it and returns an opaque
    // `AnyWindowHandle`, not the raw handle we need for the `HWND`.
    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    if let RawWindowHandle::Win32(w) = handle.as_raw() {
        let hwnd = w.hwnd.get() as *mut core::ffi::c_void;
        unsafe {
            ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
            if visible {
                SetForegroundWindow(hwnd);
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use gpui::Window;

    // Unmap the Wayland surface rather than minimize: wlroots compositors (e.g.
    // Hyprland) silently ignore `set_minimized`, so the old minimize never hid
    // the window there. Unmapping makes it vanish on every compositor. Show
    // re-maps and then focuses it.
    pub fn hide(window: &mut Window) {
        window.set_window_hidden(true);
    }

    pub fn show(window: &mut Window) {
        window.set_window_hidden(false);
        window.activate_window();
    }
}
