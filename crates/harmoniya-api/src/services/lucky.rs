use anyhow::{Result, anyhow};
use serde::Deserialize;

use crate::http;

pub const LUCKY_BASE: &str = "https://lucky.harmoniya.net";

#[derive(Clone, Debug, Deserialize)]
pub struct Group {
    pub group: String,
    pub server: String,
    pub expiry: f64,
    pub prefix: Option<String>,
    pub color: Option<String>,
    pub weight: i32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Rank {
    pub group: String,
    pub prefix: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LuckyProfile {
    pub groups: Vec<Group>,
    pub permissions: Vec<String>,
    pub rank: Option<Rank>,
}

impl LuckyProfile {
    /// Top rank for a specific server.
    pub fn rank_for(&self, server: &str) -> Option<(&str, Option<u32>)> {
        self.groups
            .iter()
            .find(|g| g.server.eq_ignore_ascii_case(server) && g.prefix.is_some())
            .map(|g| (g.prefix.as_deref().unwrap(), parse_color(g.color.as_deref())))
    }

    /// Master switch: grants every launcher permission.
    pub fn is_admin(&self) -> bool {
        self.permissions.iter().any(|p| p == "harmoniya.launcher.admin")
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

    fn has(&self, perm: &str) -> bool {
        self.is_admin() || self.permissions.iter().any(|p| p == perm)
    }
}

fn parse_color(hex: Option<&str>) -> Option<u32> {
    u32::from_str_radix(hex?.strip_prefix('#')?, 16).ok()
}

pub async fn fetch_profile(yggdrasil_token: &str) -> Result<LuckyProfile> {
    http::client()
        .get(format!("{LUCKY_BASE}/me"))
        .bearer_auth(yggdrasil_token)
        .send()
        .await
        .map_err(|e| anyhow!("lucky /me: {}", crate::obs::cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("lucky /me: {e}"))?
        .json::<LuckyProfile>()
        .await
        .map_err(|e| anyhow!("lucky /me parse: {e}"))
}
