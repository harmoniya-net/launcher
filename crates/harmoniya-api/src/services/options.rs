//! Per-modpack options — the "Налаштування модпаку" form. A schema (the modpack's
//! `options` field, served by the petal CMS) of typed fields that the user fills
//! in, resolved into opys `vars` + enabled `features` at launch.
//!
//! Binding: every field has a `name`. For leaf fields it's the var key; for a
//! `feature` field it's the feature name. Resolution: `override ?? default`,
//! writing the var if a value exists, else omitting it (manifest default wins).
//! A disabled feature is fully inert — its name isn't enabled and its child
//! options' vars are skipped.

use std::collections::HashMap;

use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Choice {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Field {
    Slider {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
        min: f64,
        max: f64,
        step: f64,
        // CMS sends `default` as a string; accept a number too.
        #[serde(deserialize_with = "de_f64")]
        default: f64,
        #[serde(default)]
        unit: Option<String>,
    },
    Select {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
        choices: Vec<Choice>,
        default: String,
    },
    Text {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
        #[serde(default)]
        placeholder: Option<String>,
        #[serde(default)]
        default: Option<String>,
    },
    File {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
    },
    Directory {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
    },
    Feature {
        name: String,
        title: String,
        #[serde(default)]
        subtitle: Option<String>,
        // CMS sends `default` as the string "true"/"false".
        #[serde(default, deserialize_with = "de_bool")]
        default: bool,
        #[serde(default)]
        options: Vec<Field>,
    },
}

impl Field {
    pub fn name(&self) -> &str {
        match self {
            Field::Slider { name, .. }
            | Field::Select { name, .. }
            | Field::Text { name, .. }
            | Field::File { name, .. }
            | Field::Directory { name, .. }
            | Field::Feature { name, .. } => name,
        }
    }
    pub fn title(&self) -> &str {
        match self {
            Field::Slider { title, .. }
            | Field::Select { title, .. }
            | Field::Text { title, .. }
            | Field::File { title, .. }
            | Field::Directory { title, .. }
            | Field::Feature { title, .. } => title,
        }
    }
    pub fn subtitle(&self) -> Option<&str> {
        match self {
            Field::Slider { subtitle, .. }
            | Field::Select { subtitle, .. }
            | Field::Text { subtitle, .. }
            | Field::File { subtitle, .. }
            | Field::Directory { subtitle, .. }
            | Field::Feature { subtitle, .. } => subtitle.as_deref(),
        }
    }
}

/// The user's saved choices for one modpack (sparse — only what they changed).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ModpackOptions {
    #[serde(default)]
    pub vars: HashMap<String, String>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

/// Accept a number or a numeric string (the CMS serves all defaults as strings).
fn de_f64<'de, D>(d: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = f64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a number or numeric string")
        }
        fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<f64, E> {
            Ok(v)
        }
        fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<f64, E> {
            Ok(v as f64)
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<f64, E> {
            v.trim().parse().map_err(E::custom)
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<f64, E> {
            self.visit_str(&v)
        }
    }
    d.deserialize_any(V)
}

/// Accept a bool or a `"true"`/`"false"` string.
fn de_bool<'de, D>(d: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct V;
    impl<'de> serde::de::Visitor<'de> for V {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a bool or \"true\"/\"false\" string")
        }
        fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<bool, E> {
            Ok(v)
        }
        fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<bool, E> {
            Ok(matches!(v.trim(), "true" | "1" | "yes"))
        }
        fn visit_string<E: serde::de::Error>(self, v: String) -> Result<bool, E> {
            self.visit_str(&v)
        }
    }
    d.deserialize_any(V)
}

/// Resolve the schema + saved choices into opys `vars` + enabled `features`.
pub fn resolve(schema: &[Field], saved: &ModpackOptions) -> (IndexMap<String, String>, Vec<String>) {
    let mut vars = IndexMap::new();
    let mut features = Vec::new();
    for field in schema {
        match field {
            Field::Feature { name, default, options, .. } => {
                let enabled = saved.features.get(name).copied().unwrap_or(*default);
                if enabled {
                    features.push(name.clone());
                    for child in options {
                        apply_leaf(child, saved, &mut vars);
                    }
                }
            }
            leaf => apply_leaf(leaf, saved, &mut vars),
        }
    }
    (vars, features)
}

