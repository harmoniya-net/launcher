//! A modpack installed end to end without the GUI or the network: a small
//! bundle is written the way `opys build` writes one, and the pipeline reads
//! its head, resolves saved options against it, installs it into the shared
//! data dir (the root; the pack keeps its game directory in a subdirectory of
//! its own, as published packs do) and says what it would run.

use std::path::Path;
use std::sync::{Arc, Mutex};

use harmoniya_launch::options::{ModpackOptions, OptionDef};
use harmoniya_launch::pipeline::{self, ManifestSource, Player};
use opys_bundle::{blob_id, write_bundle, BlobSource, Blobs, Head};
use opys_core::Manifest;
use opys_runtime::{InstallOptions, InstallProgress};

const MOD: &[u8] = b"a mod jar";
const OPTIONS_TXT: &[u8] = b"fancyGraphics:true\n";

const HEAD: &str = r#"{
  "format": 1,
  "locale": "en",
  "options": [
    {"slider": "xmx", "title": {"en": "Memory", "uk": "Пам'ять"}, "unit": "MB",
     "min": 2048, "max": 8192, "step": 512, "default": 4096},
    {"feature": "fancy", "title": {"en": "Fancy graphics", "uk": "Гарна графіка"}},
    {"feature": "custom_name", "title": "Custom name", "options": [
      {"text": "username", "title": "Name"}
    ]}
  ]
}"#;

fn manifest() -> Manifest {
    let manifest = serde_json::json!({
        "vars": { "game_directory": "${root}/testpack" },
        "artifacts": [
            { "path": "${game_directory}/mods/kept.jar", "source": { "blob": blob_id(MOD) } },
            {
                "path": "${game_directory}/options.txt",
                "source": { "blob": blob_id(OPTIONS_TXT) },
                "rules": "allow.features.fancy"
            }
        ],
        "cleanup": [{ "includes": ["${game_directory}/mods/**"] }],
        "launch": {
            "command": "java",
            "args": ["-Xmx${xmx}M", "--username", "${username}", "--uuid", "${uuid}"],
            "workdir": "${game_directory}"
        }
    });
    serde_json::from_value(manifest).expect("a manifest opys writes")
}

fn write_test_bundle(path: &Path) {
    let head: Head = serde_json::from_str(HEAD).expect("a head opys writes");
    let blobs: Blobs = [MOD, OPTIONS_TXT]
        .into_iter()
        .map(|bytes| (blob_id(bytes), BlobSource::Bytes(bytes.to_vec())))
        .collect();
    let file = std::fs::File::create(path).unwrap();
    write_bundle(file, &head, &manifest(), &blobs).expect("bundle written");
}

fn player() -> Player {
    Player { username: "Steve".into(), uuid: "0123abcd".into(), token: "jwt".into() }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap()
}

#[test]
fn a_bundle_installs_into_its_root_with_the_players_options() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("client.opys");
    write_test_bundle(&bundle);
    let data = dir.path().join("data");
    let root_str = data.to_string_lossy().into_owned();
    let root = data.join("testpack");

    // Something the pack no longer has, which its cleanup rule sweeps.
    std::fs::create_dir_all(root.join("mods")).unwrap();
    std::fs::write(root.join("mods/stale.jar"), b"old").unwrap();

    let mut saved = ModpackOptions::default();
    saved.vars.insert("xmx".into(), "6144".into());
    saved.features.insert("fancy".into(), true);

    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut install = InstallOptions::new();
    install.on_progress = Some({
        let seen = Arc::clone(&seen);
        Arc::new(move |p: InstallProgress| seen.lock().unwrap().push(format!("{p:?}")))
    });

    let spec = runtime()
        .block_on(pipeline::prepare(
            ManifestSource::Bundle(bundle),
            &root_str,
            &player(),
            &saved,
            install,
        ))
        .expect("installed");

    assert_eq!(std::fs::read(root.join("mods/kept.jar")).unwrap(), MOD);
    assert_eq!(std::fs::read(root.join("options.txt")).unwrap(), OPTIONS_TXT, "the feature was on");
    assert!(!root.join("mods/stale.jar").exists(), "cleanup removed the stale mod");

    assert_eq!(spec.command, "java");
    assert_eq!(spec.args, ["-Xmx6144M", "--username", "Steve", "--uuid", "0123abcd"]);
    assert!(Path::new(&spec.workdir).starts_with(&root));

    let seen = seen.lock().unwrap().join("\n");
    for phase in ["Resolve", "Download", "Verify", "Cleanup"] {
        assert!(seen.contains(phase), "progress reports {phase}:\n{seen}");
    }
}

#[test]
fn an_option_may_override_a_base_var_and_an_off_feature_installs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("client.opys");
    write_test_bundle(&bundle);
    let data = dir.path().join("data");
    let root = data.join("testpack");

    let mut saved = ModpackOptions::default();
    saved.features.insert("custom_name".into(), true);
    saved.vars.insert("username".into(), "Alex".into());

    let spec = runtime()
        .block_on(pipeline::prepare(
            ManifestSource::Bundle(bundle),
            &data.to_string_lossy(),
            &player(),
            &saved,
            InstallOptions::new(),
        ))
        .expect("installed");

    assert_eq!(spec.args, ["-Xmx4096M", "--username", "Alex", "--uuid", "0123abcd"]);
    assert!(!root.join("options.txt").exists(), "`fancy` is off by default");
}

#[test]
fn the_head_is_read_without_the_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let bundle = dir.path().join("client.opys");
    write_test_bundle(&bundle);

    let head = runtime()
        .block_on(opys_runtime::read_head(&ManifestSource::Bundle(bundle)))
        .unwrap()
        .expect("a bundle has a head");
    let names: Vec<&str> = head.options.as_slice().iter().map(OptionDef::name).collect();
    assert_eq!(names, ["xmx", "fancy", "custom_name"]);
}
