//! Routing, modal overlays, and launcher-wide settings.

use gpui::Context;
use harmoniya_api::config;

use super::{ActiveModal, AppEvent, AppState, LaunchState, Route};

impl AppState {
    pub fn set_route(&mut self, route: Route, cx: &mut Context<Self>) {
        self.route = route.clone();
        cx.emit(AppEvent::Routed(route));
        cx.notify();
    }

    pub fn open_modal(&mut self, modal: ActiveModal, cx: &mut Context<Self>) {
        self.active_modal = Some(modal);
        cx.notify();
    }

    pub fn close_modal(&mut self, cx: &mut Context<Self>) {
        // Closing the launch modal abandons any in-flight install (the UI stops
        // updating; the download itself keeps running, matching the web cancel).
        if self.active_modal == Some(ActiveModal::Launch) {
            self.launch_task = None;
            self.launch_state = LaunchState::Idle;
        }
        self.active_modal = None;
        cx.notify();
    }

    pub fn open_news(&mut self, body: String, cx: &mut Context<Self>) {
        self.news_modal_body = Some(body);
        self.open_modal(ActiveModal::News, cx);
    }

    pub fn set_data_dir(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        self.settings.data_dir = path.map(|p| p.trim().to_string()).filter(|s| !s.is_empty());
        let _ = config::save_json(config::SETTINGS_FILE, &self.settings);
        cx.notify();
    }

    /// Toggle whether closing the window hides to the tray or quits the app.
    /// Read by the window's close handler (via the global handle) in main.rs.
    pub fn set_close_to_tray(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.settings.close_to_tray = enabled;
        let _ = config::save_json(config::SETTINGS_FILE, &self.settings);
        cx.notify();
    }
}
