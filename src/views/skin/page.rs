use crate::widgets::icon::icon;
use gpui::{
    div, px, rgb, AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, SharedString, Styled, Window,
};

use crate::services::launch;
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
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
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
            .gap(px(16.))
            .p(px(24.))
            .size_full()
            .bg(Theme::bg())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .w(px(446.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .bg(Theme::surface())
                            .rounded(Theme::radius_panel())
                            .p(px(10.))
                            .mb(px(12.))
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
                        SkinTab::Launcher => {
                            launcher_settings(&self.state, data_dir.clone(), close_to_tray)
                        }
                    }),
            )
    }
}

/// Launcher-wide settings (the Launcher tab): the install directory (`${root}`)
/// where modpacks are downloaded, plus the close-to-tray behavior toggle.
fn launcher_settings(
    state: &Entity<AppState>,
    data_dir: Option<String>,
    close_to_tray: bool,
) -> gpui::AnyElement {
    let is_default = data_dir.is_none();
    let effective = data_dir.unwrap_or_else(launch::default_data_dir);
    let state_pick = state.clone();
    let state_clear = state.clone();

    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .size_full()
        .p(px(24.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::text_faint())
                        .child("КАТАЛОГ ВСТАНОВЛЕННЯ"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(Theme::text_muted())
                        .child("Куди завантажуються та встановлюються файли модпаків."),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .h(px(40.))
                .bg(Theme::bg())
                .rounded(Theme::radius_block())
                .overflow_hidden()
                .child(
                    div()
                        .id("pick-dir")
                        .flex_shrink_0()
                        .px(px(16.))
                        .h_full()
                        .flex()
                        .items_center()
                        .bg(Theme::surface_raised())
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text())
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(0x4a4850)))
                        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                            let result = native_dialog::DialogBuilder::file()
                                .open_single_dir()
                                .show();
                            if let Ok(Some(path)) = result {
                                let path = path.to_string_lossy().to_string();
                                state_pick.update(cx, |s, cx| s.set_data_dir(Some(path), cx));
                            }
                        })
                        .child("Обрати"),
                )
                .child(
                    div()
                        .flex_1()
                        .px(px(14.))
                        .text_size(px(13.))
                        .text_color(Theme::text_faint())
                        .child(if is_default {
                            format!("{effective}  ·  за замовчуванням")
                        } else {
                            effective.clone()
                        }),
                ),
        )
        .child(
            div().flex().gap(px(12.)).child(
                div()
                    .id("clear-dir")
                    .px(px(16.))
                    .py(px(8.))
                    .rounded(Theme::radius_block())
                    .bg(Theme::surface_raised())
                    .text_size(px(13.))
                    .text_color(Theme::text())
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(0x4a4850)))
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        state_clear.update(cx, |s, cx| s.set_data_dir(None, cx));
                    })
                    .child("Скинути"),
            ),
        )
        .child(close_to_tray_section(state, close_to_tray))
        .into_any_element()
}

/// The "hide to tray vs quit" toggle for the Launcher settings tab.
fn close_to_tray_section(state: &Entity<AppState>, on: bool) -> gpui::AnyElement {
    let knob = div()
        .w(px(16.))
        .h(px(16.))
        .rounded_full()
        .bg(rgb(0xffffff))
        .ml(if on { px(20.) } else { px(2.) });
    let state_toggle = state.clone();
    let switch = div()
        .id("close-to-tray")
        .w(px(38.))
        .h(px(22.))
        .rounded_full()
        .flex()
        .items_center()
        .flex_shrink_0()
        .bg(if on {
            Theme::accent()
        } else {
            Theme::surface_raised()
        })
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            state_toggle.update(cx, |s, cx| s.set_close_to_tray(!on, cx));
        })
        .child(knob);

    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child("ХОВАТИ У ТРЕЙ"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(12.))
                        .text_color(Theme::text_muted())
                        .child("Замість закриття вікна лаунчер ховатиметься у трей"),
                )
                .child(switch),
        )
        .into_any_element()
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
