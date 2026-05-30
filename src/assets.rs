use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

/// The SVG icons under `assets/icons/`, embedded into the binary at build time.
/// GPUI resolves `img()`/`svg()` paths like `icons/rocket.svg` through this, so
/// dropping a new SVG into `assets/icons/` makes it available with no code change.
#[derive(RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*"]
struct Embedded;

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Embedded::get(path).map(|file| file.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Embedded::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.into_owned()))
            .collect())
    }
}
