//! The single owner of the session's OAuth tokens.
//!
//! Every token consumer goes through [`TokenStore`] and receives only a
//! short-lived **access token** — never the refresh token. Because the refresh
//! token never leaves the store, no call site can forget to persist a rotation
//! or replay a stale token. (The account service rotates refresh tokens on every
//! use and *deletes the whole token family* if an already-revoked one is ever
//! replayed, so "persist every rotation" is a hard correctness requirement, not
//! a nicety.)
//!
//! Refreshes are serialized through `refresh_gate` and guarded by a
//! compare-and-swap on the access token: if another caller already rotated while
//! we were waiting, we adopt their result instead of refreshing again (which
//! would revoke the token they just minted).

use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use tokio::sync::Mutex as AsyncMutex;

use super::storage;
use super::tokens::Tokens;

pub struct TokenStore {
    /// The one true copy of the tokens. Plain (sync) mutex — never held across an
    /// `.await`, so `set`/`clear`/`signed_in` stay callable from sync UI code.
    tokens: Mutex<Option<Tokens>>,
    /// Serializes the network refresh so concurrent stale callers collapse to a
    /// single round-trip. Separate from `tokens` so we never hold a lock across
    /// the await.
    refresh_gate: AsyncMutex<()>,
}

impl TokenStore {
    /// Build the store from whatever tokens were persisted (keyring/file).
    pub fn load() -> Arc<Self> {
        Arc::new(Self {
            tokens: Mutex::new(storage::load()),
            refresh_gate: AsyncMutex::new(()),
        })
    }

    /// Cheap synchronous "is there a session?" check for UI/route gating.
    pub fn signed_in(&self) -> bool {
        self.lock().is_some()
    }

    /// Adopt the tokens from a fresh login (persists immediately).
    pub fn set(&self, tokens: Tokens) {
        let _ = storage::save(&tokens);
        *self.lock() = Some(tokens);
    }

    /// Drop the session on logout (wipes persisted + in-memory copies).
    pub fn clear(&self) {
        storage::clear();
        *self.lock() = None;
    }

    /// A usable access token, refreshing (and rotating + persisting) if the
    /// current one is stale. This is the only way consumers obtain a token.
    pub async fn access_token(&self) -> Result<String> {
        let current = self
            .snapshot()
            .ok_or_else(|| anyhow!("refresh: not signed in"))?;
        if !current.is_access_expired() {
            return Ok(current.access_token);
        }
        self.refresh_locked(&current.access_token).await
    }

    /// Force a refresh for the 401-retry path: the access token `tried` was just
    /// rejected by a protected endpoint, so mint a new one. Compare-and-swap
    /// ensures concurrent retries don't each rotate (which would revoke each
    /// other's fresh tokens).
    pub async fn refresh_if_stale(&self, tried: &str) -> Result<String> {
        self.refresh_locked(tried).await
    }

    /// Serialized refresh with compare-and-swap on the access token. If the store
    /// no longer holds `tried` (someone rotated while we waited on the gate), we
    /// return the already-rotated token instead of replaying `tried`.
    async fn refresh_locked(&self, tried: &str) -> Result<String> {
        let _gate = self.refresh_gate.lock().await;

        let current = self
            .snapshot()
            .ok_or_else(|| anyhow!("refresh: signed out"))?;
        if current.access_token != tried {
            // Another caller already rotated — adopt theirs, don't replay ours.
            return Ok(current.access_token);
        }
        let refresh = current
            .refresh_token
            .ok_or_else(|| anyhow!("refresh: session expired (no refresh token)"))?;

        let rotated = super::refresh_tokens(&refresh).await?;
        let access = rotated.access_token.clone();
        self.set(rotated);
        Ok(access)
    }

    fn snapshot(&self) -> Option<Tokens> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Tokens>> {
        // Recover from poisoning rather than cascading a panic: a poisoned lock
        // just means some task panicked mid-update; the token data itself is fine.
        self.tokens.lock().unwrap_or_else(|e| e.into_inner())
    }
}
