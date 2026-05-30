use gpui::{
    Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};

use harmoniya_api::auth::Provider;
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;

/// Icons sit on the white login buttons, so they're drawn dark.
const ICON_DARK: u32 = 0x0e0d0f;

pub struct LoginView {
    pub state: Entity<AppState>,
}

impl LoginView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state }
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let state_handle = self.state.clone();
        let harmoniya_handle = state_handle.clone();
        let discord_handle = state_handle;

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .p(px(24.))
            .size_full()
            .bg(Theme::bg())
            .child(
                // Left panel
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .w(px(446.))
                    .h_full()
                    .bg(Theme::surface())
                    .rounded(Theme::radius_panel())
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap(px(6.))
                            .w(px(232.))
                            .child(
                                div()
                                    .text_size(px(42.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(Theme::text())
                                    .w_full()
                                    .child("Вхід"),
                            )
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_muted())
                                    .w_full()
                                    .child("Увійти в акаунт"),
                            )
                            .child(div().h(px(8.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .w_full()
                                    .child(login_button(
                                        "Увійти з Harmoniya",
                                        "harmoniya-btn",
                                        "icons/harmoniya.svg",
                                        rgb(ICON_DARK).into(),
                                        move |_, _, cx| {
                                            harmoniya_handle.update(cx, |s, cx| {
                                                s.login(Provider::Harmoniya, cx);
                                            });
                                        },
                                    ))
                                    .child(login_button(
                                        "Увійти з Discord",
                                        "discord-btn",
                                        "icons/discord.svg",
                                        rgb(ICON_DARK).into(),
                                        move |_, _, cx| {
                                            discord_handle.update(cx, |s, cx| {
                                                s.login(Provider::Discord, cx);
                                            });
                                        },
                                    )),
                            ),
                    ),
            )
            .child(
                // Right hero panel (solid colour for now; the bundled hero PNG can be wired through gpui's image cache later)
                div()
                    .flex_1()
                    .h_full()
                    .rounded(Theme::radius_panel())
                    .bg(rgb(0x16151a))
                    .overflow_hidden(),
            )
    }
}

fn login_button(
    label: &'static str,
    id: &'static str,
    icon_svg: &'static str,
    icon_color: Hsla,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_between()
        .bg(Theme::text())
        .rounded(Theme::radius_block())
        .px(px(12.))
        .py(px(8.))
        .w_full()
        .text_color(rgb(0x000000))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(14.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0xeeeeee)))
        .on_mouse_down(MouseButton::Left, move |ev, w, cx| on_click(ev, w, cx))
        .child(label)
        .child(icon(icon_svg, 20., icon_color))
        .into_any_element()
}
