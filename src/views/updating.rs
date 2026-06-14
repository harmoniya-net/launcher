//! The full-window auto-update screen, rendered by `Root` in place of the normal
//! UI while a new release downloads and the app relaunches itself.

use std::time::Duration;

use gpui::{
    div, ease_in_out, px, Animation, AnimationExt, AnyElement, FontWeight, IntoElement,
    ParentElement, SharedString, Styled,
};

use crate::state::UpdatePhase;
use crate::theme::Theme;

const BAR_W: f32 = 280.;
const SEG_W: f32 = 96.;

pub(crate) fn updating_view(phase: &UpdatePhase) -> AnyElement {
    let t = crate::i18n::t();
    let (title, subtitle) = match phase {
        UpdatePhase::Downloading(version) => (t.updating_title, crate::i18n::downloading_version(version)),
        UpdatePhase::Restarting => (t.update_done, t.restarting.to_string()),
    };

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(22.))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::text())
                        .child(title),
                )
                .child(div().text_size(px(13.)).text_color(Theme::text_muted()).child(subtitle)),
        )
        .child(indeterminate_bar())
        .into_any_element()
}

/// A looping indeterminate progress bar: an accent segment slides across a fixed
/// track, since the self-updater doesn't report byte-level progress.
fn indeterminate_bar() -> AnyElement {
    div()
        .relative()
        .w(px(BAR_W))
        .h(px(4.))
        .rounded_full()
        .bg(Theme::surface_raised())
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .top(px(0.))
                .h(px(4.))
                .w(px(SEG_W))
                .rounded_full()
                .bg(Theme::accent())
                .with_animation(
                    SharedString::from("update-bar"),
                    Animation::new(Duration::from_millis(1100)).repeat().with_easing(ease_in_out),
                    |el, t| el.left(px(-SEG_W + (BAR_W + SEG_W) * t)),
                ),
        )
        .into_any_element()
}
