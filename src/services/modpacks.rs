use anyhow::{Result, anyhow};
use serde::{Deserialize, Deserializer, Serialize};

use crate::http;
use crate::services::options;

const CMS_GRAPHQL_URL: &str = "https://petal.harmoniya.net/graphql";

const QUERY: &str = r#"query Modpacks {
  modpacks {
    items {
      id title summary version manifestUrl maintaining order description
      banner { url(format: webp, width: 1280, height: 400) }
      status { online max }
      project { id title order logo { url } }
      announcements { body date }
      options {
        type name title subtitle min max step unit placeholder default
        choices { label value }
        options {
          type name title subtitle min max step unit placeholder default
          choices { label value }
        }
      }
    }
  }
}"#;

/// Deserialize accepting `null` as the type's default value.
fn null_default<'de, T, D>(d: D) -> std::result::Result<T, D::Error>
where T: Default + Deserialize<'de>, D: Deserializer<'de> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Banner {
    #[serde(default)] pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    #[serde(default)] pub online: i32,
    #[serde(default)] pub max: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Logo {
    #[serde(default)] pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    #[serde(default)] pub order: i32,
    #[serde(default)] pub logo: Logo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Announcement {
    pub body: String,
    pub date: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Modpack {
    pub id: String,
    pub title: String,
    #[serde(default)] pub summary: Option<String>,
    #[serde(default)] pub version: Option<String>,
    #[serde(rename = "manifestUrl")] pub manifest_url: String,
    #[serde(default)] pub banner: Option<Banner>,
    #[serde(default)] pub maintaining: bool,
    #[serde(default)] pub order: i32,
    #[serde(default)] pub description: Option<String>,
    #[serde(default)] pub status: Option<Status>,
    pub project: Project,
    #[serde(default, deserialize_with = "null_default")] pub announcements: Vec<Announcement>,
    #[serde(default)] pub options: Vec<options::Field>,
}

#[derive(Clone, Debug)]
pub struct ProjectGroup {
    pub project: Project,
    pub modpacks: Vec<Modpack>,
}

#[derive(Deserialize)]
struct GqlResponse { data: Option<GqlData> }
#[derive(Deserialize)]
struct GqlData { modpacks: GqlItems }
#[derive(Deserialize)]
struct GqlItems {
    #[serde(default)]
    items: Vec<Modpack>,
}

pub async fn fetch_all() -> Result<Vec<Modpack>> {
    let raw = http::client()
        .post(CMS_GRAPHQL_URL)
        .json(&serde_json::json!({ "query": QUERY }))
        .send()
        .await
        .map_err(|e| anyhow!("modpacks request: {e}"))?
        .error_for_status()
        .map_err(|e| anyhow!("modpacks: {e}"))?
        .text()
        .await?;
    let resp: GqlResponse = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("modpacks parse failed: {e}\n--- body ---\n{}\n--- end ---", raw);
            return Err(anyhow!("modpacks parse: {e}"));
        }
    };

    let mut items = resp.data.map(|d| d.modpacks.items).unwrap_or_default();
    items.sort_by(|a, b| {
        let rank = |m: &Modpack| -> i32 {
            if m.maintaining { 1 } else if m.status.is_none() { 2 } else { 0 }
        };
        a.order.cmp(&b.order).then_with(|| rank(a).cmp(&rank(b)))
    });
    Ok(items)
}

pub fn group(modpacks: &[Modpack]) -> Vec<ProjectGroup> {
    let mut order = Vec::<String>::new();
    let mut map = std::collections::HashMap::<String, ProjectGroup>::new();
    for m in modpacks {
        let key = m.project.id.clone();
        if !map.contains_key(&key) {
            order.push(key.clone());
            map.insert(key.clone(), ProjectGroup { project: m.project.clone(), modpacks: Vec::new() });
        }
        map.get_mut(&key).unwrap().modpacks.push(m.clone());
    }
    let mut groups: Vec<_> = order.into_iter().filter_map(|k| map.remove(&k)).collect();
    groups.sort_by_key(|g| g.project.order);
    groups
}
