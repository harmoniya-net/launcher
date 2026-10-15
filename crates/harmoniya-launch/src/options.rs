//! Per-modpack options — the "Налаштування модпаку" form.
//!
//! The schema is the bundle's: its head carries a tree of options (slider,
//! select, text, file, directory, and `feature`s that hold options of their
//! own), read off the front of the bundle by [`read_head`] before anything is
//! installed. What the player chose is the launcher's, stored sparse and keyed
//! by option name ([`ModpackOptions`]), and [`resolve`] turns the two into the
//! `vars` and enabled `features` an install takes.
//!
//! Binding: every option has a `name`. For a leaf it is the var it fills; for a
//! `feature` it is the feature it switches. A leaf resolves to
//! `saved ?? default`, writing the var when there is a value and omitting it
//! otherwise (the manifest's own default then wins). A feature that is off is
//! inert: its name isn't enabled and the vars of the options under it are
//! skipped.

use std::collections::HashMap;

use indexmap::IndexMap;
use opys_runtime::{InstallError, ManifestSource};
use serde::{Deserialize, Serialize};

pub use opys_bundle::{Choice, Head, Label, Localized, OptionDef, Options};

/// The player's saved choices for one modpack (sparse — only what they changed).
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ModpackOptions {
    #[serde(default)]
    pub vars: HashMap<String, String>,
    #[serde(default)]
    pub features: HashMap<String, bool>,
}

/// The head of the bundle at `url`: only the front of the file is asked for,
/// so this is one small range request, not a download of the modpack.
pub async fn read_head(url: &str) -> Result<Head, InstallError> {
    head_of(&ManifestSource::Url(url.to_owned())).await
}

/// The head of the bundle `source` names, its manifest left unread.
pub(crate) async fn head_of(source: &ManifestSource) -> Result<Head, InstallError> {
    opys_runtime::read_head(source)
        .await?
        // Only a manifest held in memory has none, and a modpack is a bundle.
        .ok_or_else(|| InstallError::Manifest("a modpack is installed from a bundle".into()))
}

/// Resolve the schema + saved choices into opys `vars` + enabled `features`.
pub fn resolve(schema: &Options, saved: &ModpackOptions) -> (IndexMap<String, String>, Vec<String>) {
    let mut vars = IndexMap::new();
    let mut features = Vec::new();
    apply(schema.as_slice(), saved, &mut vars, &mut features);
    (vars, features)
}

fn apply(
    options: &[OptionDef],
    saved: &ModpackOptions,
    vars: &mut IndexMap<String, String>,
    features: &mut Vec<String>,
) {
    for option in options {
        let (name, value) = match option {
            OptionDef::Feature { name, default, options, .. } => {
                if feature_on(saved, name, *default) {
                    features.push(name.clone());
                    apply(options, saved, vars, features);
                }
                continue;
            }
            OptionDef::Slider { name, default, .. } => {
                (name, Some(saved.vars.get(name).cloned().unwrap_or_else(|| fmt_num(*default))))
            }
            OptionDef::Select { name, default, .. } => {
                (name, Some(saved.vars.get(name).cloned().unwrap_or_else(|| default.clone())))
            }
            OptionDef::Text { name, default, .. } => {
                (name, saved.vars.get(name).cloned().or_else(|| default.clone()))
            }
            OptionDef::File { name, .. } | OptionDef::Directory { name, .. } => {
                (name, saved.vars.get(name).cloned())
            }
        };
        if let Some(v) = value.filter(|v| !v.is_empty()) {
            vars.insert(name.clone(), v);
        }
    }
}

/// Whether a feature is on: the player's choice, else the schema's default.
pub fn feature_on(saved: &ModpackOptions, name: &str, default: bool) -> bool {
    saved.features.get(name).copied().unwrap_or(default)
}

