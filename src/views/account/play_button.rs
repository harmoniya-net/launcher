//! The hero's primary play/stop button (slanted SVG tab + label overlay) and
//! its three visual states. Extracted from `hero.rs`.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, AnyElement, Div, FontWeight, Hsla, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Svg, Window, div, px, rgb, svg,
};

use crate::theme::Theme;
use crate::widgets::icon::icon;

// The play button is the page's primary action: a minimal slanted (parallelogram)
// tab, drawn via an SVG shape since GPUI divs can't skew, with the label overlaid.
const BTN_W: f32 = 200.;
const BTN_H: f32 = 52.;
const HOVER_GREEN: u32 = 0x3cb371;
const STOP_BG: u32 = 0xf25c63;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayState {
    Online,
    Offline,
    Maintenance,
    Unauthenticated,
}

/// The parallelogram fill, colored via `currentColor`; stretches to the button.
fn btn_shape(fill: impl Into<Hsla>) -> Svg {
    svg()
        .path("icons/btn-shape.svg")
        .text_color(fill)
        .absolute()
        .inset_0()
        .size_full()
}

/// The centered icon + label overlay.
fn btn_content(icon_svg: Option<&'static str>, label: &str, text: impl Into<Hsla>) -> Div {
    let text = text.into();
    let mut row = div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .font_weight(FontWeight::BOLD)
        .text_size(px(15.))
        .text_color(text);
    if let Some(svg) = icon_svg {
        row = row.child(icon(svg, 13., text));
    }
    row.child(label.to_string())
}

/// Build the play/stop button in one of its three visual states.
///
/// `on_hover` is attached only to the playable variant (it drives the green
/// reveal); `on_click` is attached to the running + playable variants (the
/// disabled variant has neither).
pub(crate) fn play_button(
    play_state: PlayState,
    running: bool,
    label: &str,
    play_hovered: bool,
    hover_seq: usize,
    on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let play_disabled = play_state != PlayState::Online;
    if running {
        // Running — offer to stop: red background, white label/icon.
        div()
            .id("play")
            .relative()
            .flex_shrink_0()
            .w(px(BTN_W))
            .h(px(BTN_H))
            .cursor_pointer()
            .hover(|s| s.opacity(0.9))
            .child(btn_shape(rgb(STOP_BG)))
            .child(btn_content(Some("icons/power.svg"), "Зупинити", Theme::text()))
            .on_mouse_down(MouseButton::Left, on_click)
            .into_any_element()
    } else if !play_disabled {
        // Playable — white tab with dark label; on hover a green layer (green
        // shape + white label, in sync) fades in over the top.
        let mut btn = div()
            .id("play")
            .relative()
            .flex_shrink_0()
            .w(px(BTN_W))
            .h(px(BTN_H))
            .cursor_pointer()
            .on_hover(on_hover)
            .child(btn_shape(rgb(0xffffff)))
            .child(btn_content(Some("icons/play.svg"), label, Theme::on_accent()));
        if play_hovered {
            // A clip box whose width animates 0 → full, revealing a fixed-size
            // green layer (green shape + white label) from left to right.
            let green = div()
                .absolute()
                .left(px(0.))
                .top(px(0.))
                .h(px(BTN_H))
                .overflow_hidden()
                .child(
                    div()
                        .flex_shrink_0()
                        .relative()
                        .w(px(BTN_W))
                        .h(px(BTN_H))
                        .child(btn_shape(rgb(HOVER_GREEN)))
                        .child(btn_content(Some("icons/play.svg"), label, Theme::text())),
                )
                .with_animation(
                    (SharedString::from("play-hover-green"), hover_seq),
                    Animation::new(Duration::from_millis(260)).with_easing(|t| {
                        let u = 1.0 - t;
                        1.0 - u * u * u
                    }),
                    |el, t| el.w(px(BTN_W * t)),
                );
            btn = btn.child(green);
        }
        btn.on_mouse_down(MouseButton::Left, on_click).into_any_element()
    } else {
        // Disabled: a muted (panel-tone) tab with status-colored label.
        let color = match play_state {
            PlayState::Maintenance => Theme::status_maintenance(),
            PlayState::Offline => Theme::status_offline(),
            _ => Theme::text_faint(),
        };
        div()
            .relative()
            .flex_shrink_0()
            .w(px(BTN_W))
            .h(px(BTN_H))
            .child(btn_shape(Theme::surface()))
            .child(btn_content(None, label, color))
            .into_any_element()
    }
}
