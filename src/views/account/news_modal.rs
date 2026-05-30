use std::sync::Arc;

use gpui::{App, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px};

use crate::state::AppState;
use crate::widgets::{markdown, modal::Modal};

type Callback = Arc<dyn Fn(&mut App) + 'static>;

pub struct NewsModal {
    state: Entity<AppState>,
    on_close: Callback,
}

impl NewsModal {
    pub fn new(state: Entity<AppState>, on_close: impl Fn(&mut App) + 'static, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
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
            .title("Новина")
            .size(780., 640.)
            .on_close(move |cx| on_close(cx))
            .render()
    }
}
