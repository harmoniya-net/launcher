use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    canvas, div, ease_in_out, px, relative, Animation, AnimationExt, App, Context, DispatchPhase,
    Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Render, SharedString,
    StatefulInteractiveElement, Styled, Window,
};

use harmoniya_api::services::options::{self, Field, ModpackOptions};
use crate::state::AppState;
use crate::theme::Theme;
use crate::views::account::settings_controls::{
    field_card, header_text, path_control, select_control, step_btn,
};
use crate::widgets::modal::{Modal, OnClose};

/// An in-progress slider drag: the dragged slider's binding plus its track
/// geometry (window-space left x + width) and value mapping, frozen at
/// mouse-down. Lets the window-level move handler map cursor x → value while the
/// pointer roams anywhere — tracking horizontal motion, ignoring vertical.
#[derive(Clone)]
struct SliderDrag {
    modpack_id: String,
    name: String,
    left: f32,
    width: f32,
    min: f64,
    max: f64,
    step: f64,
}

/// Per-modpack settings ("Налаштування модпаку"), opened from the hero's ⋮.
/// Renders the modpack's options schema (vars + features), persisting choices.
pub struct SettingsModal {
    state: Entity<AppState>,
    on_close: OnClose,
    /// One focus handle per text field, so they're independently editable.
    text_focus: HashMap<String, FocusHandle>,
    /// The slider being dragged, if any (drives window-level cursor tracking).
    slider_drag: Option<SliderDrag>,
    /// Per-slider tween baseline `(from, to)` fractions, so the fill/thumb glide
    /// between values on +/- and track-clicks. `NaN` until first shown; drags
    /// reset it to snap. Interior-mutable so the `&self` render can update it.
    slider_tween: HashMap<String, Cell<(f32, f32)>>,
}

