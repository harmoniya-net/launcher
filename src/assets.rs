use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

/// Assets embedded into the binary at build time: the SVG icons under
/// `assets/icons/` and the raster images under `assets/images/`. GPUI resolves
/// `img()`/`svg()` paths like `icons/rocket.svg` or `images/hero.png` through
/// this, so dropping a file into either folder makes it available with no code
/// change.
#[derive(RustEmbed)]
#[folder = "assets/"]
#[include = "icons/*"]
#[include = "images/*"]
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
