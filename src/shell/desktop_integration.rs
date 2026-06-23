//! Linux desktop integration: install a `.desktop` entry + hicolor icons so the
//! window manager, taskbars (e.g. waybar) and launchers show the Harmoniya logo
//! for the app, matched by `app_id` (`net.harmoniya.launcher`).
//!
//! GPUI exposes no runtime window icon on Wayland, so this is how the app itself
//! gets an icon there. Best-effort and idempotent — rewritten each launch so the
//! `Exec` path stays correct across self-updates.

/// Install/refresh desktop integration (Linux only; no-op elsewhere).
pub fn ensure() {
    #[cfg(target_os = "linux")]
    std::thread::spawn(|| {
        if let Err(e) = linux::install() {
            tracing::warn!(error = %harmoniya_api::obs::Chain(&e), "desktop integration");
        }
    });
}

#[cfg(target_os = "linux")]
mod linux {
    use std::fs;

    use anyhow::Context;

    const APP_ID: &str = "net.harmoniya.launcher";

    pub fn install() -> anyhow::Result<()> {
        let dirs = directories::BaseDirs::new().context("no base dirs")?;
        let data = dirs.data_dir(); // ~/.local/share

        // Install the logo at a few sizes into the hicolor icon theme.
        let mut icon_256 = None;
        for size in [256u32, 128, 64] {
            let dir = data.join(format!("icons/hicolor/{size}x{size}/apps"));
            fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{APP_ID}.png"));
            crate::logo::square(size)
                .save(&path)
                .with_context(|| format!("save {}", path.display()))?;
            if size == 256 {
                icon_256 = Some(path);
            }
        }

        // Desktop entry. Absolute Icon path so no icon-cache refresh is needed.
        let exe = std::env::current_exe()?;
        let icon = icon_256.map(|p| p.display().to_string()).unwrap_or_else(|| APP_ID.into());
        let apps = data.join("applications");
        fs::create_dir_all(&apps)?;
        let desktop = apps.join(format!("{APP_ID}.desktop"));
        let contents = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=Harmoniya\n\
             Comment=Harmoniya game launcher\n\
             Exec={exe}\n\
             Icon={icon}\n\
             Terminal=false\n\
             Categories=Game;\n\
             StartupWMClass={APP_ID}\n",
            exe = exe.display(),
        );
        fs::write(&desktop, contents).with_context(|| format!("write {}", desktop.display()))?;
        Ok(())
    }
}
