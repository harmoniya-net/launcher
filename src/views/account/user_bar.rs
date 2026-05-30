use gpui::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, Styled, Window, div, img, px, rgb,
};
use crate::widgets::icon::icon;

use harmoniya_api::services::group::lookup;
use crate::state::{AppState, Route, SkinTab};
use crate::theme::Theme;

pub struct UserBar {
    state: Entity<AppState>,
}

impl UserBar {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for UserBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let user = state.user.clone();
        let group = state.selected_modpack().and_then(|m| lookup(&m.title));
        let head = state
            .skin_profile
            .as_ref()
            .and_then(|p| p.skin_url.as_ref())
            .and_then(|url| state.head_cache.get(url).cloned());
        let state_handle = self.state.clone();

        let initial = user
            .as_ref()
            .and_then(|u| u.username.chars().next().map(|c| c.to_uppercase().to_string()))
            .unwrap_or_else(|| "?".to_string());

        div()
            .flex()
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .mt(px(16.))
            .px(px(16.))
            .py(px(10.))
            .h(px(56.))
            .bg(Theme::surface())
            .rounded(Theme::radius_panel())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        if let Some(head_img) = head {
                            div()
                                .w(px(32.))
                                .h(px(32.))
                                .flex_shrink_0()
                                .child(img(head_img).w(px(32.)).h(px(32.)))
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(32.))
                                .h(px(32.))
                                .rounded_full()
                                .bg(Theme::surface_raised())
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::text())
                                .child(initial)
                                .into_any_element()
                        },
                    )
                    .child({
                        let mut info = div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::text())
                                    .child(user.as_ref().map(|u| u.username.clone()).unwrap_or_default()),
                            );
                        if let Some(g) = group {
                            info = info.child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(g.color))
                                    .child(g.name.to_string()),
                            );
                        }
                        info
                    }),
            )
            .child(
                div()
                    .id("settings-btn")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.))
                    .h(px(28.))
                    .rounded(Theme::radius_card())
                    .text_color(Theme::text_faint())
                    .cursor_pointer()
                    .hover(|s| s.bg(Theme::surface_raised()).text_color(Theme::text()))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        state_handle.update(cx, |s, cx| {
                            s.set_route(Route::Skin { tab: SkinTab::Skin }, cx);
                        });
                    })
                    .child(icon("icons/settings.svg", 14., Theme::text_faint())),
            )
    }
}
