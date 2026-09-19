use gpui::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement, Render,
    StatefulInteractiveElement, Styled, StyledImage, Window, div, img, prelude::FluentBuilder, px,
};
use rsx::rsx;

use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::markdown;

pub struct Description { state: Entity<AppState> }

impl Description {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state }
    }
}

impl Render for Description {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let modpack = self.state.read(cx).selected_modpack().cloned();

        let body = if let Some(m) = &modpack {
            markdown::render(m.description.as_deref().unwrap_or(""))
        } else {
            rsx! {
                <div flex items_center justify_center size_full>
                    <div text_size={px(32.)} font_weight={FontWeight::BOLD} text_color={Theme::text()}>
                        {crate::i18n::t().select_server}
                    </div>
                </div>
            }
            .into_any_element()
        };

        let mut panel = rsx! {
            <div
                flex
                flex_1
                h_full
                min_w={px(0.)}
                min_h={px(0.)}
                flex_col
                bg={Theme::surface()}
                rounded={Theme::radius_panel()}
                overflow_hidden
            />
        };

        // Header (project logo + title) and its divider only exist for a
        // selected modpack; with nothing selected there's nothing to label, so
        // skip it — otherwise an empty header strip + border shows above the
        // "Оберіть сервер" placeholder.
        if let Some(m) = &modpack {
            let header = rsx! {
                <div
                    flex
                    items_center
                    gap={px(10.)}
                    px={px(20.)}
                    py={px(12.)}
                    border_b_1
                    border_color={Theme::surface_raised()}
                    when_some={m.project.logo.url.clone(), |el, url| el.child(
                        img(url)
                            .w(px(22.))
                            .h(px(22.))
                            .rounded(Theme::radius_block())
                            .object_fit(ObjectFit::Contain)
                            .flex_shrink_0()
                    )}
                >
                    <div text_size={px(14.)} font_weight={FontWeight::BOLD} text_color={Theme::text()}>
                        {m.title.clone()}
                    </div>
                </div>
            };
            panel = panel.child(header);
        }

        panel.child(rsx! {
            <div id="description-scroll" flex_1 min_h={px(0.)} overflow_y_scroll px={px(24.)} py={px(20.)}>
                {body}
            </div>
        })
    }
}
