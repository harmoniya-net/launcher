use crate::widgets::icon::icon;
use gpui::{
    div, px, relative, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, Styled, Window,
};

use crate::state::{AppState, Route, SkinTab};
use crate::theme::Theme;

use super::{placeholder::Placeholder, skin_form::SkinForm};
use crate::views::account::user_bar::UserBar;

pub struct SkinView {
    state: Entity<AppState>,
    user_bar: Entity<UserBar>,
    skin_form: Entity<SkinForm>,
    placeholder: Entity<Placeholder>,
    tab: SkinTab,
}

impl SkinView {
    pub fn new(state: Entity<AppState>, tab: SkinTab, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        let user_bar = cx.new(|cx| UserBar::new(state.clone(), cx));
        let skin_form = cx.new(|cx| SkinForm::new(state.clone(), cx));
        let placeholder = cx.new(|cx| Placeholder::new(state.clone(), cx));
        state.update(cx, |s, cx| {
            if s.skin_profile.is_none() {
                s.fetch_skin_profile(cx);
            }
        });
        Self {
            state,
            user_bar,
            skin_form,
            placeholder,
            tab,
        }
    }
}

impl Render for SkinView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state_back = self.state.clone();
        let state_skin = self.state.clone();
        let state_launcher = self.state.clone();
        let state_logout = self.state.clone();
        let active_skin = self.tab == SkinTab::Skin;
        let active_launcher = self.tab == SkinTab::Launcher;
        let data_dir = self.state.read(cx).settings.data_dir.clone();
        let close_to_tray = self.state.read(cx).settings.close_to_tray;

        div()
            .flex()
            .gap(Theme::panel_gap())
            .p(Theme::screen_pad())
            .size_full()
            .bg(Theme::bg())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(Theme::sidebar_gap())
                    .flex_shrink_0()
                    .w(Theme::sidebar_width())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .bg(Theme::surface())
                            .rounded(Theme::radius_panel())
                            .p(px(10.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .flex_1()
                                    .child(nav_item(
                                        Some("icons/arrow-left.svg"),
                                        "Назад",
                                        false,
                                        move |_, _, cx| {
                                            state_back.update(cx, |s, cx| {
                                                s.set_route(Route::Account, cx)
                                            });
                                        },
                                    ))
                                    .child(nav_item(
                                        Some("icons/shirt.svg"),
                                        "Скін",
                                        active_skin,
                                        move |_, _, cx| {
                                            state_skin.update(cx, |s, cx| {
                                                s.set_route(Route::Skin { tab: SkinTab::Skin }, cx);
                                            });
                                        },
                                    ))
                                    .child(nav_item(
                                        Some("icons/rocket.svg"),
                                        "Лаунчер",
                                        active_launcher,
                                        move |_, _, cx| {
                                            state_launcher.update(cx, |s, cx| {
                                                s.set_route(
                                                    Route::Skin {
                                                        tab: SkinTab::Launcher,
                                                    },
                                                    cx,
                                                );
                                            });
                                        },
                                    )),
                            )
                            .child(nav_item_styled(
                                Some("icons/log-out.svg"),
                                "Вийти",
                                false,
                                true,
                                move |_, _, cx| {
                                    state_logout.update(cx, |s, cx| s.logout(cx));
                                },
                            )),
                    )
                    .child(self.user_bar.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .bg(Theme::surface())
                    .rounded(Theme::radius_panel())
                    .overflow_hidden()
                    .child(match self.tab {
                        SkinTab::Skin => div()
                            .flex()
                            .size_full()
                            .child(self.skin_form.clone())
                            .child(self.placeholder.clone())
                            .into_any_element(),
                        SkinTab::Launcher => div()
                            .flex()
                            .size_full()
                            .child(crate::views::skin::launcher_settings::launcher_settings(
                                &self.state,
                                data_dir.clone(),
                                close_to_tray,
                            ))
                            // 40% spacer mirrors the skin preview pane so the
                            // settings inputs land at the same width as skin inputs.
                            .child(div().w(relative(0.3)).flex_shrink_0())
                            .into_any_element(),
                    }),
            )
    }
}

fn nav_item(
    icon_path: Option<&'static str>,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    nav_item_styled(icon_path, label, active, false, on_click)
}

fn nav_item_styled(
    icon_path: Option<&'static str>,
    label: &'static str,
    active: bool,
    danger: bool,
    on_click: impl Fn(&gpui::MouseDownEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let (bg, fg) = if danger {
        (Theme::surface(), Theme::status_offline())
    } else if active {
        (Theme::surface_raised(), Theme::text())
    } else {
        (Theme::surface(), Theme::text_faint())
    };
    let hover_fg = if danger {
        Theme::status_offline()
    } else {
        Theme::text_muted()
    };
    let mut item = div()
        .id(SharedString::from(format!("nav-{label}")))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(Theme::radius_block())
        .bg(bg)
        .text_color(fg)
        .text_size(px(14.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .hover(move |s| s.bg(Theme::surface_raised()).text_color(hover_fg))
        .on_mouse_down(MouseButton::Left, on_click);
    if let Some(path) = icon_path {
        item = item.child(icon(path, 14., fg));
    }
    item.child(label).into_any_element()
}
