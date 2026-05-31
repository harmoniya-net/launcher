use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
    relative,
};

use crate::state::AppState;
use crate::theme::Theme;
use crate::views::skin::viewer::SkinViewer;

pub struct Placeholder {
    _state: Entity<AppState>,
    viewer: Entity<SkinViewer>,
}

impl Placeholder {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        let viewer = cx.new(|cx| SkinViewer::new(state.clone(), cx));
        Self { _state: state, viewer }
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 40% of the skin pane so the form (and its inputs) take ~60% —
        // matching the launcher settings tab.
        div()
            .w(relative(0.4))
            .flex_shrink_0()
            .h_full()
            .bg(Theme::surface())
            .flex()
            .flex_col()
            .items_center()
            .pt(px(56.))
            .child(self.viewer.clone())
    }
}
