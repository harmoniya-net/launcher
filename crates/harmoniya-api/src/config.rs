use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::PathBuf};

const QUALIFIER: &str = "net";
const ORG: &str = "harmoniya";
const APP: &str = "launcher";

pub fn project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(QUALIFIER, ORG, APP)
        .context("no home directory available for ProjectDirs")
}

pub fn config_path(name: &str) -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.config_dir();
    fs::create_dir_all(dir)?;
    Ok(dir.join(name))
}

pub fn data_path(name: &str) -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.data_dir();
    fs::create_dir_all(dir)?;
    Ok(dir.join(name))
}

pub fn logs_dir() -> Result<PathBuf> {
    let dirs = project_dirs()?;
    let dir = dirs.data_dir().join("logs");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn load_json<T: DeserializeOwned>(name: &str) -> Option<T> {
    let path = config_path(name).ok()?;
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Serialize and write a config file off the UI thread. Returns immediately —
/// the actual disk write happens on a background thread. Selection/settings
/// state can tolerate losing the last write on crash, so we skip fsync.
pub fn save_json<T: Serialize>(name: &str, value: &T) -> Result<()> {
    let path = config_path(name)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    std::thread::spawn(move || {
        if let Err(e) = write_atomic(&path, &bytes) {
            tracing::warn!("persistence write {path:?}: {e}");
        }
    });
    Ok(())
}

#[cfg(unix)]
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        // intentionally no sync_all — fsync blocks 10–100ms on busy disks
        // and we don't need crash durability for selection.json / settings.json.
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}
