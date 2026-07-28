//! Discord Rich Presence: shows what the launcher is up to (browsing,
//! installing, playing) on the user's Discord profile.
//!
//! Talks to the desktop Discord client over its local IPC socket/pipe — no
//! network, no auth beyond the app's public client id. That client is only
//! running some of the time (or not installed at all), so every failure here
//! is swallowed and retried; this is cosmetic, never worth surfacing to the UI.
//!
//! The IPC client is blocking, so it's driven from its own OS thread (like the
//! tray's D-Bus connection) rather than the tokio runtime.

use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

use discord_rich_presence::activity::{self, Assets, Button, Timestamps};
use discord_rich_presence::error::Error as DiscordError;
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

/// Harmoniya's Discord Application id — public, used only to identify the app
/// in the local IPC handshake (not a secret, unlike an OAuth client secret).
const APPLICATION_ID: &str = "1196269194224877608";

/// Asset key uploaded under the application's Rich Presence "Art Assets" tab
/// in the Discord Developer Portal. If it isn't there, Discord just shows the
/// activity without an image — no error.
const LARGE_IMAGE: &str = "logo";

/// Shown as a button on the activity. Discord only renders activity buttons on
/// *other* users' view of your profile, never on your own — that's a Discord
/// restriction, not something we control.
const WEBSITE_URL: &str = "https://www.harmoniya.net";

/// How often to retry the handshake while Discord isn't reachable.
const RETRY: Duration = Duration::from_secs(15);

/// What the launcher is currently doing, as shown on the user's Discord
/// profile. Carries only raw data — the background thread renders it into
/// localized text via `crate::i18n` at send time, so a language switch
/// (paired with [`invalidate`]) picks it up without the caller re-deriving it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    /// Signed out, at the login screen — no activity is shown at all.
    LoggedOut,
    /// Signed in, no install/launch in progress.
    Browsing,
    /// Installing/updating a modpack ahead of launch.
    Installing { modpack: String },
    /// A modpack's game process is running. `since_ms` is when it started
    /// (unix ms), so Discord can render an elapsed-time counter.
    Playing { modpack: String, since_ms: i64 },
}

static LAST: Mutex<Option<Presence>> = Mutex::new(None);
static SENDER: Mutex<Option<SyncSender<Presence>>> = Mutex::new(None);

/// Start the background IPC thread. Call once at startup, before the first
/// [`set`].
pub fn init() {
    let (tx, rx) = sync_channel(1);
    *SENDER.lock().unwrap() = Some(tx);
    std::thread::spawn(move || run(rx));
}

/// Show `presence`, unless it's identical to what's already showing (install
/// progress ticks many times a second; only the state transitions matter
/// here). A no-op before [`init`] has run.
pub fn set(presence: Presence) {
    let mut last = LAST.lock().unwrap();
    if last.as_ref() == Some(&presence) {
        return;
    }
    *last = Some(presence.clone());
    drop(last);
    if let Some(tx) = SENDER.lock().unwrap().as_ref() {
        // Bounded(1): if the thread is mid-reconnect and behind, drop the stale
        // update rather than block the caller — a fresher one follows right
        // behind it from the next state change.
        let _ = tx.try_send(presence);
    }
}

/// Forget the last-shown presence so the next [`set`] resends even if the
/// value looks unchanged. Needed after a language switch: `Presence` doesn't
/// carry the localized label itself, so equality alone wouldn't notice it's
/// now stale.
pub fn invalidate() {
    *LAST.lock().unwrap() = None;
}

fn run(rx: Receiver<Presence>) {
    let mut client = DiscordIpcClient::new(APPLICATION_ID);
    let mut connected = false;
    let mut pending: Option<Presence> = None;
    // Logged once per outage so retries every 15s don't spam the log file.
    let mut logged_unreachable = false;

    loop {
        let timeout = if connected { Duration::from_secs(3600) } else { RETRY };
        match rx.recv_timeout(timeout) {
            Ok(p) => pending = Some(p),
            Err(RecvTimeoutError::Timeout) => {}
            // SENDER's only clone lives in the static and is never dropped
            // while the process runs, so this means the process is exiting.
            Err(RecvTimeoutError::Disconnected) => return,
        }

        if !connected {
            match client.connect() {
                Ok(()) => {
                    tracing::debug!("discord rpc connected");
                    connected = true;
                    logged_unreachable = false;
                }
                Err(_) if !logged_unreachable => {
                    tracing::debug!("discord rpc not reachable yet; retrying every {}s", RETRY.as_secs());
                    logged_unreachable = true;
                    continue;
                }
                Err(_) => continue, // Discord not running; retry after RETRY.
            }
        }

        let Some(presence) = pending.take() else { continue };
        match apply(&mut client, &presence) {
            Ok(()) => tracing::debug!(?presence, "discord rpc activity set"),
            Err(e) => {
                tracing::debug!(error = %e, "discord rpc send failed; reconnecting");
                connected = false;
                let _ = client.close();
                pending = Some(presence); // re-apply once reconnected
            }
        }
    }
}

fn apply(client: &mut DiscordIpcClient, presence: &Presence) -> Result<(), DiscordError> {
    let t = crate::i18n::t();
    let activity = match presence {
        Presence::LoggedOut => return client.clear_activity(),
        Presence::Browsing => activity::Activity::new().state(t.discord_browsing),
        // A single `details` line ("Встановлює X" / "Грає на X") rather than
        // Discord's separate details+state lines — see i18n::discord_playing.
        Presence::Installing { modpack } => activity::Activity::new().details(crate::i18n::discord_installing(modpack)),
        Presence::Playing { modpack, since_ms } => activity::Activity::new()
            .details(crate::i18n::discord_playing(modpack))
            .timestamps(Timestamps::new().start(*since_ms)),
    }
    .assets(Assets::new().large_image(LARGE_IMAGE).large_text("Harmoniya"))
    .buttons(vec![Button::new(t.discord_website, WEBSITE_URL)]);

    client.set_activity(activity)
}
