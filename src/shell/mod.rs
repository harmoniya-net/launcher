//! OS-facing desktop integration that the app drives directly: window
//! show/hide, the system tray, the single-instance guard, and the
//! self-updater. (The Linux `.desktop` entry and icons come from the install
//! script, not the app.) App-specific (hardcoded app id, logo,
//! labels), so it lives in the binary rather than a library crate.

pub mod discord;
pub mod single_instance;
pub mod tray;
pub mod update;
pub mod window_ctl;
