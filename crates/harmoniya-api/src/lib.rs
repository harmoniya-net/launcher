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
