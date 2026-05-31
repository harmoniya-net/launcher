pub mod callback_server;
pub mod pkce;
pub mod storage;
pub mod tokens;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::sync::LazyLock;
use tokio::sync::Mutex;
use url::Url;

use crate::http;
use crate::now_ms;
use tokens::Tokens;

pub const ACCOUNT_SERVICE_BASE: &str = "https://account.harmoniya.net";
pub const OAUTH_CLIENT_ID: &str = "IMcNOJwJQpQLCTpqIdinmiUTMtJAHvPy";
pub const OAUTH_SCOPE: &str = "openid email profile offline_access";

#[derive(Debug, Deserialize)]
struct RawTokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<u64>,
}

/// Runs the full OAuth2 PKCE flow: opens browser, runs a one-shot loopback listener,
/// exchanges code → tokens. The loopback listener is sync (`tiny_http`) and is
/// dispatched onto a blocking thread so it does not stall the executor.
pub async fn run_login_flow(provider: Provider) -> Result<Tokens> {
    let pkce = pkce::generate();
    let state = pkce::random_state();
    let listener = callback_server::start()?;
    let port = listener.port();
    // Match the path that's registered with the OAuth client (see existing Nuxt launcher).
    let redirect_uri = format!("http://127.0.0.1:{port}/api/auth/callback");

    let mut url = Url::parse(&format!("{ACCOUNT_SERVICE_BASE}/api/auth/oauth2/authorize"))?;
    url.query_pairs_mut()
        .append_pair("client_id", OAUTH_CLIENT_ID)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("scope", provider.scope())
        .append_pair("state", &state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256");

    if let Some(p) = provider.upstream_param() {
        url.query_pairs_mut().append_pair("connection", p);
    }

    open::that(url.as_str()).context("open browser")?;

    let captured = std::thread::spawn(move || listener.wait_for_callback())
        .join()
        .map_err(|_| anyhow!("callback thread panicked"))??;
    if captured.state != state {
        return Err(anyhow!("oauth state mismatch"));
    }
    let code = captured.code.ok_or_else(|| anyhow!("no code in callback"))?;
    exchange_code(&code, &pkce.verifier, &redirect_uri).await
}

pub enum Provider {
    Harmoniya,
    Discord,
}

impl Provider {
    fn scope(&self) -> &'static str { OAUTH_SCOPE }
    fn upstream_param(&self) -> Option<&'static str> {
        match self {
            Provider::Harmoniya => None,
            Provider::Discord => Some("discord"),
        }
    }
}

async fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<Tokens> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", OAUTH_CLIENT_ID),
        ("code_verifier", verifier),
    ];
    let raw: RawTokens = http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/auth/oauth2/token"))
        .form(&form)
        .send()
        .await
        .map_err(|e| anyhow!("token exchange: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow!("token exchange: {e}"))?
        .json()
        .await?;
    Ok(Tokens {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token,
        expires_at: now_ms() + raw.expires_in.unwrap_or(3600) * 1000,
    })
}

/// Caches the result of the most recent refresh, keyed on the refresh token it
/// consumed. See [`coordinated_refresh`].
static REFRESH_CACHE: LazyLock<Mutex<Option<(String, Tokens)>>> = LazyLock::new(|| Mutex::new(None));

/// Refresh, but coordinated across concurrent callers.
///
/// The account service **rotates** refresh tokens — each successful refresh
/// invalidates the token it consumed. Several token consumers (`fetch_me`,
/// `fetch_skin_profile`, launch, …) run concurrently and each holds its own
/// clone of the session, so without coordination they'd all POST the *same*
/// refresh token at once: the first wins and the rest get `invalid_grant`,
/// which we mistook for a dead session and logged the user out.
///
/// We serialize refreshes through one mutex and cache the result keyed on the
/// token consumed, so concurrent callers sharing a token collapse to a single
/// server round-trip and all receive the same rotated tokens.
pub async fn coordinated_refresh(refresh_token: &str) -> Result<Tokens> {
    let mut guard = REFRESH_CACHE.lock().await;
    if let Some((used, result)) = guard.as_ref() {
        // Reuse only while still fresh, so a non-rotating server (same refresh
        // token back) doesn't pin us to a stale access token forever.
        if used == refresh_token && !result.is_access_expired() {
            return Ok(result.clone());
        }
    }
    let new = refresh_tokens(refresh_token).await?;
    *guard = Some((refresh_token.to_string(), new.clone()));
    Ok(new)
}

/// Ensure the access token is usable: if it's expired (or about to be) and we
/// hold a refresh token, refresh it (coordinated across callers). Returns the
/// tokens to use plus `Some(rotated)` when they changed, so the caller can
/// persist + adopt the rotation. The single home for the "refresh if stale"
/// step that every token consumer (account, skin, launch) needs.
pub async fn ensure_fresh(tokens: Tokens) -> Result<(Tokens, Option<Tokens>)> {
    if tokens.is_access_expired() {
        if let Some(refresh) = tokens.refresh_token.clone() {
            let rotated = coordinated_refresh(&refresh).await?;
            return Ok((rotated.clone(), Some(rotated)));
        }
    }
    Ok((tokens, None))
}

pub async fn refresh_tokens(refresh_token: &str) -> Result<Tokens> {
    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("client_id", OAUTH_CLIENT_ID),
    ];
    let resp = http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/auth/oauth2/token"))
        .form(&form)
        .send()
        .await
        .map_err(|e| anyhow!("refresh: {e}"))?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("refresh: HTTP {status}: {body}"));
    }
    let raw: RawTokens = serde_json::from_str(&body)
        .map_err(|e| anyhow!("refresh: decode response: {e} (body: {body})"))?;
    Ok(Tokens {
        access_token: raw.access_token,
        refresh_token: raw.refresh_token.or(Some(refresh_token.to_string())),
        expires_at: now_ms() + raw.expires_in.unwrap_or(3600) * 1000,
    })
}

pub async fn fetch_yggdrasil_token(access_token: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct R { token: Option<String> }
    let r: R = http::client()
        .post(format!("{ACCOUNT_SERVICE_BASE}/api/bifrost"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| anyhow!("bifrost: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow!("bifrost: {e}"))?
        .json()
        .await?;
    r.token.ok_or_else(|| anyhow!("no bifrost token in response"))
}
