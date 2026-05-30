use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, px,
};

use crate::skin_viewer::SkinViewer;
use crate::state::AppState;
use crate::theme::Theme;

pub struct Placeholder {
    _state: Entity<AppState>,
    viewer: Entity<SkinViewer>,
}

impl Placeholder {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let viewer = cx.new(|cx| SkinViewer::new(state.clone(), cx));
        Self { _state: state, viewer }
    }
}

impl Render for Placeholder {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_shrink_0()
            .w(px(320.))
            .h_full()
            .bg(Theme::surface())
            .flex()
            .flex_col()
            .items_center()
            .pt(px(56.))
            .child(self.viewer.clone())
    }
}
