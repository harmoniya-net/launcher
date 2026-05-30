use anyhow::Result;
use keyring::Entry;

use super::tokens::Tokens;
use crate::config;

const SERVICE: &str = "harmoniya-launcher";
const USER: &str = "session";
const FILE: &str = "tokens.json";

pub fn load() -> Option<Tokens> {
    if let Ok(entry) = Entry::new(SERVICE, USER) {
        if let Ok(raw) = entry.get_password() {
            if let Ok(t) = serde_json::from_str::<Tokens>(&raw) {
                return Some(t);
            }
        }
    }
    config::load_json::<Tokens>(FILE)
}

pub fn save(tokens: &Tokens) -> Result<()> {
    let json = serde_json::to_string(tokens)?;
    let keyring_ok = Entry::new(SERVICE, USER)
        .and_then(|e| e.set_password(&json))
        .is_ok();
    if !keyring_ok {
        tracing::warn!("keyring unavailable, falling back to file");
        config::save_json(FILE, tokens)?;
    } else {
        let _ = std::fs::remove_file(config::config_path(FILE).unwrap_or_default());
    }
    Ok(())
}

pub fn clear() {
    if let Ok(entry) = Entry::new(SERVICE, USER) {
        let _ = entry.delete_credential();
    }
    let _ = std::fs::remove_file(config::config_path(FILE).unwrap_or_default());
}
