//! Renders a Minecraft player head from a raw skin texture.
//!
//! Mirrors the web launcher's MinecraftHead canvas trick:
//! - Head face lives at (fw, fw) of size (fw, fw) in the texture, where
//!   `fw = w/8` — same on standard (64×64), legacy (64×32), and HD layouts.
//! - Hat overlay at (5·fw, fw) of size (fw, fw) — only when the texture is
//!   square (modern 64×64 or square HD; legacy 64×32 has no hat region).
//! - Head is drawn inset by ~6% so the hat layer appears as a slightly larger
//!   shell, giving the recognisable 3D depth.

use std::io::Cursor;

use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageFormat, RgbaImage, imageops};

/// Render the head face (with hat overlay) as a PNG. Output size scales with
/// the source resolution so HD skins keep their extra detail: every source
/// texel maps to a fixed 7×7 block of output pixels.
pub fn render(png_bytes: &[u8]) -> Result<Vec<u8>> {
    let img = image::load_from_memory(png_bytes).context("decode skin texture")?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || w % 8 != 0 || h % 8 != 0 {
        anyhow::bail!("unexpected skin dimensions: {w}x{h}");
    }

    // Face cell is always (w/8) × (w/8) — both for HD modern and HD legacy.
    let fw = w / 8;
    let size = fw * 8;        // 64 on standard, 128 on HD ×2, 256 on HD ×4, …
    let inset = fw / 2;       // 4 on standard, 8 on HD ×2, …
    let head_size = size - 2 * inset;

    let head = img.crop_imm(fw, fw, fw, fw);
    let head_scaled = imageops::resize(&head.to_rgba8(), head_size, head_size, imageops::FilterType::Nearest);

    let mut canvas: RgbaImage = RgbaImage::new(size, size);
    imageops::overlay(&mut canvas, &head_scaled, inset as i64, inset as i64);

    if w == h {
        let hat = img.crop_imm(5 * fw, fw, fw, fw);
        let hat_scaled = imageops::resize(&hat.to_rgba8(), size, size, imageops::FilterType::Nearest);
        imageops::overlay(&mut canvas, &hat_scaled, 0, 0);
    }

    let mut out = Vec::new();
    DynamicImage::ImageRgba8(canvas)
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .context("encode head PNG")?;
    Ok(out)
}
