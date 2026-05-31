use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};

use crate::state::AppState;
use crate::theme::Theme;

use super::{
    right_panel::RightPanel, server_list::ServerList, user_bar::UserBar,
};

pub struct AccountView {
    pub state: Entity<AppState>,
    server_list: Entity<ServerList>,
    user_bar: Entity<UserBar>,
    right_panel: Entity<RightPanel>,
}

impl AccountView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        let server_list = cx.new(|cx| ServerList::new(state.clone(), cx));
        let user_bar = cx.new(|cx| UserBar::new(state.clone(), cx));
        let right_panel = cx.new(|cx| RightPanel::new(state.clone(), cx));

        state.update(cx, |s, cx| {
            if s.user.is_none() { s.fetch_user(cx); }
            if s.modpacks.is_empty() && !s.modpacks_loading { s.fetch_modpacks(cx); }
            if s.skin_profile.is_none() { s.fetch_skin_profile(cx); }
        });

        Self { state, server_list, user_bar, right_panel }
    }
}

impl Render for AccountView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    .w(px(448.))
                    .h_full()
                    .child(div().flex_1().min_h(px(0.)).child(self.server_list.clone()))
                    .child(self.user_bar.clone()),
            )
            .child(div().flex_1().min_w(px(0.)).h_full().child(self.right_panel.clone()))
    }
}
