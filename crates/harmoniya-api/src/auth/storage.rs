//! Persists the session token: the OS keyring when there is one, a file in the
//! config dir otherwise.

use anyhow::Result;
use keyring::Entry;

use crate::config;

const SERVICE: &str = "harmoniya-launcher";
const USER: &str = "session";
const FILE: &str = "session.json";

pub fn load() -> Option<String> {
    if let Ok(token) = Entry::new(SERVICE, USER).and_then(|e| e.get_password()) {
        return Some(token);
    }
    config::load_json::<String>(FILE)
}

pub fn save(token: &str) -> Result<()> {
    let keyring_ok = Entry::new(SERVICE, USER)
        .and_then(|e| e.set_password(token))
        .is_ok();
    if keyring_ok {
        let _ = std::fs::remove_file(config::config_path(FILE).unwrap_or_default());
    } else {
        tracing::warn!("keyring unavailable, falling back to file");
        config::save_json(FILE, &token)?;
    }
    Ok(())
}

pub fn clear() {
    if let Ok(entry) = Entry::new(SERVICE, USER) {
        let _ = entry.delete_credential();
    }
    let _ = std::fs::remove_file(config::config_path(FILE).unwrap_or_default());
}
