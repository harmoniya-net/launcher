use gpui::{
    Context, Entity, ImageSource, InteractiveElement, IntoElement, MouseButton, ObjectFit,
    ParentElement, Render, Styled, StyledImage, Window, div, hsla, img, linear_color_stop,
    linear_gradient, px,
};

use crate::state::{ActiveModal, AppState};
use crate::theme::Theme;
use crate::views::account::play_button::{PlayState, play_button};
use crate::widgets::icon::icon;

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
        crate::views::observe_repaint(&state, cx);
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
        let running = state.is_running(&modpack.id);
        let modpack_id = modpack.id.clone();
        let play_hovered = self.play_hovered;

        let on_hover = cx.listener(|this, hovered: &bool, _, cx| {
            if *hovered {
                this.hover_seq += 1;
            }
            this.play_hovered = *hovered;
            cx.notify();
        });
        let on_click = move |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut gpui::App| {
            if running {
                let id = modpack_id.clone();
                play_handle.update(cx, |s, cx| s.stop_game(id, cx));
            } else {
                play_handle.update(cx, |s, cx| s.play(cx));
            }
        };
        let play_btn = play_button(
            play_state,
            running,
            label,
            play_hovered,
            self.hover_seq,
            on_hover,
            on_click,
        );

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
