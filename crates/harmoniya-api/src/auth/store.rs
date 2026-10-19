//! The single owner of the session token.
//!
//! Every authenticated call asks [`SessionStore`] for the token, so there is
//! one in-memory copy, kept in step with the persisted one (keyring/file).

use std::sync::{Arc, Mutex, MutexGuard};

use anyhow::{Result, anyhow};

use super::storage;

pub struct SessionStore {
    /// Plain (sync) mutex — never held across an `.await`, so every method
    /// stays callable from sync UI code.
    token: Mutex<Option<String>>,
}

impl SessionStore {
    /// Build the store from whatever session was persisted.
    pub fn load() -> Arc<Self> {
        Arc::new(Self { token: Mutex::new(storage::load()) })
    }

    /// Cheap synchronous "is there a session?" check for UI/route gating.
    pub fn signed_in(&self) -> bool {
        self.lock().is_some()
    }

    /// The session token. The `auth:` prefix is what `is_auth_dead` matches.
    pub fn token(&self) -> Result<String> {
        self.lock().clone().ok_or_else(|| anyhow!("auth: signed out"))
    }

    /// Adopt the token from a fresh sign-in (persists immediately).
    pub fn set(&self, token: String) {
        if let Err(e) = storage::save(&token) {
            tracing::warn!(error = %e, "failed to persist session");
        }
        *self.lock() = Some(token);
    }

    /// Forget the session (persisted + in-memory) and hand back the token so
    /// the caller can revoke it server-side.
    pub fn take(&self) -> Option<String> {
        storage::clear();
        self.lock().take()
    }

    fn lock(&self) -> MutexGuard<'_, Option<String>> {
        // Recover from poisoning rather than cascading a panic: a poisoned lock
        // just means some task panicked mid-update; the data itself is fine.
        self.token.lock().unwrap_or_else(|e| e.into_inner())
    }
}
