//! The UI-language switcher, in two presentations sharing one click behavior:
//! - [`language_switcher`] — filled pill chips, for the Launcher settings tab
//!   (matches the chip controls it sits among).
//! - [`language_toggle`] — a quiet text toggle, for the login screen where the
//!   chips would be too heavy.
//!
//! Both keep each language's native name in every catalog, so an option stays
//! recognizable regardless of the active language.

use gpui::{
    div, px, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled,
};

use crate::i18n::{self, Lang};
use crate::state::AppState;
use crate::theme::Theme;

/// A row of language chips (Українська / English) that switch + persist the
/// active UI language on click.
pub fn language_switcher(state: &Entity<AppState>) -> gpui::AnyElement {
    let current = i18n::current();
    div()
        .flex()
        .gap(px(8.))
        .child(lang_chip("lang-uk", "Українська", Lang::Uk, current == Lang::Uk, state))
        .child(lang_chip("lang-en", "English", Lang::En, current == Lang::En, state))
        .into_any_element()
}

/// A minimal text toggle: the two language names with a faint dot between them.
/// The active one is bright, the others muted — no backgrounds, so it reads as
/// a quiet footer control rather than a settings field.
pub fn language_toggle(state: &Entity<AppState>) -> gpui::AnyElement {
    let current = i18n::current();
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(lang_text("lang-uk-min", "Українська", Lang::Uk, current == Lang::Uk, state))
        .child(div().w(px(3.)).h(px(3.)).rounded_full().bg(Theme::text_faint()))
        .child(lang_text("lang-en-min", "English", Lang::En, current == Lang::En, state))
        .into_any_element()
}

fn lang_chip(
    id: &'static str,
    label: &'static str,
    lang: Lang,
    active: bool,
    state: &Entity<AppState>,
) -> gpui::AnyElement {
    let chip = div()
        .px(px(14.))
        .py(px(7.))
        .rounded(Theme::radius_block())
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .bg(if active { Theme::accent() } else { Theme::surface_raised() })
        .text_color(if active { Theme::on_accent() } else { Theme::text() })
        .child(label);
    if active {
        return chip.into_any_element();
    }
    let handle = state.clone();
    chip.id(id)
        .cursor_pointer()
        .hover(|s| s.bg(Theme::surface_hover()))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |s, cx| s.set_language(lang, cx));
        })
        .into_any_element()
}

fn lang_text(
    id: &'static str,
    label: &'static str,
    lang: Lang,
    active: bool,
    state: &Entity<AppState>,
) -> gpui::AnyElement {
    let item = div()
        .text_size(px(13.))
        .font_weight(if active { FontWeight::SEMIBOLD } else { FontWeight::MEDIUM })
        .text_color(if active { Theme::text() } else { Theme::text_faint() })
        .child(label);
    if active {
        return item.into_any_element();
    }
    let handle = state.clone();
    item.id(id)
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::text_muted()))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            handle.update(cx, |s, cx| s.set_language(lang, cx));
        })
        .into_any_element()
}
