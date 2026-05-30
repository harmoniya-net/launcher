use gpui::{hsla, px, rgb, Hsla, Pixels, Rgba, SharedString};

#[derive(Clone, Copy)]
pub struct Theme;

impl Theme {
    pub fn bg() -> Rgba {
        rgb(0x0e0d0f)
    }
    pub fn surface() -> Rgba {
        rgb(0x1b1a1d)
    }
    pub fn surface_raised() -> Rgba {
        rgb(0x3a383e)
    }

    pub fn text() -> Rgba {
        rgb(0xffffff)
    }
    pub fn text_secondary() -> Rgba {
        rgb(0xd0cfd2)
    }
    pub fn text_muted() -> Rgba {
        rgb(0xb9b9b9)
    }
    pub fn text_faint() -> Rgba {
        rgb(0x8d8a93)
    }

    pub fn accent() -> Rgba {
        rgb(0xff6699)
    }

    pub fn status_online() -> Rgba {
        rgb(0x4ade80)
    }
    pub fn status_offline() -> Rgba {
        rgb(0xff5d5d)
    }
    pub fn status_maintenance() -> Rgba {
        rgb(0xf59e0b)
    }

    pub fn overlay() -> Hsla {
        hsla(0.0, 0.0, 0.0, 0.65)
    }

    pub fn radius_panel() -> Pixels {
        px(6.)
    }
    pub fn radius_card() -> Pixels {
        px(3.)
    }
    pub fn radius_block() -> Pixels {
        px(3.)
    }

    pub fn font() -> SharedString {
        "Roboto".into()
    }
    pub fn font_fallback() -> SharedString {
        "Noto Sans".into()
    }
}
