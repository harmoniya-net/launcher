//! Stateless control builders for the per-modpack settings form (slider, select,
//! path, step button, field card). Extracted from `settings_modal.rs`.

use gpui::{
    div, px, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, SharedString, Styled,
};

use harmoniya_api::services::options::{self, Choice, ModpackOptions};
use crate::state::AppState;
use crate::theme::Theme;

pub(crate) fn header_text(title: &str, subtitle: Option<&str>) -> gpui::Div {
    let mut col = div().flex().flex_col().gap(px(2.)).child(
        div()
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(Theme::text())
            .child(title.to_string()),
    );
    if let Some(sub) = subtitle {
        col = col.child(div().text_size(px(12.)).text_color(Theme::text_faint()).child(sub.to_string()));
    }
    col
}

pub(crate) fn field_card(title: &str, subtitle: Option<&str>, enabled: bool, control: gpui::AnyElement) -> gpui::AnyElement {
    let card = div().flex().flex_col().gap(px(8.)).child(header_text(title, subtitle)).child(control);
    if enabled { card.into_any_element() } else { card.opacity(0.4).into_any_element() }
}

pub(crate) fn step_btn(
    handle: &Entity<AppState>,
    modpack_id: &str,
    name: &str,
    tag: &str,
    glyph: &'static str,
    target: f64,
    enabled: bool,
) -> gpui::AnyElement {
    let base = div()
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .w(px(28.))
        .h(px(28.))
        .rounded(Theme::radius_block())
        .bg(Theme::surface_raised())
        .text_color(Theme::text())
        .text_size(px(16.))
        .child(glyph);
    if !enabled {
        return base.into_any_element();
    }
    let h = handle.clone();
    let mid = modpack_id.to_string();
    let nm = name.to_string();
    base.id(SharedString::from(format!("st-{name}-{tag}")))
        .cursor_pointer()
        .hover(|s| s.bg(Theme::surface_hover()))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            h.update(cx, |s, cx| s.set_option_value(mid.clone(), nm.clone(), Some(options::fmt_num(target)), cx));
        })
        .into_any_element()
}

pub(crate) fn select_control(
    handle: &Entity<AppState>,
    modpack_id: &str,
    name: &str,
    choices: &[Choice],
    default: &str,
    saved: &ModpackOptions,
    enabled: bool,
) -> gpui::AnyElement {
    let current = saved.vars.get(name).cloned().unwrap_or_else(|| default.to_string());
    let mut row = div().flex().flex_wrap().gap(px(8.));
    for c in choices {
        let active = c.value == current;
        let chip = div()
            .px(px(14.))
            .py(px(7.))
            .rounded(Theme::radius_block())
            .text_size(px(13.))
            .bg(if active { Theme::accent() } else { Theme::surface_raised() })
            .text_color(if active { Theme::on_accent() } else { Theme::text() })
            .child(c.label.clone());
        let chip = if enabled {
            let h = handle.clone();
            let mid = modpack_id.to_string();
            let nm = name.to_string();
            let val = c.value.clone();
            chip.id(SharedString::from(format!("sel-{name}-{}", c.value)))
                .cursor_pointer()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    h.update(cx, |s, cx| s.set_option_value(mid.clone(), nm.clone(), Some(val.clone()), cx));
                })
                .into_any_element()
        } else {
            chip.into_any_element()
        };
        row = row.child(chip);
    }
    row.into_any_element()
}

pub(crate) fn path_control(
    handle: &Entity<AppState>,
    modpack_id: &str,
    name: &str,
    dir: bool,
    saved: &ModpackOptions,
    enabled: bool,
) -> gpui::AnyElement {
    let current = saved.vars.get(name).cloned();
    let pick = {
        let base = div()
            .flex_shrink_0()
            .px(px(14.))
            .h(px(40.))
            .flex()
            .items_center()
            .bg(Theme::surface_raised())
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(Theme::text())
            .child(crate::i18n::t().pick);
        if enabled {
            let h = handle.clone();
            let mid = modpack_id.to_string();
            let nm = name.to_string();
            base.id(SharedString::from(format!("path-{name}")))
                .cursor_pointer()
                .hover(|s| s.bg(Theme::surface_hover()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let b = native_dialog::DialogBuilder::file();
                    let result = if dir { b.open_single_dir().show() } else { b.open_single_file().show() };
                    if let Ok(Some(path)) = result {
                        let p = path.to_string_lossy().to_string();
                        h.update(cx, |s, cx| s.set_option_value(mid.clone(), nm.clone(), Some(p), cx));
                    }
                })
                .into_any_element()
        } else {
            base.into_any_element()
        }
    };

    let mut row = div()
        .flex()
        .items_center()
        .h(px(40.))
        .bg(Theme::bg())
        .rounded(Theme::radius_block())
        .overflow_hidden()
        .child(pick)
        .child(
            div()
                .flex_1()
                .px(px(14.))
                .text_size(px(13.))
                .text_color(Theme::text_faint())
                .child(current.clone().unwrap_or_else(|| crate::i18n::t().not_set.into())),
        );
    if enabled && current.is_some() {
        let h = handle.clone();
        let mid = modpack_id.to_string();
        let nm = name.to_string();
        row = row.child(
            div()
                .id(SharedString::from(format!("path-{name}-reset")))
                .flex_shrink_0()
                .px(px(12.))
                .h_full()
                .flex()
                .items_center()
                .text_size(px(12.))
                .text_color(Theme::text_faint())
                .cursor_pointer()
                .hover(|s| s.text_color(Theme::text()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    h.update(cx, |s, cx| s.set_option_value(mid.clone(), nm.clone(), None, cx));
                })
                .child(crate::i18n::t().reset),
        );
    }
    row.into_any_element()
}
