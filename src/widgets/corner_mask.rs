//! Fake rounded corners for Cover-fit images. gpui can't clip an image to a
//! rounded rect — its content mask is rectangle-only and a `Cover` image paints
//! an oversized quad — so we overlay four small `bg`-coloured concave PNGs at the
//! panel's corners. Add them as the last children of a `relative` panel whose own
//! background is `rounded(radius)`. The PNGs are tinted to `Theme::bg`, matching
//! the area around the panel, so the masked corners read as rounded.

use gpui::{img, px, Img, Pixels, Styled};

/// The four corner overlays, each sized to `radius` and pinned to a corner.
/// Spread into a panel with `.children(corner_masks(r))`.
pub fn corner_masks(radius: Pixels) -> [Img; 4] {
    [
        img("images/corner-tl.png").absolute().w(radius).h(radius).top(px(0.)).left(px(0.)),
        img("images/corner-tr.png").absolute().w(radius).h(radius).top(px(0.)).right(px(0.)),
        img("images/corner-bl.png").absolute().w(radius).h(radius).bottom(px(0.)).left(px(0.)),
        img("images/corner-br.png").absolute().w(radius).h(radius).bottom(px(0.)).right(px(0.)),
    ]
}
