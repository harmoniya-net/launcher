//! Per-card animation state and easing for the server list (height tween +
//! banner/shadow opacity). Extracted from `server_list.rs`.

use std::time::Instant;

/// Heights mirror the original Vue CSS: active middle grows to 200, edge-active
/// caps at 173 (can only steal from one neighbor), and immediate neighbors of
/// the active card shrink to 119. Sums per group stay constant.
pub(crate) const H_NORMAL: f32 = 146.0;
pub(crate) const H_ACTIVE: f32 = 200.0;
pub(crate) const H_ACTIVE_EDGE: f32 = 173.0;
pub(crate) const H_NEIGHBOR: f32 = 119.0;
pub(crate) const H_HOVER_BUMP: f32 = 14.0;
/// Vertical gap between cards within a group.
pub(crate) const CARD_GAP: f32 = 10.0;
/// Keep in sync with the duration used in server_card.
pub(crate) const ANIM_MS: f32 = 140.0;

/// Fixed width for the card column inside the (448px) sidebar, leaving a 16px
/// gutter for the scrollbar so cards never reflow when it appears.
pub(crate) const CARD_COL_W: f32 = 432.0;

/// Bottom-shadow alpha per visual state (see server_card); tracked so only the
/// card whose state changed animates.
pub(crate) const SHADOW_REST: f32 = 0.85;
pub(crate) const SHADOW_HOVER: f32 = 0.45;
pub(crate) const SHADOW_ACTIVE: f32 = 0.20;
/// Banner image opacity: dimmed at rest, revealed on hover/active.
pub(crate) const BANNER_REST: f32 = 0.35;
pub(crate) const BANNER_LIT: f32 = 0.85;

/// Per-card animation state. `source` is where the current tween started,
/// `target` is where it's heading, and `started_at` lets us compute the actual
/// in-flight position when target changes mid-animation — without it, a fresh
/// tween would snap to the previous target as its source and jolt visibly.
#[derive(Clone, Copy)]
pub(crate) struct CardHeight {
    pub source: f32,
    pub target: f32,
    pub started_at: Instant,
}

/// In-flight banner + shadow ease for a card, tracked exactly like [`CardHeight`]
/// (source/target/started_at) so the list can compute the eased opacity per
/// frame and apply it *statically* — no per-card `with_animation`, which (since
/// the list re-renders every frame) GPUI would re-fire and flicker on siblings.
#[derive(Clone, Copy)]
pub(crate) struct CardVisual {
    pub banner_src: f32,
    pub banner_dst: f32,
    pub shadow_src: f32,
    pub shadow_dst: f32,
    pub started_at: Instant,
}

/// Per-frame animation inputs that `server_card` consumes: the height to tween
/// from/to plus the already-eased banner and bottom-shadow opacities the caller
/// computed for this frame.
#[derive(Clone, Copy)]
pub(crate) struct CardFrame {
    pub prev_h: f32,
    pub target_h: f32,
    pub banner_opacity: f32,
    pub shadow_opacity: f32,
}

pub(crate) fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 { 2.0 * t * t } else { let x = -2.0 * t + 2.0; 1.0 - x * x / 2.0 }
}

pub(crate) fn target_height(is_active: bool, is_neighbor: bool, is_edge: bool) -> f32 {
    if is_active {
        if is_edge { H_ACTIVE_EDGE } else { H_ACTIVE }
    } else if is_neighbor {
        H_NEIGHBOR
    } else {
        H_NORMAL
    }
}
