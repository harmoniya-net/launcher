//! The player's launcher permissions, live from jus.
//!
//! The launcher holds a player-scoped, read-only `GET /events` socket (opened
//! with the bifrost JWT, never an API key) and resolves its own permissions
//! with `jus-client`, the same way a Minecraft server does from the full
//! feed. Every check runs at the [`LAUNCHER_SCOPE`] scope (grants at
//! `default` apply there too); the state itself carries every scope.

use std::sync::{Arc, Mutex};

use anyhow::Result;
use base64::Engine;
use jus_client::client::{Client, Config, Status};
use jus_client::State;
use tokio::sync::watch;

use crate::auth::{self, SessionStore};
use crate::http;

pub const JUS_EVENTS_URL: &str = "wss://jus.harmoniya.net/events";
/// The scope every launcher check resolves in.
pub const LAUNCHER_SCOPE: &str = "launcher";

/// The live connection. Dropping it closes the socket.
pub struct JusConnection {
    client: Client,
    /// The uuid the last token was issued for (lowercase, dashed).
    player: Arc<Mutex<Option<String>>>,
}

impl JusConnection {
    /// Connects in the background on the shared tokio runtime. Each
    /// (re)connect fetches a fresh bifrost token from the session.
    pub fn connect(session: Arc<SessionStore>) -> JusConnection {
        let player = Arc::new(Mutex::new(None));
        let slot = player.clone();
        let token = Arc::new(move || {
            let session = session.clone();
            let slot = slot.clone();
            Box::pin(async move {
                let jwt = bifrost_token(&session).await.map_err(|e| format!("{:#}", e))?;
                let uuid = jwt_uuid(&jwt).ok_or("bifrost token has no uuid")?;
                *slot.lock().unwrap() = Some(uuid);
                Ok(jwt)
            }) as jus_client::client::TokenFuture
        });
        let client = Client::spawn_on(&http::handle(), Config { url: JUS_EVENTS_URL.to_owned(), token });
        JusConnection { client, player }
    }

    /// Changes on every snapshot and update.
    pub fn subscribe(&self) -> watch::Receiver<Option<Arc<State>>> {
        self.client.subscribe()
    }

    pub fn status(&self) -> Status {
        self.client.status()
    }

    /// The current permissions, once the first snapshot has arrived.
    pub fn permissions(&self) -> Option<Permissions> {
        let state = self.client.state()?;
        let player = self.player.lock().unwrap().clone()?;
        Some(Permissions { state, player })
    }
}

async fn bifrost_token(session: &SessionStore) -> Result<String> {
    auth::fetch_yggdrasil_token(&session.token()?).await
}

/// The `uuid` claim of a bifrost JWT (32 hex, no dashes), normalized. The
/// signature is jus's to check; this only reads which player we are.
fn jwt_uuid(jwt: &str) -> Option<String> {
    let payload = jwt.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    let claims: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    Some(jus_client::normalize_uuid(claims.get("uuid")?.as_str()?))
}

/// A snapshot of one player's permissions, resolved at [`LAUNCHER_SCOPE`].
#[derive(Clone)]
pub struct Permissions {
    state: Arc<State>,
    player: String,
}

impl Permissions {
    fn has(&self, node: &str) -> bool {
        let r = self.state.resolver();
        r.has(&self.player, "harmoniya.launcher.admin", LAUNCHER_SCOPE) || r.has(&self.player, node, LAUNCHER_SCOPE)
    }

    /// Master switch: grants every launcher permission.
    pub fn is_admin(&self) -> bool {
        self.state.resolver().has(&self.player, "harmoniya.launcher.admin", LAUNCHER_SCOPE)
    }

    pub fn can_view_modpack(&self, server: &str) -> bool {
        self.has(&format!("harmoniya.launcher.modpack.view.{server}"))
    }

    pub fn can_play_modpack(&self, server: &str) -> bool {
        self.has(&format!("harmoniya.launcher.modpack.play.{server}"))
    }

    pub fn can_bypass_maintenance(&self, server: &str) -> bool {
        self.has(&format!("harmoniya.launcher.modpack.bypass.{server}"))
    }

    pub fn can_view_feature(&self, name: &str) -> bool {
        self.has(&format!("harmoniya.launcher.feature.{name}"))
    }

    pub fn can_upload_skin(&self) -> bool {
        self.has("harmoniya.launcher.texture.skin")
    }

    pub fn can_upload_skin_hd(&self) -> bool {
        self.has("harmoniya.launcher.texture.skin.hd")
    }

    pub fn can_upload_cape(&self) -> bool {
        self.has("harmoniya.launcher.texture.cape")
    }

    pub fn can_upload_cape_hd(&self) -> bool {
        self.has("harmoniya.launcher.texture.cape.hd")
    }

    /// The player's rank: the text and name color of their `chat.prefix`
    /// (the `value` metadata of the winning grant, e.g. `&7[&cОмеґа&7] `).
    pub fn rank(&self) -> Option<(String, Option<u32>)> {
        let raw = self.state.resolver().meta(&self.player, "chat.prefix", LAUNCHER_SCOPE)?;
        let (text, color) = parse_prefix(&raw);
        (!text.is_empty()).then_some((text, color))
    }
}

