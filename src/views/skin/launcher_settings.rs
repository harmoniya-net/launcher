//! The Launcher-settings tab content: install directory, close-to-tray toggle,
//! and the UI language switcher.
//! Extracted from `skin/page.rs` (it isn't skin-specific — only shares the nav).
//!
//! Mirrors the skin form's layout (`skin_form.rs`) so the two tabs feel like one
//! settings surface: same 40px padding, 28px title, 32px title→body gap, 20px
//! between fields, and `flex_col gap(8)` label-over-control fields.

use gpui::{
    div, px, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled,
};

use harmoniya_launch::pipeline as launch;

use crate::i18n;
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;
use crate::widgets::toggle::toggle_switch;

use super::skin_form_widgets::truncate_start;
use rsx::rsx;

/// Launcher-wide settings (the Launcher tab): the install directory (`${root}`)
/// where modpacks are downloaded, the close-to-tray behavior toggle, and the
/// UI language.
pub(crate) fn launcher_settings(
    state: &Entity<AppState>,
    data_dir: Option<String>,
    close_to_tray: bool,
) -> gpui::AnyElement {
    rsx! {
        <div
            id="launcher-settings-scroll"
            flex_1
            p={px(40.)}
            overflow_y_scroll
            flex
            flex_col
            gap={px(32.)}
        >
            <div text_size={px(28.)} font_weight={FontWeight::BOLD} text_color={Theme::text()}>
                {i18n::t().launcher}
            </div>
            <div flex flex_col gap={px(20.)}>
                {install_dir_section(state, data_dir)}
                {close_to_tray_section(state, close_to_tray)}
                {language_section(state)}
            </div>
        </div>
    }
    .into_any_element()
}

/// The install-folder field: where modpacks download to (`${root}`), with a
/// pick button, a reset-to-default control, the current path, and an open button.
fn install_dir_section(state: &Entity<AppState>, data_dir: Option<String>) -> gpui::AnyElement {
    let is_default = data_dir.is_none();
    let effective = data_dir.unwrap_or_else(launch::default_data_dir);
    let effective_open = effective.clone();
    let state_pick = state.clone();
    let state_reset = state.clone();

    // Reset-to-default sits just left of the path, shown only
    // when a custom directory is set.
    let reset_dir: Vec<gpui::AnyElement> = if is_default {
        Vec::new()
    } else {
        vec![rsx! {
            <div
                id="reset-dir"
                flex_shrink_0
                mx={px(6.)}
                w={px(28.)}
                h={px(28.)}
                flex
                items_center
                justify_center
                rounded={Theme::radius_block()}
                cursor_pointer
                hover={|s| s.bg(Theme::surface_hover())}
                on_mouse_down={MouseButton::Left, move |_, _, cx| {
                    state_reset.update(cx, |s, cx| s.set_data_dir(None, cx));
                }}
            >
                {icon("icons/rotate-ccw.svg", 15., Theme::text_muted())}
            </div>
        }
        .into_any_element()]
    };

    field(i18n::t().install_dir, i18n::t().install_dir_desc)
        .child(rsx! {
            <div flex items_center gap={px(8.)}>
                // The picker bar: pick button + optional reset + the current path.
                <div
                    flex_1
                    min_w={px(0.)}
                    flex
                    items_center
                    h={px(40.)}
                    bg={Theme::bg()}
                    rounded={Theme::radius_block()}
                    overflow_hidden
                >
                    <div
                        id="pick-dir"
                        flex_shrink_0
                        px={px(16.)}
                        h_full
                        flex
                        items_center
                        bg={Theme::surface_raised()}
                        text_size={px(13.)}
                        font_weight={FontWeight::SEMIBOLD}
                        text_color={Theme::text()}
                        cursor_pointer
                        hover={|s| s.bg(Theme::surface_hover())}
                        on_mouse_down={MouseButton::Left, move |_, _, cx| {
                            let handle = state_pick.clone();
                            crate::views::pick_path(cx, true, move |path, cx| {
                                let path = path.to_string_lossy().to_string();
                                handle.update(cx, |s, cx| s.set_data_dir(Some(path), cx)).ok();
                            });
                        }}
                    >
                        {i18n::t().pick}
                    </div>
                    {..reset_dir}
                    <div
                        flex_1
                        min_w={px(0.)}
                        px={px(14.)}
                        text_size={px(13.)}
                        text_color={Theme::text_faint()}
                    >
                        // Leading ellipsis keeps the install folder (the
                        // path's meaningful tail) visible.
                        {truncate_start(&effective, 40)}
                    </div>
                </div>
                // A standalone square button, detached from the picker bar, that
                // opens the install directory in the system file manager.
                <div
                    id="open-dir"
                    flex_shrink_0
                    w={px(40.)}
                    h={px(40.)}
                    flex
                    items_center
                    justify_center
                    bg={Theme::surface_raised()}
                    rounded={Theme::radius_block()}
                    cursor_pointer
                    hover={|s| s.bg(Theme::surface_hover())}
                    on_mouse_down={MouseButton::Left, move |_, _, _| {
                        let _ = open::that(&effective_open);
                    }}
                >
                    {icon("icons/folder-open.svg", 16., Theme::text_muted())}
                </div>
            </div>
        })
        .into_any_element()
}

/// The hide-to-tray field: hide-to-tray vs quit on window close.
fn close_to_tray_section(state: &Entity<AppState>, on: bool) -> gpui::AnyElement {
    let state_toggle = state.clone();
    let switch = toggle_switch("close-to-tray", on, move |_, _, cx| {
        state_toggle.update(cx, |s, cx| s.set_close_to_tray(!on, cx));
    });

    field(i18n::t().hide_to_tray, i18n::t().hide_to_tray_desc)
        .child(switch)
        .into_any_element()
}

/// The UI-language field: the shared language switcher under a labeled field.
fn language_section(state: &Entity<AppState>) -> gpui::AnyElement {
    field(i18n::t().language, i18n::t().language_desc)
        .child(crate::widgets::lang_switch::language_switcher(state))
        .into_any_element()
}

/// A settings field shared by both sections: an uppercase label and a muted
/// description stacked above whatever control the caller appends. Matches the
/// skin form's `flex_col gap(8)` field rhythm so the two tabs line up.
fn field(label: &'static str, description: &'static str) -> gpui::Div {
    rsx! {
        <div flex flex_col gap={px(8.)}>
            <div flex flex_col gap={px(4.)}>
                <div text_size={px(11.)} font_weight={FontWeight::BOLD} text_color={Theme::text_faint()}>
                    {label}
                </div>
                <div text_size={px(12.)} text_color={Theme::text_muted()}>
                    {description}
                </div>
            </div>
        </div>
    }
}
