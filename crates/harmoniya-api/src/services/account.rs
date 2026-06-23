use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::auth::ACCOUNT_SERVICE_BASE;
use crate::http;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub rank: Option<String>,
}

pub async fn fetch_me(access_token: &str) -> Result<User> {
    http::client()
        .get(format!("{ACCOUNT_SERVICE_BASE}/api/me"))
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| anyhow!("/api/me request: {}", crate::obs::cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("/api/me: {e}"))?
        .json::<User>()
        .await
        .map_err(|e| anyhow!("/api/me parse: {e}"))
}
