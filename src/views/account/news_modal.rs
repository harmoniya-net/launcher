use std::sync::Arc;

use gpui::{App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px};

use crate::state::AppState;
use crate::widgets::{markdown, modal::{Modal, OnClose}};

pub struct NewsModal {
    state: Entity<AppState>,
    on_close: OnClose,
}

impl NewsModal {
    pub fn new(state: Entity<AppState>, on_close: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state, on_close: Arc::new(on_close) }
    }
}

impl Render for NewsModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = self.state.read(cx).news_modal_body.clone().unwrap_or_default();
        let on_close = self.on_close.clone();

        let content = div()
            .id("news-modal-scroll")
            .flex_1()
            .overflow_y_scroll()
            .p(px(24.))
            .child(markdown::render(&body));

        Modal::new(content)
            .title(crate::i18n::t().news_modal_title)
            .size(780., 640.)
            .on_close(move |cx| on_close(cx))
            .render()
    }
}