fn apply_leaf(field: &Field, saved: &ModpackOptions, vars: &mut IndexMap<String, String>) {
    let (name, value): (&str, Option<String>) = match field {
        Field::Slider { name, default, .. } => {
            (name, Some(saved.vars.get(name).cloned().unwrap_or_else(|| fmt_num(*default))))
        }
        Field::Select { name, default, .. } => {
            (name, Some(saved.vars.get(name).cloned().unwrap_or_else(|| default.clone())))
        }
        Field::Text { name, default, .. } => {
            (name, saved.vars.get(name).cloned().or_else(|| default.clone()))
        }
        Field::File { name, .. } | Field::Directory { name, .. } => {
            (name, saved.vars.get(name).cloned())
        }
        Field::Feature { .. } => return,
    };
    if let Some(v) = value {
        if !v.is_empty() {
            vars.insert(name.to_string(), v);
        }
    }
}

/// Format a slider number without a trailing `.0` for whole values.
pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shape petal serves: a "fat" object per option with all fields
    /// present (null when irrelevant) and `default` always a string.
    const CMS_OPTIONS: &str = r#"[
      {"type":"slider","name":"xmx","title":"RAM","subtitle":null,"min":1024,"max":16384,"step":512,"unit":"МБ","placeholder":null,"default":"4096","choices":[],"options":[]},
      {"type":"select","name":"renderer","title":"R","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":"auto","choices":[{"label":"Авто","value":"auto"},{"label":"GL","value":"gl"}],"options":[]},
      {"type":"text","name":"jvm_args","title":"JVM","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":"-XX","default":"","choices":[],"options":[]},
      {"type":"feature","name":"fullscreen","title":"FS","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":"false","choices":[],"options":[]},
      {"type":"feature","name":"custom_java","title":"CJ","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":"true","choices":[],"options":[
        {"type":"file","name":"java_home","title":"JH","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":null,"choices":[],"options":[]},
        {"type":"directory","name":"java_libs","title":"JL","subtitle":null,"min":null,"max":null,"step":null,"unit":null,"placeholder":null,"default":null,"choices":[],"options":[]}
      ]}
    ]"#;

    #[test]
    fn parses_cms_fat_object_shape() {
        let schema: Vec<Field> = serde_json::from_str(CMS_OPTIONS).expect("parse CMS options");
        assert_eq!(schema.len(), 5);

        // String default deserializes into f64; unit survives.
        match &schema[0] {
            Field::Slider { default, unit, min, .. } => {
                assert_eq!(*default, 4096.0);
                assert_eq!(*min, 1024.0);
                assert_eq!(unit.as_deref(), Some("МБ"));
            }
            other => panic!("expected slider, got {other:?}"),
        }
        // Feature with "true" string default + nested leaf options.
        match &schema[4] {
            Field::Feature { name, default, options, .. } => {
                assert_eq!(name, "custom_java");
                assert!(*default);
                assert_eq!(options.len(), 2);
                assert!(matches!(options[0], Field::File { .. }));
                assert!(matches!(options[1], Field::Directory { .. }));
            }
            other => panic!("expected feature, got {other:?}"),
        }
    }

    #[test]
    fn resolves_defaults_with_feature_gating() {
        let schema: Vec<Field> = serde_json::from_str(CMS_OPTIONS).unwrap();
        let (vars, features) = resolve(&schema, &ModpackOptions::default());

        // Slider/select fall back to defaults; empty text is omitted.
        assert_eq!(vars.get("xmx").map(String::as_str), Some("4096"));
        assert_eq!(vars.get("renderer").map(String::as_str), Some("auto"));
        assert_eq!(vars.get("jvm_args"), None);

        // Feature default "true" → enabled; "false" → inert.
        assert!(features.contains(&"custom_java".to_string()));
        assert!(!features.contains(&"fullscreen".to_string()));
    }
}
