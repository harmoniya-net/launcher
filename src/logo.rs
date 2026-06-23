//! The bundled Harmoniya logo (SVG), rasterized at the requested size for use
//! as the tray and app icons. Rendering from SVG keeps the mark crisp at any
//! size (the tray/desktop icon APIs want raster pixels, so we rasterize here).

use image::RgbaImage;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo.svg");

/// Rasterize the logo into a square `size`×`size` RGBA image (aspect-preserved,
/// centered). Falls back to a transparent square if rendering fails.
pub fn square(size: u32) -> RgbaImage {
    render(size).unwrap_or_else(|| RgbaImage::new(size, size))
}

/// Cropped logo as straight-alpha RGBA8 bytes at `size`×`size`.
pub fn rgba(size: u32) -> Vec<u8> {
    square(size).into_raw()
}

fn render(size: u32) -> Option<RgbaImage> {
    let tree = match usvg::Tree::from_data(LOGO_SVG, &usvg::Options::default()) {
        Ok(tree) => tree,
        Err(e) => {
            tracing::warn!(error = %harmoniya_api::obs::cause_chain(&e), "logo svg parse");
            return None;
        }
    };

    // Fit the SVG into the square, preserving aspect ratio and centering.
    let svg = tree.size();
    let scale = size as f32 / svg.width().max(svg.height());
    let tx = (size as f32 - svg.width() * scale) / 2.0;
    let ty = (size as f32 - svg.height() * scale) / 2.0;

    let mut pixmap = Pixmap::new(size, size)?;
    resvg::render(&tree, Transform::from_scale(scale, scale).post_translate(tx, ty), &mut pixmap.as_mut());

    // tiny-skia stores premultiplied alpha; the icon APIs want straight RGBA.
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(size, size, data)
}
