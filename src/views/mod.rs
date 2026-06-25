pub mod account;
pub mod login;
pub mod skin;
pub mod updating;

use std::path::PathBuf;

use gpui::{App, AsyncApp, Context, Entity, PathPromptOptions};

use crate::state::AppState;

/// Subscribe a view to `AppState` so it repaints on every state change. Replaces
/// the `cx.observe(&state, |_, _, cx| cx.notify()).detach()` line every view's
/// constructor used to repeat verbatim.
pub fn observe_repaint<V: 'static>(state: &Entity<AppState>, cx: &mut Context<V>) {
    cx.observe(state, |_, _, cx| cx.notify()).detach();
}

/// Show a native file/directory picker without blocking the UI, then call
/// `then` with the chosen path on the foreground loop. Pass `directories: true`
/// to pick a folder instead of a file.
///
/// We deliberately avoid `native_dialog`'s blocking `.show()`: invoking it from
/// inside an event handler pumps the platform event loop while the GPUI `App`
/// is still borrowed, re-entering it and panicking with "RefCell already
/// borrowed". GPUI's async portal-based prompt sidesteps that entirely.
pub fn pick_path(
    cx: &mut App,
    directories: bool,
    then: impl FnOnce(PathBuf, &mut AsyncApp) + 'static,
) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: !directories,
        directories,
        multiple: false,
        prompt: None,
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(Ok(Some(picked))) = paths.await {
            if let Some(path) = picked.into_iter().next() {
                then(path, cx);
            }
        }
    })
    .detach();
}
