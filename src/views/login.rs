use gpui::{
    Context, Entity, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ObjectFit,
    ParentElement, Render, Styled, StyledImage, Window, div, img, prelude::FluentBuilder, px, rgb,
};

use harmoniya_api::auth::Provider;
use crate::state::{AppState, LoginPhase};
use crate::theme::Theme;
use crate::widgets::icon::icon;

pub struct LoginView {
    pub state: Entity<AppState>,
}

impl LoginView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state }
    }
}

impl Render for LoginView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state_handle = self.state.clone();
        let harmoniya_handle = state_handle.clone();
        let discord_handle = state_handle;
        let login_error = match &self.state.read(cx).login_phase {
            LoginPhase::Error(msg) => Some(msg.clone()),
            _ => None,
        };
        let t = crate::i18n::t();

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
                                    .child(t.login_title),
                            )
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Theme::text_muted())
                                    .w_full()
                                    .child(t.login_subtitle),
                            )
                            .child(div().h(px(8.)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .w_full()
                                    .child(login_button(
                                        t.login_harmoniya,
                                        "harmoniya-btn",
                                        "icons/harmoniya.svg",
                                        Theme::on_accent().into(),
                                        move |_, _, cx| {
                                            harmoniya_handle.update(cx, |s, cx| {
                                                s.login(Provider::Harmoniya, cx);
                                            });
                                        },
                                    ))
                                    .child(login_button(
                                        t.login_discord,
                                        "discord-btn",
                                        "icons/discord.svg",
                                        Theme::on_accent().into(),
                                        move |_, _, cx| {
                                            discord_handle.update(cx, |s, cx| {
                                                s.login(Provider::Discord, cx);
                                            });
                                        },
                                    ))
                                    .when_some(login_error, |col, msg| {
                                        col.child(
                                            div()
                                                .pt(px(2.))
                                                .text_size(px(13.))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Theme::status_offline())
                                                .w_full()
                                                .child(msg),
                                        )
                                    }),
                            )
                            // Quiet language toggle, so the UI language can be
                            // picked before signing in (settings is post-login).
                            // A hairline sets it apart as a footer control.
                            .child(
                                div()
                                    .mt(px(20.))
                                    .pt(px(16.))
                                    .w_full()
                                    .border_t_1()
                                    .border_color(Theme::surface_raised())
                                    .child(crate::widgets::lang_switch::language_toggle(&self.state)),
                            ),
                    ),
            )
            .child(
                // Right hero panel: a bundled Minecraft scene, Cover-fit so it never
                // stretches at the window's (tiling-WM, arbitrary) aspect. gpui can't
                // round a Cover image — it paints an oversized quad and the content
                // mask is rectangle-only — so the four corners are masked by small
                // bg-coloured concave overlays that turn the square corners round.
                // The bg shows through until the image decodes on first paint.
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .rounded(Theme::radius_panel())
                    .bg(rgb(0x16151a))
                    .overflow_hidden()
                    .child(img("images/hero.webp").size_full().object_fit(ObjectFit::Cover))
                    .children(crate::widgets::corner_mask::corner_masks(Theme::radius_panel())),
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
