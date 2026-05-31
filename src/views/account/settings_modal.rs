use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, Context, Entity, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use harmoniya_api::services::options::{Field, ModpackOptions};
use crate::state::AppState;
use crate::theme::Theme;
use crate::views::account::settings_controls::{
    field_card, header_text, path_control, select_control, slider_control,
};
use crate::widgets::modal::{Modal, OnClose};

/// Per-modpack settings ("Налаштування модпаку"), opened from the hero's ⋮.
/// Renders the modpack's options schema (vars + features), persisting choices.
pub struct SettingsModal {
    state: Entity<AppState>,
    on_close: OnClose,
    /// One focus handle per text field, so they're independently editable.
    text_focus: HashMap<String, FocusHandle>,
}

impl SettingsModal {
    pub fn new(state: Entity<AppState>, on_close: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state, on_close: Arc::new(on_close), text_focus: HashMap::new() }
    }

    /// Recursively collect the names of text fields (which need focus handles).
    fn collect_text_names(fields: &[Field], out: &mut Vec<String>) {
        for f in fields {
            match f {
                Field::Text { name, .. } => out.push(name.clone()),
                Field::Feature { options, .. } => Self::collect_text_names(options, out),
                _ => {}
            }
        }
    }

    /// One schema field. `enabled` is false for children under a disabled feature.
    fn field_view(
        &self,
        modpack_id: &str,
        field: &Field,
        saved: &ModpackOptions,
        enabled: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let handle = &self.state;
        match field {
            Field::Feature { name, default, options, title, subtitle } => {
                self.feature_view(modpack_id, name, *default, options, saved, title, subtitle.as_deref(), window, cx)
            }
            Field::Slider { name, min, max, step, default, title, subtitle, unit } => field_card(
                title,
                subtitle.as_deref(),
                enabled,
                slider_control(handle, modpack_id, name, *min, *max, *step, *default, unit.as_deref(), saved, enabled),
            ),
            Field::Select { name, choices, default, title, subtitle } => field_card(
                title,
                subtitle.as_deref(),
                enabled,
                select_control(handle, modpack_id, name, choices, default, saved, enabled),
            ),
            Field::File { name, title, subtitle } => field_card(
                title,
                subtitle.as_deref(),
                enabled,
                path_control(handle, modpack_id, name, false, saved, enabled),
            ),
            Field::Directory { name, title, subtitle } => field_card(
                title,
                subtitle.as_deref(),
                enabled,
                path_control(handle, modpack_id, name, true, saved, enabled),
            ),
            Field::Text { name, default, placeholder, title, subtitle } => field_card(
                title,
                subtitle.as_deref(),
                enabled,
                self.text_control(modpack_id, name, default.as_deref(), placeholder.as_deref(), saved, enabled, window, cx),
            ),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn feature_view(
        &self,
        modpack_id: &str,
        name: &str,
        default: bool,
        options: &[Field],
        saved: &ModpackOptions,
        title: &str,
        subtitle: Option<&str>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let on = saved.features.get(name).copied().unwrap_or(default);
        let h = self.state.clone();
        let mid = modpack_id.to_string();
        let nm = name.to_string();
        let switch =
            crate::widgets::toggle::toggle_switch(SharedString::from(format!("feat-{name}")), on, move |_, _, cx| {
                h.update(cx, |s, cx| s.set_feature(mid.clone(), nm.clone(), !on, cx));
            });

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(header_text(title, subtitle).flex_1())
            .child(switch);

        let mut col = div().flex().flex_col().gap(px(12.)).child(header);
        for child in options {
            col = col.child(
                div().pl(px(12.)).child(self.field_view(modpack_id, child, saved, on, window, cx)),
            );
        }
        col.into_any_element()
    }

    /// A minimal live-editing text input: focusable, keystrokes edit the var.
    #[allow(clippy::too_many_arguments)]
    fn text_control(
        &self,
        modpack_id: &str,
        name: &str,
        default: Option<&str>,
        placeholder: Option<&str>,
        saved: &ModpackOptions,
        enabled: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let value = saved
            .vars
            .get(name)
            .cloned()
            .or_else(|| default.map(str::to_string))
            .unwrap_or_default();
        let handle = self.text_focus.get(name).cloned();
        let focused = handle.as_ref().map(|h| h.is_focused(window)).unwrap_or(false);
        let show_placeholder = value.is_empty() && !focused;
        let display = if show_placeholder {
            placeholder.unwrap_or("—").to_string()
        } else if focused {
            format!("{value}│")
        } else {
            value
        };

        let base = div()
            .h(px(40.))
            .px(px(14.))
            .flex()
            .items_center()
            .bg(Theme::bg())
            .rounded(Theme::radius_block())
            .border_1()
            .border_color(if focused { Theme::accent() } else { Theme::surface_raised() })
            .text_size(px(13.))
            .text_color(if show_placeholder { Theme::text_faint() } else { Theme::text() })
            .child(display);

        let Some(handle) = handle.filter(|_| enabled) else {
            return base.into_any_element();
        };
        let focus = handle.clone();
        let mid = modpack_id.to_string();
        let nm = name.to_string();
        base.id(SharedString::from(format!("txt-{name}")))
            .track_focus(&handle)
            .cursor_text()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                focus.focus(window);
            })
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                let ks = &ev.keystroke;
                if ks.modifiers.control || ks.modifiers.platform {
                    return;
                }
                let mut cur = this.state.read(cx).option_value(&mid, &nm).unwrap_or_default();
                match ks.key.as_str() {
                    "backspace" => {
                        cur.pop();
                    }
                    "enter" | "escape" | "tab" => return,
                    _ => {
                        if let Some(ch) = &ks.key_char {
                            cur.push_str(ch);
                        }
                    }
                }
                this.state.update(cx, |s, cx| s.set_option_value(mid.clone(), nm.clone(), Some(cur), cx));
            }))
            .into_any_element()
    }
}

impl Render for SettingsModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let on_close = self.on_close.clone();
        let modpack = self.state.read(cx).selected_modpack().cloned();

        let Some(m) = modpack else {
            let body = div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .text_color(Theme::text_faint())
                .child("Оберіть модпак");
            return Modal::new(body)
                .title("Налаштування модпаку")
                .size(720., 620.)
                .on_close(move |cx| on_close(cx))
                .render();
        };

        let schema = m.options.clone();
        let saved = self.state.read(cx).settings.modpack_options.get(&m.id).cloned().unwrap_or_default();

        // Ensure a focus handle exists for every text field.
        let mut text_names = Vec::new();
        Self::collect_text_names(&schema, &mut text_names);
        for name in text_names {
            self.text_focus.entry(name).or_insert_with(|| cx.focus_handle());
        }

        let mut col = div()
            .id("modpack-settings")
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(24.))
            .size_full()
            .overflow_y_scroll()
            // Click anywhere that isn't the text field itself unfocuses it
            // (text fields stop propagation in their own mouse-down handler).
            .on_mouse_down(MouseButton::Left, |_, window, _| window.blur());
        if schema.is_empty() {
            col = col.child(
                div().text_size(px(13.)).text_color(Theme::text_faint()).child("Для цього модпаку немає налаштувань."),
            );
        }
        for field in &schema {
            col = col.child(self.field_view(&m.id, field, &saved, true, window, cx));
        }

        Modal::new(col)
            .title("Налаштування модпаку")
            .size(720., 620.)
            .on_close(move |cx| on_close(cx))
            .render()
    }
}