/// Minecraft `&` color codes, 0-f.
fn mc_color(code: char) -> Option<u32> {
    Some(match code.to_ascii_lowercase() {
        '0' => 0x000000,
        '1' => 0x0000AA,
        '2' => 0x00AA00,
        '3' => 0x00AAAA,
        '4' => 0xAA0000,
        '5' => 0xAA00AA,
        '6' => 0xFFAA00,
        '7' => 0xAAAAAA,
        '8' => 0x555555,
        '9' => 0x5555FF,
        'a' => 0x55FF55,
        'b' => 0x55FFFF,
        'c' => 0xFF5555,
        'd' => 0xFF55FF,
        'e' => 0xFFFF55,
        'f' => 0xFFFFFF,
        _ => return None,
    })
}

/// `&7[&cОмеґа&7] &7` -> (`Омеґа`, red): the name's color is the code right
/// after `[`; the text drops every `&x` code and the brackets. Same rule
/// lucky applied to LuckPerms prefixes.
fn parse_prefix(raw: &str) -> (String, Option<u32>) {
    let color = raw.find("[&").and_then(|i| raw[i + 2..].chars().next()).and_then(mc_color);
    let mut text = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '&' if chars.peek().is_some_and(|n| n.is_ascii_alphanumeric()) => {
                chars.next();
            }
            '[' | ']' => {}
            '\\' if chars.peek() == Some(&'.') => {}
            c => text.push(c),
        }
    }
    (text.trim().to_owned(), color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const P: &str = "176cbd47-25a2-4af3-b459-c39bcf101da2";

    fn perms(snapshot: serde_json::Value) -> Permissions {
        Permissions { state: Arc::new(State::from_snapshot(snapshot).unwrap()), player: P.to_owned() }
    }

    #[test]
    fn parses_luckperms_style_prefixes() {
        assert_eq!(parse_prefix("&7[&cОмеґа&7] &7"), ("Омеґа".to_owned(), Some(0xFF5555)));
        assert_eq!(parse_prefix("&7[&6Тех.Адмін&7] &7"), ("Тех.Адмін".to_owned(), Some(0xFFAA00)));
        assert_eq!(parse_prefix("plain"), ("plain".to_owned(), None));
    }

    #[test]
    fn reads_the_uuid_claim() {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"uuid":"176CBD4725A24AF3B459C39BCF101DA2","username":"x"}"#);
        assert_eq!(jwt_uuid(&format!("h.{payload}.s")).as_deref(), Some(P));
        assert_eq!(jwt_uuid("garbage"), None);
    }

    #[test]
    fn checks_resolve_at_the_launcher_scope() {
        let p = perms(json!({
            "groups": [{"name": "default", "priority": 0}, {"name": "omega", "priority": 15}],
            "groupPermissions": [
                {"group": "default", "scope": "default", "node": "harmoniya.launcher.modpack.play.wizard", "value": true},
                {"group": "default", "scope": "default", "node": "chat.prefix", "value": true, "metadata": {"value": "&7[&7Гравець&7] &7"}},
                {"group": "default", "scope": "technomagic", "node": "harmoniya.launcher.feature.tm_nea", "value": true},
                {"group": "omega", "scope": "default", "node": "harmoniya.launcher.texture.cape", "value": true},
                {"group": "omega", "scope": "default", "node": "chat.prefix", "value": true, "metadata": {"value": "&7[&cОмеґа&7] &7"}}
            ],
            "playerInheritance": [{"player": P, "group": "omega", "scope": "launcher", "value": true}]
        }));
        assert!(p.can_play_modpack("wizard"), "default-scope grants apply at launcher");
        assert!(!p.can_play_modpack("technomagic"));
        assert!(!p.can_view_feature("tm_nea"), "a technomagic-scoped grant doesn't");
        assert!(p.can_upload_cape(), "launcher-scoped membership counts");
        assert!(!p.is_admin());
        assert_eq!(p.rank(), Some(("Омеґа".to_owned(), Some(0xFF5555))), "heavier group's prefix wins");
    }

    #[test]
    fn admin_grants_everything() {
        let p = perms(json!({
            "playerPermissions": [{"player": P, "scope": "launcher", "node": "harmoniya.launcher.admin", "value": true}]
        }));
        assert!(p.is_admin() && p.can_bypass_maintenance("anything") && p.can_upload_skin_hd());
    }
}

#[cfg(test)]
mod live {
    use std::time::Duration;

    use jus_client::client::{Client, Config, Status};

    /// Network: the launcher's TLS stack reaches jus, and a bad token is a
    /// clean HTTP 401 retry, not a panic. `cargo test -p harmoniya-api -- --ignored`
    #[test]
    #[ignore]
    fn bad_token_is_a_clean_401_against_prod() {
        let rt = crate::http::handle();
        rt.block_on(async {
            let client = Client::spawn(Config {
                url: super::JUS_EVENTS_URL.to_owned(),
                token: std::sync::Arc::new(|| Box::pin(async { Ok("not-a-token".to_owned()) })),
            });
            let mut status = client.subscribe_status();
            let got = tokio::time::timeout(Duration::from_secs(20), async {
                loop {
                    status.changed().await.unwrap();
                    if let Status::Retrying { error } = &*status.borrow() {
                        return error.clone();
                    }
                }
            })
            .await
            .expect("no connect result within 20s");
            assert!(got.contains("HTTP 401"), "{got}");
        });
    }
}
