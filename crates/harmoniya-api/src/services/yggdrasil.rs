use anyhow::{Result, anyhow};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;

use crate::auth;
use crate::http;

pub const YGGDRASIL_BASE: &str = "https://bifrost.harmoniya.net";

#[derive(Clone, Debug, Default)]
pub struct SkinProfile {
    pub skin_url: Option<String>,
    pub cape_url: Option<String>,
    pub skin_model: SkinModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SkinModel {
    #[default]
    Classic,
    Slim,
}

impl SkinModel {
    pub fn as_form_str(self) -> &'static str {
        match self { Self::Classic => "classic", Self::Slim => "slim" }
    }
}

#[derive(Deserialize)]
struct ProfileEnvelope { properties: Option<Vec<Property>> }

#[derive(Deserialize)]
struct Property { name: String, value: String }

#[derive(Deserialize)]
struct Textures { textures: TextureMap }

#[derive(Deserialize, Default)]
struct TextureMap {
    #[serde(rename = "SKIN")] skin: Option<Texture>,
    #[serde(rename = "CAPE")] cape: Option<Texture>,
}

#[derive(Deserialize)]
struct Texture {
    url: String,
    #[serde(default)] metadata: Option<TextureMeta>,
}

#[derive(Deserialize)]
struct TextureMeta {
    #[serde(default)] model: Option<String>,
}

pub async fn fetch_profile(access_token: &str) -> Result<Option<SkinProfile>> {
    let ygg_token = auth::fetch_yggdrasil_token(access_token).await?;
    let parts: Vec<&str> = ygg_token.split('.').collect();
    if parts.len() < 2 { return Err(anyhow!("malformed yggdrasil token")); }
    let payload = URL_SAFE_NO_PAD
        .decode(parts[1].trim_end_matches('='))
        .map_err(|e| anyhow!("token payload: {e}"))?;
    let claims: serde_json::Value = serde_json::from_slice(&payload)?;
    let uuid = claims.get("uuid").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("no uuid in claims"))?;
    let clean = uuid.replace('-', "");

    let profile_url = format!("{YGGDRASIL_BASE}/session/minecraft/profile/{clean}");
    let mut resp = http::client()
        .get(&profile_url)
        .send()
        .await
        .map_err(|e| anyhow!("profile: {e}"))?;

    // Bifrost upserts the profile from the JWT on any *authed* request, but this
    // read isn't authed. On a brand-new account's first login the profile isn't
    // cached yet, so bifrost 204s and the skin viewer would be blank. Warm up
    // with one authed call to register the player — bifrost then serves the
    // default skin for the (now skin-less) profile — and retry the read once.
    if resp.status() == reqwest::StatusCode::NO_CONTENT {
        let _ = http::client()
            .get(format!("{YGGDRASIL_BASE}/privileges"))
            .bearer_auth(&ygg_token)
            .send()
            .await;
        resp = http::client()
            .get(&profile_url)
            .send()
            .await
            .map_err(|e| anyhow!("profile: {e}"))?;
    }

    if resp.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let envelope: ProfileEnvelope = resp.error_for_status()?.json().await?;

    let mut profile = SkinProfile::default();
    let Some(props) = envelope.properties else { return Ok(Some(profile)); };
    let Some(tex_prop) = props.into_iter().find(|p| p.name == "textures") else {
        return Ok(Some(profile));
    };
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(tex_prop.value)
        .map_err(|e| anyhow!("textures b64: {e}"))?;
    let parsed: Textures = serde_json::from_slice(&decoded)?;

    if let Some(skin) = parsed.textures.skin {
        profile.skin_url = Some(skin.url);
        if matches!(skin.metadata.and_then(|m| m.model).as_deref(), Some("slim")) {
            profile.skin_model = SkinModel::Slim;
        }
    }
    if let Some(cape) = parsed.textures.cape {
        profile.cape_url = Some(cape.url);
    }
    Ok(Some(profile))
}

pub async fn upload_skin(access_token: &str, bytes: Vec<u8>, filename: String, model: SkinModel) -> Result<()> {
    let ygg_token = auth::fetch_yggdrasil_token(access_token).await?;
    let form = Form::new()
        .part("file", Part::bytes(bytes).file_name(filename).mime_str("image/png")?)
        .text("variant", model.as_form_str());
    let resp = http::client()
        .post(format!("{YGGDRASIL_BASE}/minecraft/profile/skins"))
        .bearer_auth(ygg_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("upload skin: {e}"))?;
    resp.error_for_status().map_err(|e| anyhow!("upload skin: {e}"))?;
    Ok(())
}

pub async fn upload_cape(access_token: &str, bytes: Vec<u8>, filename: String) -> Result<()> {
    let ygg_token = auth::fetch_yggdrasil_token(access_token).await?;
    let form = Form::new()
        .part("file", Part::bytes(bytes).file_name(filename).mime_str("image/png")?);
    let resp = http::client()
        .post(format!("{YGGDRASIL_BASE}/minecraft/profile/capes"))
        .bearer_auth(ygg_token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| anyhow!("upload cape: {e}"))?;
    resp.error_for_status().map_err(|e| anyhow!("upload cape: {e}"))?;
    Ok(())
}

pub async fn reset_skin(access_token: &str) -> Result<()> {
    delete_active(access_token, "skins").await
}

pub async fn reset_cape(access_token: &str) -> Result<()> {
    delete_active(access_token, "capes").await
}

async fn delete_active(access_token: &str, kind: &str) -> Result<()> {
    let ygg_token = auth::fetch_yggdrasil_token(access_token).await?;
    let resp = http::client()
        .delete(format!("{YGGDRASIL_BASE}/minecraft/profile/{kind}/active"))
        .bearer_auth(ygg_token)
        .send()
        .await
        .map_err(|e| anyhow!("delete {kind}: {e}"))?;
    resp.error_for_status().map_err(|e| anyhow!("delete {kind}: {e}"))?;
    Ok(())
}
