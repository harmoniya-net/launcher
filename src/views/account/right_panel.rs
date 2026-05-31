use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px};

use crate::state::AppState;

use super::{description::Description, hero::Hero, news_panel::NewsPanel};

pub struct RightPanel {
    state: Entity<AppState>,
    hero: Entity<Hero>,
    description: Entity<Description>,
    news: Entity<NewsPanel>,
}

impl RightPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        let hero = cx.new(|cx| Hero::new(state.clone(), cx));
        let description = cx.new(|cx| Description::new(state.clone(), cx));
        let news = cx.new(|cx| NewsPanel::new(state.clone(), cx));
        Self { state, hero, description, news }
    }
}

impl Render for RightPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_modpack = self.state.read(cx).selected_modpack().is_some();

        let mut root = div()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .flex()
            .flex_col()
            .gap(px(16.));

        if has_modpack {
            root = root.child(self.hero.clone());
        }

        let mut bottom = div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .gap(px(16.))
            .child(div().flex_1().min_w(px(0.)).min_h(px(0.)).child(self.description.clone()));

        // The news block is per-modpack, so only show it once one is selected.
        if has_modpack {
            bottom = bottom.child(
                div()
                    .w(px(280.))
                    .flex_shrink_0()
                    .min_h(px(0.))
                    .flex()
                    .child(self.news.clone()),
            );
        }

        root.child(bottom)
    }
}
