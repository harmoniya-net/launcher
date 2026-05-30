use std::time::Duration;

use gpui::{
    Animation, AnimationExt, Context, Div, Entity, FontWeight, Hsla, ImageSource,
    InteractiveElement, IntoElement, MouseButton, ObjectFit, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, StyledImage, Svg, Window, div, hsla, img,
    linear_color_stop, linear_gradient, px, rgb, svg,
};

use crate::state::{ActiveModal, AppState};
use crate::theme::Theme;
use crate::widgets::icon::icon;

// The play button is the page's primary action: a minimal slanted (parallelogram)
// tab, drawn via an SVG shape since GPUI divs can't skew, with the label overlaid.
const BTN_W: f32 = 200.;
const BTN_H: f32 = 52.;
const HOVER_GREEN: u32 = 0x3cb371;
const STOP_BG: u32 = 0xf25c63;
const LABEL_DARK: u32 = 0x0e0d0f;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum PlayState {
    Online,
    Offline,
    Maintenance,
    Unauthenticated,
}

pub struct Hero {
    state: Entity<AppState>,
    /// Whether the play button is hovered, to drive the green fade-in.
    play_hovered: bool,
    /// Bumped on each hover-enter so the fade animation restarts (its state is
    /// keyed by element id, which would otherwise stick at "done").
    hover_seq: usize,
}

impl Hero {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, play_hovered: false, hover_seq: 0 }
    }
}

/// A round-cornered icon button for the bottom toolbar (settings, favourite).
fn tool_icon(id: &'static str, svg: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(44.))
        .h(px(44.))
        .rounded(Theme::radius_card())
        .text_color(Theme::text())
        .cursor_pointer()
        .hover(|s| s.bg(hsla(0., 0., 1., 0.16)))
        .child(icon(svg, 20., Theme::text()))
}

impl Render for Hero {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let Some(modpack) = state.selected_modpack().cloned() else {
            return div().into_any_element();
        };
        let authed = state.tokens.is_some();
        let play_state = if !authed {
            PlayState::Unauthenticated
        } else if modpack.maintaining {
            PlayState::Maintenance
        } else if modpack.status.is_none() {
            PlayState::Offline
        } else {
            PlayState::Online
        };

        let label = match play_state {
            PlayState::Maintenance => "Тех. роботи",
            PlayState::Offline => "Сервер офлайн",
            PlayState::Unauthenticated => "Увійдіть",
            PlayState::Online => "Грати",
        };

        let banner_url = modpack
            .banner
            .as_ref()
            .and_then(|b| b.url.as_deref())
            .map(|u| crate::banner::at_size(u, 2400, 440));
        let cached_banner = banner_url.as_ref().and_then(|url| state.banner_cache.get(url).cloned());

        let play_handle = self.state.clone();
        let settings_handle = self.state.clone();
        let play_disabled = play_state != PlayState::Online;
        let running = state.is_running(&modpack.id);
        let modpack_id = modpack.id.clone();
        let play_hovered = self.play_hovered;

        let play_btn = if running {
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
                .child(btn_content(Some("icons/power.svg"), "Зупинити", rgb(0xffffff)))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    let id = modpack_id.clone();
                    play_handle.update(cx, |s, cx| s.stop_game(id, cx));
                })
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
                .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                    if *hovered {
                        this.hover_seq += 1;
                    }
                    this.play_hovered = *hovered;
                    cx.notify();
                }))
                .child(btn_shape(rgb(0xffffff)))
                .child(btn_content(Some("icons/play.svg"), label, rgb(LABEL_DARK)));
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
                            .child(btn_content(Some("icons/play.svg"), label, rgb(0xffffff))),
                    )
                    .with_animation(
                        (SharedString::from("play-hover-green"), self.hover_seq),
                        Animation::new(Duration::from_millis(260)).with_easing(|t| {
                            let u = 1.0 - t;
                            1.0 - u * u * u
                        }),
                        |el, t| el.w(px(BTN_W * t)),
                    );
                btn = btn.child(green);
            }
            btn.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                play_handle.update(cx, |s, cx| s.play(cx));
            })
            .into_any_element()
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
        };

        // Right side of the toolbar: favourite (star) toggle + settings.
        // Outline star when not pinned, filled when pinned — white either way.
        let fav_active = state.is_favourite(&modpack.id);
        let fav_color = Theme::text();
        let fav_icon = if fav_active { "icons/star-filled.svg" } else { "icons/star.svg" };
        let fav_handle = self.state.clone();
        let fav_id = modpack.id.clone();
        let favourite_btn = div()
            .id("hero-favourite")
            .flex()
            .items_center()
            .justify_center()
            .w(px(44.))
            .h(px(44.))
            .rounded(Theme::radius_card())
            .text_color(fav_color)
            .cursor_pointer()
            .hover(|s| s.bg(hsla(0., 0., 1., 0.16)))
            .child(icon(fav_icon, 20., fav_color))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                fav_handle.update(cx, |s, cx| s.toggle_favourite(fav_id.clone(), cx));
            });
        let settings_btn = tool_icon("hero-settings", "icons/settings.svg").on_mouse_down(
            MouseButton::Left,
            move |_, _, cx| {
                settings_handle.update(cx, |s, cx| s.open_modal(ActiveModal::Settings, cx));
            },
        );

        let toolbar = div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .px(px(20.))
            .py(px(9.))
            .child(play_btn)
            .child(div().flex().items_center().gap(px(8.)).child(favourite_btn).child(settings_btn));

        let mut hero = div()
            .relative()
            .flex_shrink_0()
            .w_full()
            .h(px(220.))
            .bg(Theme::surface())
            .rounded(Theme::radius_panel())
            .overflow_hidden();

        if let Some(url) = banner_url {
            let source: ImageSource = match cached_banner {
                Some(arc) => arc.into(),
                None => url.into(),
            };
            hero = hero.child(
                img(source)
                    .object_fit(ObjectFit::Cover)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .rounded(Theme::radius_panel()),
            );
        }
        // Dark fade rising from the bottom so the title + toolbar stay legible.
        hero = hero.child(
            div().absolute().inset_0().rounded(Theme::radius_panel()).bg(linear_gradient(
                180.,
                linear_color_stop(hsla(0., 0., 0., 0.0), 0.05),
                linear_color_stop(hsla(0., 0., 0., 0.85), 1.0),
            )),
        );

        hero.child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .justify_end()
                .child(toolbar),
        )
        .into_any_element()
    }
}
