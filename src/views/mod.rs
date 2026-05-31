pub mod account;
pub mod login;
pub mod skin;
pub mod updating;

use gpui::{Context, Entity};

use crate::state::AppState;

/// Subscribe a view to `AppState` so it repaints on every state change. Replaces
/// the `cx.observe(&state, |_, _, cx| cx.notify()).detach()` line every view's
/// constructor used to repeat verbatim.
pub fn observe_repaint<V: 'static>(state: &Entity<AppState>, cx: &mut Context<V>) {
    cx.observe(state, |_, _, cx| cx.notify()).detach();
}
