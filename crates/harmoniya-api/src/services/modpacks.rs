use anyhow::{Result, anyhow};
use serde::{Deserialize, Deserializer, Serialize};

use crate::http;
use crate::services::options;

const CMS_GRAPHQL_URL: &str = "https://one.harmoniya.net/api/graphql";
/// Where `banner`/`logo` R2 object keys resolve to — same bypass-Access path
/// as the GraphQL API itself (see one.harmoniya.net's alchemy.run.ts).
const FILES_BASE_URL: &str = "https://one.harmoniya.net/files";

const QUERY: &str = r#"query Modpacks {
  modpacks {
    id title summary version manifestUrl maintaining order description
    banner
    status { online max }
    servers { title }
    project { id title order logo }
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
}"#;

/// Deserialize accepting `null` as the type's default value.
fn null_default<'de, T, D>(d: D) -> std::result::Result<T, D::Error>
where T: Default + Deserialize<'de>, D: Deserializer<'de> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// `key` is the bare R2 object key the CMS stores (`"banners/<uuid>.png"`) —
/// empty means unset. `None` either way so callers keep treating a missing
/// image as absent rather than a URL to a 404.
fn file_url(key: &str) -> Option<String> {
    (!key.is_empty()).then(|| format!("{FILES_BASE_URL}/{key}"))
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModpackBanner {
    #[serde(default)] pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerStatus {
    #[serde(default)] pub online: i32,
    #[serde(default)] pub max: i32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProjectLogo {
    #[serde(default)] pub url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    #[serde(default)] pub order: i32,
    #[serde(default)] pub logo: ProjectLogo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModpackAnnouncement {
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
    #[serde(default)] pub banner: Option<ModpackBanner>,
    #[serde(default)] pub maintaining: bool,
    #[serde(default)] pub order: i32,
    #[serde(default)] pub description: Option<String>,
    #[serde(default)] pub status: Option<ServerStatus>,
    pub project: Project,
    #[serde(default, deserialize_with = "null_default")] pub announcements: Vec<ModpackAnnouncement>,
    #[serde(default)] pub options: Vec<options::Field>,
}

#[derive(Clone, Debug)]
pub struct ProjectGroup {
    pub project: Project,
    pub modpacks: Vec<Modpack>,
}

/// Wire shape of the `one.harmoniya.net` API — flat fields where the old
/// petal schema nested objects (`banner`, `project.logo`), and a `status`
/// that's always present (a live `mcping` result) where petal's could be
/// absent. `from_raw` below adapts both back to the shape the rest of the
/// app already expects.
#[derive(Deserialize)]
struct RawModpack {
    id: String,
    title: String,
    #[serde(default)] summary: String,
    #[serde(default)] version: String,
    #[serde(rename = "manifestUrl")] manifest_url: String,
    #[serde(default)] maintaining: bool,
    #[serde(default)] order: i32,
    #[serde(default)] description: String,
    #[serde(default)] banner: String,
    status: RawStatus,
    #[serde(default)] servers: Vec<RawServerRef>,
    project: RawProject,
    #[serde(default, deserialize_with = "null_default")] announcements: Vec<ModpackAnnouncement>,
    #[serde(default)] options: Vec<options::Field>,
}

#[derive(Deserialize)]
struct RawStatus {
    #[serde(default)] online: i32,
    #[serde(default)] max: i32,
}

/// Only fetched to tell "no server configured yet" (empty list) apart from a
/// real live status — the title itself isn't used.
#[derive(Deserialize)]
struct RawServerRef {
    #[allow(dead_code)]
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
struct RawProject {
    id: String,
    title: String,
    #[serde(default)] order: i32,
    #[serde(default)] logo: String,
}

impl RawModpack {
    fn into_modpack(self) -> Modpack {
        Modpack {
            id: self.id,
            title: self.title,
            summary: (!self.summary.is_empty()).then_some(self.summary),
            version: (!self.version.is_empty()).then_some(self.version),
            manifest_url: self.manifest_url,
            banner: file_url(&self.banner).map(|url| ModpackBanner { url: Some(url) }),
            maintaining: self.maintaining,
            order: self.order,
            description: (!self.description.is_empty()).then_some(self.description),
            // No servers configured for this modpack yet == no status to show,
            // same as petal omitting `status` entirely; a configured-but-down
            // server still reports Some({online: 0, ..}) via mcping.
            status: (!self.servers.is_empty())
                .then_some(ServerStatus { online: self.status.online, max: self.status.max }),
            project: Project {
                id: self.project.id,
                title: self.project.title,
                order: self.project.order,
                logo: ProjectLogo { url: file_url(&self.project.logo) },
            },
            announcements: self.announcements,
            options: self.options,
        }
    }
}

#[derive(Deserialize)]
struct GqlResponse { data: Option<GqlData> }
#[derive(Deserialize)]
struct GqlData {
    #[serde(default)]
    modpacks: Vec<RawModpack>,
}

pub async fn fetch_all() -> Result<Vec<Modpack>> {
    let raw = http::client()
        .post(CMS_GRAPHQL_URL)
        .json(&serde_json::json!({ "query": QUERY }))
        .send()
        .await
        .map_err(|e| anyhow!("modpacks request: {}", crate::obs::cause_chain(&e)))?
        .error_for_status()
        .map_err(|e| anyhow!("modpacks: {e}"))?
        .text()
        .await?;
    let resp: GqlResponse = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %crate::obs::cause_chain(&e), body = %raw, "modpacks parse failed");
            return Err(anyhow!("modpacks parse: {e}"));
        }
    };

    let mut items: Vec<Modpack> = resp
        .data
        .map(|d| d.modpacks)
        .unwrap_or_default()
        .into_iter()
        .map(RawModpack::into_modpack)
        .collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// One real modpack's shape from `one.harmoniya.net/api/graphql` (banner/
    /// logo as bare R2 keys, `status` always present, `announcements: null`,
    /// options with `choices`/`options` null on non-select/non-feature rows)
    /// plus a second, server-less entry to exercise the "no status" path.
    const RESPONSE: &str = r##"{
      "data": {
        "modpacks": [
          {
            "id": "arcanetech", "title": "ArcaneTech", "summary": "Палічка та ваджра",
            "version": "1.7.10",
            "manifestUrl": "https://gitlab.com/harmoniya/modpacks/arcanetech/client/-/jobs/artifacts/main/raw/manifest.json?job=publish",
            "maintaining": false, "order": 1, "description": "# ArcaneTech",
            "banner": "banners/faf0cbfc-84db-44a4-94b7-639ea5be1e06.png",
            "options": [
              {"type":"slider","name":"xmx","title":"RAM","subtitle":"Скільки RAM виділяти грі",
               "min":2048,"max":16384,"step":1024,"unit":"МБ","placeholder":null,"default":"4096",
               "choices":null,"options":null},
              {"type":"feature","name":"potato_pc_fix","title":"Сумісність зі старим залізом","subtitle":null,
               "min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":"false",
               "choices":null,"options":null}
            ],
            "announcements": null,
            "project": {"id":"harmoniya","title":"Harmoniya","order":0,"logo":"logos/ef5a5670-2674-43b8-9f80-efef15e5550f.svg"},
            "servers": [{"title":"Main"}],
            "status": {"online": 1, "max": 100}
          },
          {
            "id": "upcoming", "title": "Upcoming", "summary": "", "version": "",
            "manifestUrl": "", "maintaining": false, "order": 2, "description": "",
            "banner": "", "options": [], "announcements": null,
            "project": {"id":"harmoniya","title":"Harmoniya","order":0,"logo":""},
            "servers": [],
            "status": {"online": 0, "max": 0}
          }
        ]
      }
    }"##;

    #[test]
    fn parses_live_response_shape() {
        let resp: GqlResponse = serde_json::from_str(RESPONSE).expect("parse response");
        let items: Vec<Modpack> =
            resp.data.unwrap().modpacks.into_iter().map(RawModpack::into_modpack).collect();
        assert_eq!(items.len(), 2);

        let m = &items[0];
        assert_eq!(m.id, "arcanetech");
        assert_eq!(
            m.banner.as_ref().and_then(|b| b.url.as_deref()),
            Some("https://one.harmoniya.net/files/banners/faf0cbfc-84db-44a4-94b7-639ea5be1e06.png")
        );
        assert_eq!(
            m.project.logo.url.as_deref(),
            Some("https://one.harmoniya.net/files/logos/ef5a5670-2674-43b8-9f80-efef15e5550f.svg")
        );
        assert!(m.status.is_some(), "has a server -> status carries the live ping result");
        assert_eq!(m.status.as_ref().unwrap().online, 1);
        assert_eq!(m.options.len(), 2);
    }

    #[test]
    fn empty_servers_means_no_status_and_blank_fields_become_none() {
        let resp: GqlResponse = serde_json::from_str(RESPONSE).expect("parse response");
        let items: Vec<Modpack> =
            resp.data.unwrap().modpacks.into_iter().map(RawModpack::into_modpack).collect();
        let m = &items[1];
        assert!(m.status.is_none(), "no servers configured -> no status, same as petal's null");
        assert!(m.banner.is_none());
        assert!(m.project.logo.url.is_none());
        assert_eq!(m.summary, None);
        assert_eq!(m.version, None);
        assert_eq!(m.description, None);
    }

    #[test]
    fn ranks_maintaining_below_normal_and_no_status_last() {
        let mut items: Vec<Modpack> = serde_json::from_str::<GqlResponse>(RESPONSE)
            .unwrap()
            .data
            .unwrap()
            .modpacks
            .into_iter()
            .map(RawModpack::into_modpack)
            .collect();
        // Both already have distinct `order` (1, 2), so the no-status rank only
        // kicks in as a tiebreaker; flip orders equal to actually exercise it.
        items[1].order = items[0].order;
        items.sort_by(|a, b| {
            let rank = |m: &Modpack| -> i32 {
                if m.maintaining { 1 } else if m.status.is_none() { 2 } else { 0 }
            };
            a.order.cmp(&b.order).then_with(|| rank(a).cmp(&rank(b)))
        });
        assert_eq!(items[0].id, "arcanetech");
        assert_eq!(items[1].id, "upcoming");
    }
}
