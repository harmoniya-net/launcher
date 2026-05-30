use std::collections::HashMap;
use std::time::Instant;

use gpui::{
    AnyElement, Context, Entity, FontWeight, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Render, StatefulInteractiveElement, StyledImage, Styled, Window, div, img, px,
    prelude::FluentBuilder,
};

use harmoniya_api::services::modpacks::Modpack;
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;

use super::server_card::server_card;

/// Heights mirror the original Vue CSS: active middle grows to 200, edge-active
/// caps at 173 (can only steal from one neighbor), and immediate neighbors of
/// the active card shrink to 119. Sums per group stay constant.
const H_NORMAL: f32 = 146.0;
const H_ACTIVE: f32 = 200.0;
const H_ACTIVE_EDGE: f32 = 173.0;
const H_NEIGHBOR: f32 = 119.0;
const H_HOVER_BUMP: f32 = 14.0;
/// Keep in sync with the duration used in server_card.
const ANIM_MS: f32 = 140.0;

/// Per-card animation state. `source` is where the current tween started,
/// `target` is where it's heading, and `started_at` lets us compute the actual
/// in-flight position when target changes mid-animation — without it, a fresh
/// tween would snap to the previous target as its source and jolt visibly.
#[derive(Clone, Copy)]
struct CardHeight { source: f32, target: f32, started_at: Instant }

fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t } else { let x = -2.0 * t + 2.0; 1.0 - x * x / 2.0 }
}

pub struct ServerList {
    state: Entity<AppState>,
    heights: HashMap<String, CardHeight>,
    hovered_id: Option<String>,
}

impl ServerList {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, heights: HashMap::new(), hovered_id: None }
    }
}

fn target_height(is_active: bool, is_neighbor: bool, is_edge: bool) -> f32 {
    if is_active {
        if is_edge { H_ACTIVE_EDGE } else { H_ACTIVE }
    } else if is_neighbor {
        H_NEIGHBOR
    } else {
        H_NORMAL
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

        let cards = div().flex().flex_col().gap(px(10.)).children(
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
                server_card(
                    m,
                    active,
                    is_hovered,
                    prev,
                    target,
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

        div().flex().flex_col().gap(px(12.)).child(header).child(cards).into_any_element()
    }
}

impl Render for ServerList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let selected = state.selection.selected_modpack_id.clone();
        let groups = state.groups.clone();
        let modpacks = state.modpacks.clone();
        let favourites = state.favourites.clone();
        let loading = state.modpacks_loading;
        let error = state.modpacks_error.clone();
        let state_handle = self.state.clone();

        let mut list = div()
            .id("server-list")
            .flex()
            .flex_col()
            .gap(px(16.))
            .size_full()
            // Reserve a fixed gutter for the scrollbar so it never reflows the
            // card width when it appears (e.g. mid-animation as a card grows on
            // select/hover).
            .pr(px(8.))
            .overflow_y_scroll();

        if loading && groups.is_empty() {
            list = list.child(empty("Завантаження…"));
        } else if let Some(err) = error {
            list = list.child(empty(format!("Помилка: {err}")));
        } else if groups.is_empty() {
            list = list.child(empty("Поки що немає модпаків"));
        } else {
            // Favourites group first — pinned modpacks moved out of their projects.
            let favs: Vec<Modpack> =
                modpacks.iter().filter(|m| favourites.contains(&m.id)).cloned().collect();
            if !favs.is_empty() {
                list = list.child(self.render_group(fav_header(), favs, &selected, &state_handle, cx));
            }
            for group in groups {
                let remaining: Vec<Modpack> =
                    group.modpacks.into_iter().filter(|m| !favourites.contains(&m.id)).collect();
                if remaining.is_empty() {
                    continue;
                }
                let header = project_header(group.project.logo.url.clone(), group.project.title.clone());
                list = list.child(self.render_group(header, remaining, &selected, &state_handle, cx));
            }
        }

        list
    }
}

/// A project group's label: logo + uppercase project name.
fn project_header(logo_url: Option<String>, title: String) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .py(px(4.))
        .opacity(0.6)
        .when_some(logo_url, |this, url| {
            this.child(
                img(url)
                    .w(px(20.))
                    .h(px(20.))
                    .rounded_full()
                    .object_fit(ObjectFit::Cover)
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

/// The Favourites group label: pin icon + "ОБРАНЕ".
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
                .child("ОБРАНЕ"),
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
