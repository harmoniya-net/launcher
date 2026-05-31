use gpui::{
    AnyView, AppContext, Context, Entity, Font, FontFeatures, FontStyle,
    FontWeight, IntoElement, ParentElement, Render, Styled, Window, div,
    prelude::FluentBuilder,
};

use crate::state::{ActiveModal, AppState, Route};
use crate::theme::Theme;
use crate::views::{
    account::{AccountView, launch_modal::LaunchModal, news_modal::NewsModal, settings_modal::SettingsModal},
    login::LoginView,
    skin::SkinView,
};

pub struct Root {
    state: Entity<AppState>,
    current: Option<AnyView>,
    current_route: Option<Route>,
    launch_modal: Entity<LaunchModal>,
    settings_modal: Entity<SettingsModal>,
    news_modal: Entity<NewsModal>,
}

impl Root {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, _, cx| {
            this.refresh(cx);
            cx.notify();
        }).detach();

        let close = {
            let handle = state.clone();
            move |cx: &mut gpui::App| {
                handle.update(cx, |s, cx| s.close_modal(cx));
            }
        };
        let launch_modal = cx.new({
            let state = state.clone();
            let close = close.clone();
            |cx| LaunchModal::new(state, close, cx)
        });
        let settings_modal = cx.new({
            let state = state.clone();
            |cx| SettingsModal::new(state, close.clone(), cx)
        });
        let news_modal = cx.new({
            let state = state.clone();
            |cx| NewsModal::new(state, close, cx)
        });

        let mut me = Self {
            state,
            current: None,
            current_route: None,
            launch_modal,
            settings_modal,
            news_modal,
        };
        me.refresh(cx);
        me
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let route = self.state.read(cx).route.clone();
        // Only rebuild the view if the route variant or tab actually changed —
        // every state notification would otherwise destroy child entity state
        // (like the skin viewer's rotation).
        if self.current.is_some() && route_matches(self.current_route.as_ref(), &route) {
            return;
        }
        let state = self.state.clone();
        let view: AnyView = match &route {
            Route::Login => cx.new(|cx| LoginView::new(state, cx)).into(),
            Route::Account => cx.new(|cx| AccountView::new(state, cx)).into(),
            Route::Skin { tab } => cx.new(|cx| SkinView::new(state, *tab, cx)).into(),
        };
        self.current_route = Some(route);
        self.current = Some(view);
    }
}

fn route_matches(a: Option<&Route>, b: &Route) -> bool {
    match (a, b) {
        (Some(Route::Login), Route::Login) => true,
        (Some(Route::Account), Route::Account) => true,
        (Some(Route::Skin { tab: t1 }), Route::Skin { tab: t2 }) => t1 == t2,
        _ => false,
    }
}

impl Render for Root {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = self.current.clone();
        let modal = self.state.read(cx).active_modal;

        // Only the bundled Inter is used — no system-font fallbacks. (Emoji are
        // rendered as Twemoji images by the emoji/markdown widgets, not glyphs.)
        let font = Font {
            family: Theme::font(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::NORMAL,
            style: FontStyle::Normal,
        };

        div()
            .relative()
            .size_full()
            .bg(Theme::bg())
            .text_color(Theme::text())
            .font(font)
            .when_some(view, |this, v| this.child(v))
            .when_some(modal, |this, m| {
                this.child(match m {
                    ActiveModal::Launch => self.launch_modal.clone().into_any_element(),
                    ActiveModal::Settings => self.settings_modal.clone().into_any_element(),
                    ActiveModal::News => self.news_modal.clone().into_any_element(),
                })
            })
    }
}
