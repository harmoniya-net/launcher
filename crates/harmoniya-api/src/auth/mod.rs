//! Sign-in with the OAuth device flow (RFC 8628) against the account service.
//!
//! [`start_login`] asks for a code, the user approves it on the account site
//! (we open the page for them), and [`DeviceLogin::wait`] polls until that
//! happens. The result is a session token, which every authenticated call sends
//! as `Authorization: Bearer`. There is no refresh token: the session slides
//! forward while it's used, and a 401 means "sign in again".

pub mod storage;
pub mod store;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use url::Url;

use crate::http;
use crate::obs::cause_chain;
pub use store::SessionStore;

pub const ACCOUNT_SERVICE_BASE: &str = "https://account.harmoniya.net";
/// Must match `LAUNCHER_CLIENT_ID` in the account service.
const CLIENT_ID: &str = "launcher";
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Clone, Copy)]
pub enum Provider {
    Harmoniya,
    Discord,
}

/// Why a device sign-in ended without a session.
#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error("sign-in was denied in the browser")]
    Denied,
    #[error("sign-in code expired")]
    Expired,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// A sign-in waiting for the user to approve [`user_code`](Self::user_code) in
/// the browser.
pub struct DeviceLogin {
    device_code: String,
    /// The code as shown to the user, `ABCD-1234`; the approval page shows it too.
    pub user_code: String,
    /// The approval page with the code filled in.
    pub verification_url: String,
    interval: Duration,
    expires_at: Instant,
}

/// Ask the account service for a device code. `provider` only picks which
/// sign-in the approval page leads with when the browser isn't signed in yet.
pub async fn start_login(provider: Provider) -> Result<DeviceLogin> {
    #[derive(Deserialize)]
    struct CodeResponse {
        device_code: String,
        user_code: String,
        verification_uri_complete: String,
        expires_in: u64,
        interval: u64,
    }

    let r: CodeResponse = http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/auth/device/code"))
        .json(&serde_json::json!({ "client_id": CLIENT_ID }))
        .send()
        .await
        // Fold reqwest's `source()` chain into the message: the bare error is
        // the opaque "error sending request for url (…)"; the real cause (DNS /
        // connect refused / TLS-trust failure) is one level down.
        .map_err(|e| anyhow!("sign-in: {}", cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("sign-in: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("sign-in: decode device code: {e}"))?;

    let mut url = Url::parse(&r.verification_uri_complete).context("sign-in: verification url")?;
    if let Provider::Discord = provider {
        url.query_pairs_mut().append_pair("provider", "discord");
    }

    Ok(DeviceLogin {
        device_code: r.device_code,
        user_code: format_user_code(&r.user_code),
        verification_url: url.into(),
        interval: Duration::from_secs(r.interval.max(1)),
        expires_at: Instant::now() + Duration::from_secs(r.expires_in),
    })
}

impl DeviceLogin {
    pub fn open_browser(&self) -> Result<()> {
        open::that(&self.verification_url).context("open browser")
    }

    /// Poll until the user approves or denies the code, or it expires. Returns
    /// the session token. Network hiccups don't end the wait; the code's expiry
    /// does.
    pub async fn wait(self) -> Result<String, LoginError> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }
        #[derive(Deserialize)]
        struct ErrorResponse {
            error: String,
        }

        let mut interval = self.interval;
        loop {
            tokio::time::sleep(interval).await;
            if Instant::now() >= self.expires_at {
                return Err(LoginError::Expired);
            }

            let resp = http::client()
                .post(format!("{ACCOUNT_SERVICE_BASE}/api/auth/device/token"))
                .json(&serde_json::json!({
                    "grant_type": DEVICE_CODE_GRANT,
                    "device_code": self.device_code,
                    "client_id": CLIENT_ID,
                }))
                .send()
                .await;
            let resp = match resp {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %cause_chain(&e), "device token poll failed");
                    continue;
                }
            };

            let status = resp.status();
            if status.is_success() {
                let body: TokenResponse = resp
                    .json()
                    .await
                    .map_err(|e| anyhow!("sign-in: decode token: {e}"))?;
                return Ok(body.access_token);
            }
            if status.is_server_error() {
                tracing::warn!(%status, "device token poll failed");
                continue;
            }

            let body = resp.text().await.unwrap_or_default();
            let error = serde_json::from_str::<ErrorResponse>(&body)
                .map(|e| e.error)
                .unwrap_or_default();
            match error.as_str() {
                "authorization_pending" => {}
                // RFC 8628 §3.5: back off by 5 seconds for every slow_down.
                "slow_down" => interval += Duration::from_secs(5),
                "access_denied" => return Err(LoginError::Denied),
                "expired_token" => return Err(LoginError::Expired),
                _ => return Err(anyhow!("sign-in: HTTP {status}: {body}").into()),
            }
        }
    }
}

/// Revoke the session server-side. Signing out locally doesn't depend on it.
pub async fn sign_out(token: &str) -> Result<()> {
    http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/auth/sign-out"))
        .bearer_auth(token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| anyhow!("sign-out: {}", cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("sign-out: {e}"))?;
    Ok(())
}

pub async fn fetch_yggdrasil_token(token: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct R {
        token: Option<String>,
    }
    let r: R = http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/bifrost"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| anyhow!("bifrost: {}", cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("bifrost: {e}"))?
        .json()
        .await?;
    r.token.ok_or_else(|| anyhow!("no bifrost token in response"))
}

/// The account service hands out bare 8-character codes; show them as
/// `ABCD-1234`, like the approval page does.
fn format_user_code(code: &str) -> String {
    if code.len() == 8 && code.is_ascii() {
        format!("{}-{}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::format_user_code;

    #[test]
    fn user_code_is_split_in_halves() {
        assert_eq!(format_user_code("ABCD1234"), "ABCD-1234");
        assert_eq!(format_user_code("ABC"), "ABC");
    }
}
