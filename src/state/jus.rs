use gpui::Context;
use harmoniya_api::services::jus::JusConnection;

use super::AppState;

impl AppState {
    /// Opens the player's jus socket once signed in. `permissions` then
    /// follows it live: every grant or revoke shows up without a refresh.
    pub fn connect_jus(&mut self, cx: &mut Context<Self>) {
        if !self.session.signed_in() || self.jus.is_some() {
            return;
        }
        let conn = JusConnection::connect(self.session.clone());
        let mut changes = conn.subscribe();
        self.jus = Some(conn);
        self.jus_task = Some(cx.spawn(async move |this, cx| {
            // Ends when the connection is dropped (sign-out) or the state entity goes away.
            while changes.changed().await.is_ok() {
                let alive = this.update(cx, |state, cx| {
                    state.permissions = state.jus.as_ref().and_then(JusConnection::permissions);
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
    }

    /// Closes the socket and forgets the permissions (sign-out).
    pub(super) fn disconnect_jus(&mut self) {
        self.jus_task = None;
        self.jus = None;
        self.permissions = None;
    }
}
