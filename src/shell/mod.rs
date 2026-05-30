//! OS-facing desktop integration that the app drives directly: window
//! show/hide, the system tray, the single-instance guard, `.desktop` entry
//! installation, and the self-updater. App-specific (hardcoded app id, logo,
//! labels), so it lives in the binary rather than a library crate.

pub mod desktop_integration;
pub mod single_instance;
pub mod tray;
pub mod update;
pub mod window_ctl;
