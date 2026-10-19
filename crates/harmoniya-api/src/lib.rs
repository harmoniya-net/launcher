//! Harmoniya backend client — the UI-framework-free domain layer.
//!
//! - [`http`] — process-wide tokio runtime + shared `reqwest` client.
//! - [`config`] — config/data directories and atomic JSON persistence.
//! - [`auth`] — device-flow sign-in, the session token store, keyring storage.
//! - [`services`] — typed calls against the account service and the CMS
//!   (one.harmoniya.net's public GraphQL API).
//!
//! Everything here returns plain data and is independent of GPUI, so it can be
//! unit-tested and reused on its own.

pub mod auth;
pub mod config;
pub mod http;
pub mod obs;
pub mod services;

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch (saturating to 0 if the clock predates it).
/// One definition shared across the crate and the launch pipeline.
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
