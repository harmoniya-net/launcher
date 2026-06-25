//! Stateless widget builders for the skin form (file field, model radios, action
//! button, reset link) and small helpers. Extracted from `skin_form.rs`.

use gpui::{
    FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, Window, div, px,
};

use harmoniya_api::services::yggdrasil::SkinModel;
use crate::theme::Theme;

/// Truncate from the start with a leading ellipsis so the *end* of the file
/// name (extension, suffix) remains visible: "long-filename.png" → "…name.png".
pub(crate) fn truncate_start(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars { return s.to_string(); }
    let kept: String = s.chars().skip(count - (max_chars - 1)).collect();
    format!("…{kept}")
}

pub(crate) fn file_field(
    label: &'static str,
    name: String,
    enabled: bool,
    on_pick: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let mut btn = div()
        .id(SharedString::from(format!("pick-{label}")))
        .flex_shrink_0()
        .px(px(16.))
        .h_full()
        .flex()
        .items_center()
        .bg(Theme::surface_raised())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(Theme::text())
        .child(crate::i18n::t().browse);
    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(|s| s.bg(Theme::surface_hover()))
            .on_mouse_down(MouseButton::Left, on_pick);
    } else {
        btn = btn.opacity(0.35);
    }
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .opacity(if enabled { 1.0 } else { 0.5 })
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .h(px(40.))
                .bg(Theme::bg())
                .rounded(Theme::radius_block())
                .overflow_hidden()
                .child(btn)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .px(px(14.))
                        .text_size(px(13.))
                        .text_color(Theme::text_faint())
                        .child(truncate_start(&name, 24)),
                ),
        )
        .into_any_element()
}

pub(crate) fn model_field(
    model: SkinModel,
    enabled: bool,
    on_classic: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
    on_slim: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child(crate::i18n::t().arm_model),
        )
        .opacity(if enabled { 1.0 } else { 0.5 })
        .child(
            div()
                .flex()
                .gap(px(24.))
                .child(radio(crate::i18n::t().model_classic, model == SkinModel::Classic, if enabled { Some(on_classic) } else { None }))
                .child(radio(crate::i18n::t().model_slim, model == SkinModel::Slim, if enabled { Some(on_slim) } else { None })),
        )
        .into_any_element()
}

pub(crate) fn radio(
    label: &'static str,
    selected: bool,
    on_click: Option<impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static>,
) -> gpui::AnyElement {
    let mut el = div()
        .id(SharedString::from(format!("radio-{label}")))
        .flex()
        .items_center()
        .gap(px(9.));
    if let Some(handler) = on_click {
        el = el.cursor_pointer().on_mouse_down(MouseButton::Left, handler);
    }
    el
        .child(
            div()
                .w(px(16.))
                .h(px(16.))
                .rounded_full()
                .border_2()
                .border_color(if selected { Theme::accent() } else { Theme::surface_raised() })
                .flex()
                .items_center()
                .justify_center()
                .child(if selected {
                    div().w(px(8.)).h(px(8.)).rounded_full().bg(Theme::accent()).into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if selected { Theme::text() } else { Theme::text_muted() })
                .child(label),
        )
        .into_any_element()
}

pub(crate) fn action_button(
    label: &'static str,
    enabled: bool,
    bg: gpui::Rgba,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let mut btn = div()
        .id(SharedString::from(format!("action-{label}")))
        .px(px(24.))
        .py(px(10.))
        .rounded(Theme::radius_block())
        .bg(bg)
        .text_color(Theme::on_accent())
        .font_weight(FontWeight::BOLD)
        .text_size(px(14.))
        .child(label);
    if enabled {
        btn = btn.cursor_pointer().on_mouse_down(MouseButton::Left, on_click);
    } else {
        btn = btn.opacity(0.4);
    }
    btn.into_any_element()
}

pub(crate) fn reset_link(
    label: &'static str,
    enabled: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let mut btn = div()
        .id(SharedString::from(format!("reset-{label}")))
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(Theme::text_faint())
        .child(label);
    if enabled {
        btn = btn
            .cursor_pointer()
            .hover(|s| s.text_color(Theme::status_offline()))
            .on_mouse_down(MouseButton::Left, on_click);
    } else {
        btn = btn.opacity(0.35);
    }
    btn.into_any_element()
}
