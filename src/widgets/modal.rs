use std::sync::Arc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Styled, div, ease_out_quint, px, svg,
};

use crate::theme::Theme;

/// Shared alias for a "fire this on close/dismiss" callback. Re-used by every
/// modal view instead of each re-declaring its own identical `Callback` type.
pub type OnClose = Arc<dyn Fn(&mut App) + 'static>;

pub struct Modal {
    title: Option<String>,
    width: f32,
    height: f32,
    body: AnyElement,
    on_close: Option<OnClose>,
}

impl Modal {
    pub fn new(body: impl IntoElement) -> Self {
        Self {
            title: None,
            width: 560.0,
            height: 480.0,
            body: body.into_any_element(),
            on_close: None,
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.width = w;
        self.height = h;
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut App) + 'static) -> Self {
        self.on_close = Some(Arc::new(f));
        self
    }

    pub fn render(self) -> AnyElement {
        let Modal { title, width, height, body, on_close } = self;
        let on_close_for_overlay = on_close.clone();
        let on_close_for_button = on_close;

        let overlay = div()
            .id("modal-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p(px(32.))
            .bg(Theme::overlay())
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                if let Some(cb) = on_close_for_overlay.as_ref() { cb(cx); }
            })
            .child(
                div()
                    .id("modal-card")
                    .flex()
                    .flex_col()
                    .w(px(width))
                    .h(px(height))
                    .bg(Theme::surface())
                    .rounded(Theme::radius_panel())
                    .overflow_hidden()
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(24.))
                            .py(px(16.))
                            .border_b_1()
                            .border_color(Theme::surface_raised())
                            .child(
                                div()
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(16.))
                                    .text_color(Theme::text())
                                    .child(title.unwrap_or_default()),
                            )
                            .child(
                                div()
                                    .id("modal-close")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(32.))
                                    .h(px(32.))
                                    .rounded(Theme::radius_card())
                                    .text_color(Theme::text_muted())
                                    .hover(|s| s.bg(Theme::surface_raised()).text_color(Theme::text()))
                                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(cb) = on_close_for_button.as_ref() { cb(cx); }
                                    })
                                    .child(
                                        svg()
                                            .path("icons/x.svg")
                                            .text_color(Theme::text_muted())
                                            .w(px(16.))
                                            .h(px(16.)),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h(px(0.))
                            .child(body),
                    ),
            );

        overlay
            .with_animation(
                "modal-fade-in",
                Animation::new(Duration::from_millis(120)).with_easing(ease_out_quint()),
                |this, t| this.opacity(t),
            )
            .into_any_element()
    }
}

