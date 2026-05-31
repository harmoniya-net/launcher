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
    /// Foreground (text/icon) drawn on top of accent or other light fills.
    pub fn on_accent() -> Rgba {
        rgb(0x0e0d0f)
    }
    /// Hover background for raised, interactive rows and buttons.
    pub fn surface_hover() -> Rgba {
        rgb(0x4a4850)
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
        px(12.)
    }
    pub fn radius_card() -> Pixels {
        px(8.)
    }
    pub fn radius_block() -> Pixels {
        px(6.)
    }

    // ── Layout — shared so every screen lines up to the same grid ──
    /// Outer padding of a full screen (account, skin, launcher).
    pub fn screen_pad() -> Pixels {
        px(24.)
    }
    /// Gap between the sidebar column and the main panel.
    pub fn panel_gap() -> Pixels {
        px(16.)
    }
    /// Width of the left sidebar column (nav/list + profile).
    pub fn sidebar_width() -> Pixels {
        px(448.)
    }
    /// Gap between the sidebar's top section and the profile card below it.
    pub fn sidebar_gap() -> Pixels {
        px(12.)
    }

    /// The one bundled UI family. Google's static Inter carries the optical size
    /// in the family name, so the 18pt set (right for UI text) is "Inter 18pt".
    pub fn font() -> SharedString {
        "Inter 18pt".into()
    }
}
