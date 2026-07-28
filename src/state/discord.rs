//! Derives the Discord Rich Presence activity from the rest of `AppState`. See
//! [`crate::shell::discord`] for the IPC side.

use super::{AppState, LaunchState};
use crate::shell::discord::Presence;

impl AppState {
    pub(super) fn discord_presence(&self) -> Presence {
        if !self.session.signed_in() {
            return Presence::LoggedOut;
        }

        // A running game outranks the merely-selected modpack: the window is
        // usually hidden to the tray by the time the game is up, so
        // `selection` no longer reflects what's actually on screen. `running`
        // is normally 0-1 entries; `min()` just picks deterministically if
        // more than one is ever up at once.
        if let Some(id) = self.running.iter().min() {
            let modpack = self.modpack_title(id);
            let since_ms = self.running_since.get(id).copied().unwrap_or_else(|| harmoniya_api::now_ms() as i64);
            return Presence::Playing { modpack, since_ms };
        }

        if matches!(self.launch_state, LaunchState::Starting | LaunchState::Progress(_)) {
            if let Some(modpack) = self.selected_modpack() {
                return Presence::Installing { modpack: modpack.title.clone() };
            }
        }

        Presence::Browsing
    }

    fn modpack_title(&self, id: &str) -> String {
        self.modpacks.iter().find(|m| m.id == id).map(|m| m.title.clone()).unwrap_or_else(|| id.to_string())
    }
}
