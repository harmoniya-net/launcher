use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    AnyElement, Context, Entity, FontWeight, Image, ImageSource, InteractiveElement, IntoElement,
    ObjectFit, ParentElement, Render, StatefulInteractiveElement, StyledImage, Styled, Window,
    div, img, px, prelude::FluentBuilder,
};

use harmoniya_api::services::modpacks::Modpack;
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;

use super::card_anim::{
    ANIM_MS, BANNER_LIT, BANNER_REST, CARD_GAP, CardFrame, CardHeight, CardVisual,
    H_ACTIVE_EDGE, H_HOVER_BUMP, H_NORMAL, SHADOW_ACTIVE, SHADOW_HOVER, SHADOW_REST,
    ease_in_out, target_height,
};
use super::server_card::server_card;

pub struct ServerList {
    state: Entity<AppState>,
    heights: HashMap<String, CardHeight>,
    /// Last banner + shadow alpha rendered per card, so the next render can ease
    /// from it — and resting cards (unchanged) pass `from == to` and don't animate.
    visuals: HashMap<String, CardVisual>,
    hovered_id: Option<String>,
}

impl ServerList {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state, heights: HashMap::new(), visuals: HashMap::new(), hovered_id: None }
    }
}

impl ServerList {
    /// Render one labelled group: `header` above a column of animated cards.
    fn render_group(
        &mut self,
        header: AnyElement,
        modpacks: Vec<Modpack>,
        selected: &Option<String>,
        state_handle: &Entity<AppState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = modpacks.len();
        let active_idx = modpacks
            .iter()
            .position(|m| selected.as_deref() == Some(m.id.as_str()));
        let hover_idx = modpacks
            .iter()
            .position(|m| self.hovered_id.as_deref() == Some(m.id.as_str()));

        // Base targets from active/neighbor/edge layout.
        let mut targets: Vec<f32> = (0..count)
            .map(|i| {
                let active = active_idx == Some(i);
                let is_neighbor = active_idx
                    .map(|a| a + 1 == i || (a > 0 && a - 1 == i))
                    .unwrap_or(false);
                let is_edge = active && (i == 0 || i == count - 1);
                target_height(active, is_neighbor, is_edge)
            })
            .collect();

        // Apply hover bump while keeping the group's total height constant: grow
        // the hovered card and steal the same amount from its immediate neighbors.
        if let Some(hi) = hover_idx.filter(|hi| active_idx != Some(*hi)) {
            targets[hi] += H_HOVER_BUMP;
            let has_prev = hi > 0;
            let has_next = hi + 1 < count;
            match (has_prev, has_next) {
                (true, true) => {
                    targets[hi - 1] -= H_HOVER_BUMP / 2.0;
                    targets[hi + 1] -= H_HOVER_BUMP / 2.0;
                }
                (true, false) => targets[hi - 1] -= H_HOVER_BUMP,
                (false, true) => targets[hi + 1] -= H_HOVER_BUMP,
                (false, false) => {}
            }
        }

        // Reserve a constant height for the group so selecting/hovering never
        // changes the list's total height (which would pop the scrollbar and
        // shift the layout). Multi-card groups always sum to the baseline (the
        // active card's growth is stolen from neighbors); a lone card has no
        // neighbor to steal from, so reserve its active height and let it grow
        // into the reserved space. Cards animate *within* this fixed slot.
        let group_content_h = if count <= 1 {
            H_ACTIVE_EDGE
        } else {
            H_NORMAL * count as f32 + CARD_GAP * (count as f32 - 1.0)
        };

        let cards = div().flex().flex_col().gap(px(CARD_GAP)).h(px(group_content_h)).overflow_hidden().children(
            modpacks.into_iter().enumerate().map(|(i, m)| {
                let active = active_idx == Some(i);
                let is_hovered = hover_idx == Some(i);
                let target = targets[i];
                let now = Instant::now();
                let prev_state = self.heights.get(&m.id).copied().unwrap_or(CardHeight {
                    source: H_NORMAL,
                    target: H_NORMAL,
                    started_at: now,
                });
                // If target changed mid-tween, continue from the real current
                // position rather than snapping to the previous target.
                let curr_state = if (target - prev_state.target).abs() > f32::EPSILON {
                    let elapsed_ms = now
                        .saturating_duration_since(prev_state.started_at)
                        .as_secs_f32() * 1000.0;
                    let t = (elapsed_ms / ANIM_MS).clamp(0.0, 1.0);
                    let eased = ease_in_out(t);
                    let current = prev_state.source + (prev_state.target - prev_state.source) * eased;
                    CardHeight { source: current, target, started_at: now }
                } else {
                    prev_state
                };
                self.heights.insert(m.id.clone(), curr_state);
                let prev = curr_state.source;

                // Banner + shadow targets for this state, eased over the same
                // duration as the height tween (which is what drives the
                // per-frame re-renders). Computed to a static opacity below; on
                // a target change we continue from the current interpolated
                // value so an interrupted ease doesn't jump.
                let (banner_dst, shadow_dst) = if active {
                    (BANNER_LIT, SHADOW_ACTIVE)
                } else if is_hovered {
                    (BANNER_LIT, SHADOW_HOVER)
                } else {
                    (BANNER_REST, SHADOW_REST)
                };
                let ease_at = |start: Instant| {
                    ease_in_out(
                        (now.saturating_duration_since(start).as_secs_f32() * 1000.0 / ANIM_MS)
                            .clamp(0.0, 1.0),
                    )
                };
                let vis = match self.visuals.get(&m.id).copied() {
                    Some(p) if p.banner_dst == banner_dst && p.shadow_dst == shadow_dst => p,
                    Some(p) => {
                        let t = ease_at(p.started_at);
                        CardVisual {
                            banner_src: p.banner_src + (p.banner_dst - p.banner_src) * t,
                            banner_dst,
                            shadow_src: p.shadow_src + (p.shadow_dst - p.shadow_src) * t,
                            shadow_dst,
                            started_at: now,
                        }
                    }
                    None => CardVisual {
                        banner_src: banner_dst,
                        banner_dst,
                        shadow_src: shadow_dst,
                        shadow_dst,
                        started_at: now,
                    },
                };
                self.visuals.insert(m.id.clone(), vis);
                let vt = ease_at(vis.started_at);
                let banner_opacity = vis.banner_src + (vis.banner_dst - vis.banner_src) * vt;
                let shadow_opacity = vis.shadow_src + (vis.shadow_dst - vis.shadow_src) * vt;

                let id = m.id.clone();
                let banner = m
                    .banner
                    .as_ref()
                    .and_then(|b| b.url.as_deref())
                    .map(|u| crate::banner::at_size(u, 816, 400))
                    .and_then(|url| self.state.read(cx).banner_cache.get(&url).cloned());
                let handle = state_handle.clone();
                let hover_id = m.id.clone();
                let on_hover = cx.listener(move |this: &mut Self, hovered: &bool, _, cx| {
                    let currently = this.hovered_id.as_deref() == Some(hover_id.as_str());
                    if *hovered {
                        if !currently {
                            this.hovered_id = Some(hover_id.clone());
                            cx.notify();
                        }
                    } else if currently {
                        this.hovered_id = None;
                        cx.notify();
                    }
                });
                let frame = CardFrame {
                    prev_h: prev,
                    target_h: target,
                    banner_opacity,
                    shadow_opacity,
                };
                server_card(
                    m,
                    active,
                    is_hovered,
                    frame,
                    banner,
                    move |_, _, cx| {
                        handle.update(cx, |s, cx| {
                            s.select_modpack(Some(id.clone()), cx);
                        });
                    },
                    on_hover,
                )
            }),
        );

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap(px(12.))
            .child(header)
            .child(cards)
            .into_any_element()
    }
}

