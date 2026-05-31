//! Stateless control builders for the per-modpack settings form (slider, select,
//! path, step button, field card). Extracted from `settings_modal.rs`.

use gpui::{
    Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Styled, div, px, relative,
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn slider_control(
    handle: &Entity<AppState>,
    modpack_id: &str,
    name: &str,
    min: f64,
    max: f64,
    step: f64,
    default: f64,
    unit: Option<&str>,
    saved: &ModpackOptions,
    enabled: bool,
) -> gpui::AnyElement {
    let cur = saved.vars.get(name).and_then(|s| s.parse::<f64>().ok()).unwrap_or(default);
    let frac = if max > min { ((cur - min) / (max - min)).clamp(0., 1.) as f32 } else { 0. };
    let unit_suffix = unit.map(|u| format!(" {u}")).unwrap_or_default();
    let sign = |n: f64| {
        div()
            .text_size(px(11.))
            .text_color(Theme::text_faint())
            .child(format!("{}{}", options::fmt_num(n), unit_suffix))
    };

    let track_col = div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .h(px(8.))
                .rounded_full()
                .bg(Theme::surface_raised())
                .overflow_hidden()
                .child(div().h_full().w(relative(frac)).bg(Theme::accent()).rounded_full()),
        )
        .child(div().flex().justify_between().child(sign(min)).child(sign(max)));

    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(step_btn(handle, modpack_id, name, "minus", "−", (cur - step).max(min), enabled))
        .child(track_col)
        .child(step_btn(handle, modpack_id, name, "plus", "+", (cur + step).min(max), enabled))
        .child(
            div()
                .w(px(72.))
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::text())
                .child(format!("{}{}", options::fmt_num(cur), unit_suffix)),
        )
        .into_any_element()
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
            .child("Обрати");
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
                .child(current.clone().unwrap_or_else(|| "не вибрано".into())),
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
                .child("Скинути"),
        );
    }
    row.into_any_element()
}
