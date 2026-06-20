//! Regenerate `assets/logo.png` from `assets/logo.svg` (the source of truth).
//! Run after editing the logo SVG: `cargo run --example render_logo`.
//! `build.rs` turns the PNG into the Windows `.ico`; the tray/desktop icons
//! rasterize the SVG directly at runtime (see `src/logo.rs`).

use image::RgbaImage;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

fn main() {
    let svg = std::fs::read("assets/logo.svg").expect("read assets/logo.svg");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("parse svg");

    let size: u32 = 1024;
    let s = tree.size();
    let scale = size as f32 / s.width().max(s.height());
    let tx = (size as f32 - s.width() * scale) / 2.0;
    let ty = (size as f32 - s.height() * scale) / 2.0;

    let mut pixmap = Pixmap::new(size, size).expect("alloc pixmap");
    resvg::render(
        &tree,
        Transform::from_scale(scale, scale).post_translate(tx, ty),
        &mut pixmap.as_mut(),
    );

    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for px in pixmap.pixels() {
        let c = px.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    RgbaImage::from_raw(size, size, data)
        .expect("build image")
        .save("assets/logo.png")
        .expect("save assets/logo.png");
    println!("wrote assets/logo.png ({size}x{size})");
}