impl Render for ServerList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selection.selected_modpack_id.clone();
        let groups = state.groups.clone();
        let modpacks = state.modpacks.clone();
        let favourites = state.favourites.clone();
        let lucky = state.lucky_profile.clone();
        let loading = state.modpacks_loading;
        let error = state.modpacks_error.clone();
        let logo_cache = state.logo_cache.clone();
        let state_handle = self.state.clone();

        let mut list = div()
            .id("server-list")
            .flex()
            .flex_col()
            .gap(px(16.))
            .size_full()
            .overflow_y_scroll();

        if loading && groups.is_empty() {
            list = list.child(empty(crate::i18n::t().loading));
        } else if let Some(err) = error {
            list = list.child(empty(crate::i18n::catalog_error(&err)));
        } else if groups.is_empty() {
            list = list.child(empty(crate::i18n::t().no_modpacks));
        } else {
            let can_view = |m: &Modpack| {
                lucky.as_ref().is_some_and(|p| p.can_view_modpack(&m.title.to_lowercase()))
            };
            // Favourites group first — pinned modpacks moved out of their projects.
            let favs: Vec<Modpack> = modpacks
                .iter()
                .filter(|m| favourites.contains(&m.id) && can_view(m))
                .cloned()
                .collect();
            if !favs.is_empty() {
                list = list.child(self.render_group(fav_header(), favs, &selected, &state_handle, cx));
            }
            for group in groups {
                let remaining: Vec<Modpack> = group
                    .modpacks
                    .into_iter()
                    .filter(|m| !favourites.contains(&m.id) && can_view(m))
                    .collect();
                if remaining.is_empty() {
                    continue;
                }
                let logo = group.project.logo.url.as_deref()
                    .and_then(|u| logo_cache.get(u).cloned());
                let header = project_header(logo, group.project.title.clone());
                list = list.child(self.render_group(header, remaining, &selected, &state_handle, cx));
            }
        }

        list
    }
}

/// A project group's label: logo + uppercase project name.
fn project_header(logo: Option<Arc<Image>>, title: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(4.))
        .opacity(0.6)
        .when_some(logo, |this, arc| {
            let source: ImageSource = arc.into();
            this.child(
                img(source)
                    .w(px(20.))
                    .h(px(20.))
                    .rounded_full()
                    .object_fit(ObjectFit::Fill)
                    .flex_shrink_0(),
            )
        })
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child(title.to_uppercase()),
        )
        .into_any_element()
}

/// The Favourites group label: pin icon + localized "FAVOURITES".
fn fav_header() -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(4.))
        .opacity(0.6)
        .child(icon("icons/pin-filled.svg", 14., Theme::text_faint()))
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(Theme::text_faint())
                .child(crate::i18n::t().favourites),
        )
        .into_any_element()
}

fn empty(text: impl Into<gpui::SharedString>) -> gpui::AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(120.))
        .text_color(Theme::text_faint())
        .child(text.into())
        .into_any_element()
}
