//! "Update launcher": the running version plus a button that updates right
//! away (`AppState::update_now`). Shown on the login screen and in the
//! Launcher settings tab, for launchers that sat in the tray past a release.

use gpui::{
    div, px, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled,
};

use crate::i18n;
use crate::state::{AppState, ManualUpdate};
use crate::theme::Theme;

fn version() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

/// What to show in place of the button label while an update runs or after
/// it finished; `None` means the button is idle and clickable.
fn status(manual_update: &ManualUpdate) -> Option<&'static str> {
    let t = i18n::t();
    match manual_update {
        ManualUpdate::Idle => None,
        ManualUpdate::Checking => Some(t.update_checking),
        ManualUpdate::UpToDate => Some(t.update_up_to_date),
        ManualUpdate::Busy => Some(t.update_busy),
        ManualUpdate::Failed => Some(t.update_failed),
    }
}

/// Login footer: `v0.7.2 · Update launcher`, a quiet text link.
pub fn update_link(state: &Entity<AppState>, manual_update: &ManualUpdate) -> gpui::AnyElement {
    let handle = state.clone();
    let label = status(manual_update).unwrap_or(i18n::t().update_now);
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .text_color(Theme::text_faint())
        .child(version())
        .child("·")
        .child(
            div()
                .id("update-link")
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::accent()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    handle.update(cx, |s, cx| s.update_now(cx));
                })
                .child(label),
        )
        .into_any_element()
}

/// Settings: a button followed by the version and the last outcome.
pub fn update_row(state: &Entity<AppState>, manual_update: &ManualUpdate) -> gpui::AnyElement {
    let handle = state.clone();
    let outcome = status(manual_update);
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .id("update-now")
                .flex_shrink_0()
                .px(px(16.))
                .h(px(40.))
                .flex()
                .items_center()
                .bg(Theme::surface_raised())
                .rounded(Theme::radius_block())
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::text())
                .cursor_pointer()
                .hover(|s| s.bg(Theme::surface_hover()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    handle.update(cx, |s, cx| s.update_now(cx));
                })
                .child(i18n::t().update_now),
        )
        .child(
            div()
                .text_size(px(13.))
                .text_color(Theme::text_faint())
                .child(match outcome {
                    Some(outcome) => format!("{} · {outcome}", version()),
                    None => version(),
                }),
        )
        .into_any_element()
}