impl SettingsModal {
    pub fn new(state: Entity<AppState>, on_close: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self {
            state,
            on_close: Arc::new(on_close),
            text_focus: HashMap::new(),
            slider_drag: None,
            slider_tween: HashMap::new(),
        }
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

    /// Recursively collect slider names (which keep per-slider tween state).
    fn collect_slider_names(fields: &[Field], out: &mut Vec<String>) {
        for f in fields {
            match f {
                Field::Slider { name, .. } => out.push(name.clone()),
                Field::Feature { options, .. } => Self::collect_slider_names(options, out),
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
                self.slider_view(modpack_id, name, *min, *max, *step, *default, unit.as_deref(), saved, enabled, cx),
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

    /// A slider: `−`/`+` step buttons around a draggable track. The track is
    /// click-to-set and grab-to-drag; a canvas overlay measures its window-space
    /// geometry, and mouse-down freezes that into `slider_drag` so `render`'s
    /// window-level handler can follow the cursor's x anywhere on screen.
    #[allow(clippy::too_many_arguments)]
    fn slider_view(
        &self,
        modpack_id: &str,
        name: &str,
        min: f64,
        max: f64,
        step: f64,
        default: f64,
        unit: Option<&str>,
        saved: &ModpackOptions,
        enabled: bool,
        cx: &mut Context<Self>,
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

        // Tween endpoints for the fill/thumb. A drag of this slider snaps (it must
        // follow the cursor); +/- and track-clicks glide from the shown value. The
        // `to` is baked into the animation id, so it restarts only on a real change
        // and otherwise keeps advancing across the animation's frames.
        let dragging_this = self
            .slider_drag
            .as_ref()
            .is_some_and(|d| d.modpack_id == modpack_id && d.name == name);
        let (from, to) = match self.slider_tween.get(name) {
            Some(cell) => {
                let (pf, pt) = cell.get();
                if dragging_this || pt.is_nan() {
                    cell.set((frac, frac));
                    (frac, frac)
                } else if (frac - pt).abs() > 0.0005 {
                    cell.set((pt, frac));
                    (pt, frac)
                } else {
                    (pf, pt)
                }
            }
            None => (frac, frac),
        };
        let tween = (to - from).abs() > 0.0005;
        let glide = Animation::new(Duration::from_millis(140)).with_easing(ease_in_out);

        let accent_fill = div().h_full().bg(Theme::accent()).rounded_full();
        let accent_fill = if tween {
            accent_fill
                .with_animation(SharedString::from(format!("slider-fill-{name}-{to}")), glide.clone(), move |el, t| {
                    el.w(relative(from + (to - from) * t))
                })
                .into_any_element()
        } else {
            accent_fill.w(relative(to)).into_any_element()
        };
        let bar = div()
            .flex_1()
            .h(px(8.))
            .rounded_full()
            .bg(Theme::surface_raised())
            .overflow_hidden()
            .child(accent_fill);

        // Grab handle centred on the fill's edge (`-8` = half its 16px width).
        let thumb = div()
            .absolute()
            .top(px(2.))
            .ml(px(-8.))
            .w(px(16.))
            .h(px(16.))
            .rounded_full()
            .bg(Theme::text())
            .border_2()
            .border_color(Theme::accent());
        let thumb = if tween {
            thumb
                .with_animation(SharedString::from(format!("slider-thumb-{name}-{to}")), glide, move |el, t| {
                    el.left(relative(from + (to - from) * t))
                })
                .into_any_element()
        } else {
            thumb.left(relative(to)).into_any_element()
        };

        // Taller, transparent hit row so a click/drag is easy to land.
        let mut track = div()
            .id(SharedString::from(format!("slider-{name}")))
            .relative()
            .h(px(20.))
            .flex()
            .items_center()
            .child(bar)
            .child(thumb);

        if enabled {
            // Track geometry (window-space left x + width), measured each paint by
            // a canvas overlay. mouse-down freezes it into `slider_drag`.
            let geo = Rc::new(Cell::new((0.0f32, 0.0f32)));
            let measure = geo.clone();
            let mid = modpack_id.to_string();
            let nm = name.to_string();
            track = track
                .cursor_pointer()
                .child(
                    canvas(
                        move |b, _, _| measure.set((f32::from(b.origin.x), f32::from(b.size.width))),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, _window, cx| {
                        let (left, width) = geo.get();
                        let drag = SliderDrag {
                            modpack_id: mid.clone(),
                            name: nm.clone(),
                            left,
                            width,
                            min,
                            max,
                            step,
                        };
                        this.apply_slider(e.position.x, &drag, cx);
                        this.slider_drag = Some(drag);
                        cx.notify();
                    }),
                );
        }

        let track_col = div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(5.))
            .child(track)
            .child(div().flex().justify_between().child(sign(min)).child(sign(max)));

        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(step_btn(&self.state, modpack_id, name, "minus", "−", (cur - step).max(min), enabled))
            .child(track_col)
            .child(step_btn(&self.state, modpack_id, name, "plus", "+", (cur + step).min(max), enabled))
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

    /// Map a window-space cursor x onto the slider's stepped, clamped value and
    /// persist it (a no-op when the stepped value is unchanged).
    fn apply_slider(&self, x: Pixels, drag: &SliderDrag, cx: &mut Context<Self>) {
        if drag.width <= 0.0 {
            return;
        }
        let frac = ((f32::from(x) - drag.left) / drag.width).clamp(0.0, 1.0) as f64;
        let raw = drag.min + frac * (drag.max - drag.min);
        let stepped = if drag.step > 0.0 { (raw / drag.step).round() * drag.step } else { raw };
        let value = stepped.clamp(drag.min.min(drag.max), drag.min.max(drag.max));
        let (mid, nm) = (drag.modpack_id.clone(), drag.name.clone());
        self.state
            .update(cx, |s, cx| s.set_option_value(mid, nm, Some(options::fmt_num(value)), cx));
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

        // Ensure a tween cell exists for every slider (NaN = "not shown yet").
        let mut slider_names = Vec::new();
        Self::collect_slider_names(&schema, &mut slider_names);
        for name in slider_names {
            self.slider_tween.entry(name).or_insert_with(|| Cell::new((f32::NAN, f32::NAN)));
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

        // While a slider is dragged, track the cursor at the window level so it
        // follows horizontal movement even when the pointer leaves the slider
        // row. Only x is used; vertical motion is ignored.
        if self.slider_drag.is_some() {
            let this = cx.entity();
            window.on_mouse_event({
                let this = this.clone();
                move |e: &MouseMoveEvent, phase, _window, cx| {
                    if phase != DispatchPhase::Capture {
                        return;
                    }
                    this.update(cx, |modal, cx| {
                        let Some(drag) = modal.slider_drag.clone() else { return };
                        if e.pressed_button == Some(MouseButton::Left) {
                            modal.apply_slider(e.position.x, &drag, cx);
                        } else {
                            // Button released off-window (no mouse-up reached us).
                            modal.slider_drag = None;
                            cx.notify();
                        }
                    });
                }
            });
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }
                this.update(cx, |modal, cx| {
                    if modal.slider_drag.take().is_some() {
                        cx.notify();
                    }
                });
            });
        }

        Modal::new(col)
            .title("Налаштування модпаку")
            .size(720., 620.)
            .on_close(move |cx| on_close(cx))
            .render()
    }
}
