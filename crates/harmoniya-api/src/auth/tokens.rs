use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: u64,
}

impl Tokens {
    pub fn is_access_expired(&self) -> bool {
        crate::now_ms() >= self.expires_at.saturating_sub(30_000)
    }
}