/// Format a slider number without a trailing `.0` for whole values.
pub fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The text to show a player whose UI is in `lang` (a language tag, `uk` or
/// `en`): that language, else the head's `locale`, else whatever there is.
/// A plain string is in the head's locale and is the only text there is.
pub fn localize<'a>(text: &'a Localized, lang: &str, locale: Option<&str>) -> &'a str {
    let map = match text {
        Localized::Plain(text) => return text,
        Localized::Translated(map) => map,
    };
    let primary = |tag: &str| tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
    let pick = |want: &str| {
        map.get(want).or_else(|| {
            let want = primary(want);
            map.iter().find(|(tag, _)| primary(tag) == want).map(|(_, text)| text)
        })
    };
    pick(lang)
        .or_else(|| locale.and_then(pick))
        .or_else(|| map.values().next())
        .map(String::as_str)
        // A translated text is never empty (it does not decode otherwise).
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A head as `opys build` writes it: one of each kind, a feature holding
    /// options of its own, and texts both plain and translated.
    const HEAD: &str = r#"{
      "format": 1,
      "locale": "en",
      "options": [
        {"slider": "xmx", "title": {"en": "Memory", "uk": "Пам'ять"}, "min": 1024, "max": 16384,
         "step": 512, "unit": "MB", "default": 4096},
        {"select": "renderer", "title": "Renderer", "default": "auto",
         "choices": [{"value": "auto", "label": "Auto"}, {"value": "gl", "label": "OpenGL"}]},
        {"text": "jvm_args", "title": "JVM", "placeholder": "-XX"},
        {"feature": "fullscreen", "title": "Fullscreen"},
        {"feature": "uuc_custom_username", "title": "Custom name", "default": true, "options": [
          {"text": "username", "title": "Name"},
          {"directory": "java_libs", "title": "Libraries"}
        ]}
      ]
    }"#;

    fn head() -> Head {
        serde_json::from_str(HEAD).expect("a head opys writes")
    }

    #[test]
    fn defaults_fill_vars_and_an_off_feature_is_inert() {
        let (vars, features) = resolve(&head().options, &ModpackOptions::default());
        assert_eq!(vars.get("xmx").map(String::as_str), Some("4096"));
        assert_eq!(vars.get("renderer").map(String::as_str), Some("auto"));
        // No default, nothing saved: left to the manifest.
        assert_eq!(vars.get("jvm_args"), None);
        assert_eq!(vars.get("username"), None);
        assert_eq!(features, ["uuc_custom_username"]);
    }

    #[test]
    fn saved_choices_win_and_a_feature_gates_what_it_holds() {
        let mut saved = ModpackOptions::default();
        saved.vars.insert("xmx".into(), "8192".into());
        saved.vars.insert("username".into(), "Steve".into());
        let (vars, features) = resolve(&head().options, &saved);
        assert_eq!(vars["xmx"], "8192");
        assert_eq!(vars["username"], "Steve");
        assert!(features.contains(&"uuc_custom_username".to_owned()));

        saved.features.insert("uuc_custom_username".into(), false);
        saved.features.insert("fullscreen".into(), true);
        let (vars, features) = resolve(&head().options, &saved);
        assert_eq!(vars.get("username"), None, "an off feature's options fill nothing");
        assert_eq!(features, ["fullscreen"]);
    }

    #[test]
    fn text_is_shown_in_the_ui_language_then_the_heads_then_any() {
        let head = head();
        let OptionDef::Slider { label, unit, .. } = &head.options.as_slice()[0] else {
            panic!("the first option is a slider");
        };
        let locale = head.locale.as_deref();
        assert_eq!(localize(&label.title, "uk", locale), "Пам'ять");
        assert_eq!(localize(&label.title, "en", locale), "Memory");
        assert_eq!(localize(&label.title, "en-GB", locale), "Memory");
        assert_eq!(localize(&label.title, "de", locale), "Memory", "falls back to the head's locale");
        assert_eq!(localize(unit.as_ref().unwrap(), "uk", locale), "MB", "a plain string is all there is");

        let only_uk: Localized = serde_json::from_str(r#"{"uk": "Тільки"}"#).unwrap();
        assert_eq!(localize(&only_uk, "en", Some("en")), "Тільки", "else whatever there is");
    }
}
