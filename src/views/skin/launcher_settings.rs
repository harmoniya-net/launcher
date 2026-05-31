//! The Launcher-settings tab content: install directory + close-to-tray toggle.
//! Extracted from `skin/page.rs` (it isn't skin-specific — only shares the nav).

use gpui::{
    div, prelude::FluentBuilder, px, relative, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Styled,
};

use harmoniya_launch::pipeline as launch;

use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;
use crate::widgets::toggle::toggle_switch;

use super::skin_form_widgets::truncate_start;

/// Launcher-wide settings (the Launcher tab): the install directory (`${root}`)
/// where modpacks are downloaded, plus the close-to-tray behavior toggle.
pub(crate) fn launcher_settings(
    state: &Entity<AppState>,
    data_dir: Option<String>,
    close_to_tray: bool,
) -> gpui::AnyElement {
    let is_default = data_dir.is_none();
    let effective = data_dir.unwrap_or_else(launch::default_data_dir);
    let state_pick = state.clone();
    let state_reset = state.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .size_full()
        // Cap settings width to 60% of the pane so inputs match the skin tab.
        .max_w(relative(0.6))
        .p(px(24.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::text_faint())
                        .child("ТЕКА ВСТАНОВЛЕННЯ"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(Theme::text_muted())
                        .child("Куди завантажуються та встановлюються файли модпаків."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .h(px(40.))
                .bg(Theme::bg())
                .rounded(Theme::radius_block())
                .overflow_hidden()
                .child(
                    div()
                        .id("pick-dir")
                        .flex_shrink_0()
                        .px(px(16.))
                        .h_full()
                        .flex()
                        .items_center()
                        .bg(Theme::surface_raised())
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text())
                        .cursor_pointer()
                        .hover(|s| s.bg(Theme::surface_hover()))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let result = native_dialog::DialogBuilder::file()
                                .open_single_dir()
                                .show();
                            if let Ok(Some(path)) = result {
                                let path = path.to_string_lossy().to_string();
                                state_pick.update(cx, |s, cx| s.set_data_dir(Some(path), cx));
                            }
                        })
                        .child("Обрати"),
                )
                // Reset-to-default sits just left of the path, shown only when a
                // custom directory is set.
                .when(!is_default, |row| {
                    row.child(
                        div()
                            .id("reset-dir")
                            .flex_shrink_0()
                            .w(px(28.))
                            .h(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(Theme::radius_block())
                            .cursor_pointer()
                            .hover(|s| s.bg(Theme::surface_hover()))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                state_reset.update(cx, |s, cx| s.set_data_dir(None, cx));
                            })
                            .child(icon("icons/rotate-ccw.svg", 15., Theme::text_muted())),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .pr(px(14.))
                        .text_size(px(13.))
                        .text_color(Theme::text_faint())
                        // Leading ellipsis keeps the install folder (the path's
                        // meaningful tail) visible instead of cutting it off.
                        .child(truncate_start(&effective, 40)),
                ),
        )
        .child(close_to_tray_section(state, close_to_tray))
        .into_any_element()
}

/// The "hide to tray vs quit" toggle for the Launcher settings tab.
fn close_to_tray_section(state: &Entity<AppState>, on: bool) -> gpui::AnyElement {
    let state_toggle = state.clone();
    let switch = toggle_switch("close-to-tray", on, move |_, _, cx| {
        state_toggle.update(cx, |s, cx| s.set_close_to_tray(!on, cx));
    });

    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child("ХОВАТИ У ТРЕЙ"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(Theme::text_muted())
                        .child("Замість закриття вікна лаунчер ховатиметься у трей"),
                )
                .child(switch),
        )
        .into_any_element()
}
