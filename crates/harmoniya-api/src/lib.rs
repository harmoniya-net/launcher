//! Harmoniya backend client — the UI-framework-free domain layer.
//!
//! - [`http`] — process-wide tokio runtime + shared `reqwest` client.
//! - [`config`] — config/data directories and atomic JSON persistence.
//! - [`auth`] — OAuth2 PKCE login, token refresh (with rotation), keyring storage.
//! - [`services`] — typed calls against the account service and Petal CMS.
//!
//! Everything here returns plain data and is independent of GPUI, so it can be
//! unit-tested and reused on its own.

pub mod auth;
pub mod config;
pub mod http;
pub mod services;

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch (saturating to 0 if the clock predates it).
/// One definition shared by token expiry, refresh, and the launch pipeline.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
