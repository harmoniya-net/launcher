//! UI localization: a typed string catalog in Ukrainian (default) and English.
//!
//! Every user-facing literal lives in [`Strings`]; the two catalogs are plain
//! statics, so a missing translation is a compile error. The active language is
//! a process-wide atomic so non-UI threads (the tray's menu render) can read it
//! too. Views call [`t`] at render time — switching languages just needs a
//! repaint (`cx.notify` on `AppState`) plus `shell::tray::refresh()`; see
//! `AppState::set_language`.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

use harmoniya_launch::pipeline::{ErrorCode, Phase};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Uk,
    En,
}

static CURRENT: AtomicU8 = AtomicU8::new(0); // 0 = Uk, 1 = En

pub fn set(lang: Lang) {
    let v = match lang {
        Lang::Uk => 0,
        Lang::En => 1,
    };
    CURRENT.store(v, Ordering::Relaxed);
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::En,
        _ => Lang::Uk,
    }
}

/// The active language's catalog. Read it at render/event time — don't stash
/// the reference across frames, or a language switch won't show until restart.
pub fn t() -> &'static Strings {
    match current() {
        Lang::Uk => &UK,
        Lang::En => &EN,
    }
}

pub struct Strings {
    // Tray menu
    pub tray_hide: &'static str,
    pub tray_show: &'static str,
    pub tray_quit: &'static str,

    // Hero play button states
    pub play: &'static str,
    pub stop: &'static str,
    pub play_maintenance: &'static str,
    pub play_offline: &'static str,
    pub play_sign_in: &'static str,
    pub play_no_access: &'static str,

    // Server card status badge
    pub status_maintenance: &'static str,
    pub status_offline: &'static str,

    // Server list
    pub loading: &'static str,
    pub no_modpacks: &'static str,
    pub favourites: &'static str,

    // Description pane
    pub select_server: &'static str,

    // News
    pub news_header: &'static str,
    pub no_news: &'static str,
    pub news_modal_title: &'static str,

    // Login screen
    pub login_title: &'static str,
    pub login_subtitle: &'static str,
    pub login_harmoniya: &'static str,
    pub login_discord: &'static str,

    // Settings sidebar nav (shared with the tab titles)
    pub nav_back: &'static str,
    pub skin: &'static str,
    pub launcher: &'static str,
    pub nav_logout: &'static str,

    // Skin form
    pub skin_file: &'static str,
    pub cape_file: &'static str,
    pub no_file: &'static str,
    pub arm_model: &'static str,
    pub model_classic: &'static str,
    pub model_slim: &'static str,
    pub save: &'static str,
    pub reset_skin: &'static str,
    pub reset_cape: &'static str,
    pub saving: &'static str,
    pub saved: &'static str,
    pub resetting: &'static str,
    pub reset_done: &'static str,
    pub browse: &'static str,
    pub drag_to_rotate: &'static str,

    // Launcher settings tab
    pub install_dir: &'static str,
    pub install_dir_desc: &'static str,
    pub pick: &'static str,
    pub hide_to_tray: &'static str,
    pub hide_to_tray_desc: &'static str,
    pub language: &'static str,
    pub language_desc: &'static str,

    // Per-modpack settings modal
    pub modpack_settings_title: &'static str,
    pub select_modpack: &'static str,
    pub no_modpack_settings: &'static str,
    pub not_set: &'static str,
    pub reset: &'static str,

    // Launch modal
    pub preparing: &'static str,
    pub game_starting: &'static str,
    pub retry: &'static str,
    pub launch_title: &'static str,
    pub sign_in_to_play: &'static str,
    pub no_manifest: &'static str,

    // Install pipeline phases (long form for the progress bar, short for the
    // error footer) and error-code titles.
    pub phase_resolve: &'static str,
    pub phase_download: &'static str,
    pub phase_verify: &'static str,
    pub phase_extract: &'static str,
    pub phase_resolve_short: &'static str,
    pub phase_download_short: &'static str,
    pub phase_verify_short: &'static str,
    pub phase_extract_short: &'static str,
    pub err_network: &'static str,
    pub err_integrity: &'static str,
    pub err_extract: &'static str,
    pub err_unknown: &'static str,

    // Auto-update screen
    pub updating_title: &'static str,
    pub update_done: &'static str,
    pub restarting: &'static str,
}

impl Strings {
    pub fn phase_label(&self, phase: Phase) -> &'static str {
        match phase {
            Phase::Resolve => self.phase_resolve,
            Phase::Download => self.phase_download,
            Phase::Verify => self.phase_verify,
            Phase::Extract => self.phase_extract,
        }
    }

    pub fn phase_short(&self, phase: Phase) -> &'static str {
        match phase {
            Phase::Resolve => self.phase_resolve_short,
            Phase::Download => self.phase_download_short,
            Phase::Verify => self.phase_verify_short,
            Phase::Extract => self.phase_extract_short,
        }
    }

    pub fn error_label(&self, code: ErrorCode) -> &'static str {
        match code {
            ErrorCode::Network => self.err_network,
            ErrorCode::Integrity => self.err_integrity,
            ErrorCode::Extract => self.err_extract,
            ErrorCode::Unknown => self.err_unknown,
        }
    }
}

static UK: Strings = Strings {
    tray_hide: "Сховати",
    tray_show: "Показати",
    tray_quit: "Вийти",

    play: "Грати",
    stop: "Зупинити",
    play_maintenance: "Тех. роботи",
    play_offline: "Сервер офлайн",
    play_sign_in: "Увійдіть",
    play_no_access: "Немає доступу",

    status_maintenance: "Тех. Роботи",
    status_offline: "Офлайн",

    loading: "Завантаження…",
    no_modpacks: "Поки що немає модпаків",
    favourites: "ОБРАНЕ",

    select_server: "Оберіть сервер",

    news_header: "НОВИНИ",
    no_news: "Поки що немає новин",
    news_modal_title: "Новина",

    login_title: "Вхід",
    login_subtitle: "Увійти в акаунт",
    login_harmoniya: "Увійти з Harmoniya",
    login_discord: "Увійти з Discord",

    nav_back: "Назад",
    skin: "Скін",
    launcher: "Лаунчер",
    nav_logout: "Вийти",

    skin_file: "ФАЙЛ СКІНУ",
    cape_file: "ПЛАЩ",
    no_file: "файл не обрано",
    arm_model: "МОДЕЛЬ РУК",
    model_classic: "звичайна",
    model_slim: "тонка",
    save: "Зберегти",
    reset_skin: "Скинути скін",
    reset_cape: "Скинути плащ",
    saving: "Збереження...",
    saved: "Збережено!",
    resetting: "Скидання...",
    reset_done: "Скинуто",
    browse: "Вибрати",
    drag_to_rotate: "Перетягни для обертання",

    install_dir: "ТЕКА ВСТАНОВЛЕННЯ",
    install_dir_desc: "Куди завантажуються та встановлюються файли модпаків.",
    pick: "Обрати",
    hide_to_tray: "ХОВАТИ У ТРЕЙ",
    hide_to_tray_desc: "Замість закриття вікна лаунчер ховатиметься у трей.",
    language: "МОВА",
    language_desc: "Мова інтерфейсу лаунчера.",

    modpack_settings_title: "Налаштування модпаку",
    select_modpack: "Оберіть модпак",
    no_modpack_settings: "Для цього модпаку немає налаштувань.",
    not_set: "не вибрано",
    reset: "Скинути",

    preparing: "Підготовка…",
    game_starting: "Гра запускається…",
    retry: "Повторити",
    launch_title: "Запуск",
    sign_in_to_play: "Увійдіть в акаунт, щоб запустити гру.",
    no_manifest: "Для цього модпаку не налаштовано маніфест.",

    phase_resolve: "Завантаження маніфесту…",
    phase_download: "Завантаження файлів…",
    phase_verify: "Перевірка цілісності…",
    phase_extract: "Розпакування…",
    phase_resolve_short: "отримання маніфесту",
    phase_download_short: "завантаження",
    phase_verify_short: "перевірка цілісності",
    phase_extract_short: "розпакування",
    err_network: "Помилка мережі",
    err_integrity: "Файл пошкоджено",
    err_extract: "Помилка розпакування",
    err_unknown: "Невідома помилка",

    updating_title: "Оновлення",
    update_done: "Готово",
    restarting: "Перезапуск…",
};

static EN: Strings = Strings {
    tray_hide: "Hide",
    tray_show: "Show",
    tray_quit: "Quit",

    play: "Play",
    stop: "Stop",
    play_maintenance: "Maintenance",
    play_offline: "Server offline",
    play_sign_in: "Sign in",
    play_no_access: "No access",

    status_maintenance: "Maintenance",
    status_offline: "Offline",

    loading: "Loading…",
    no_modpacks: "No modpacks yet",
    favourites: "FAVOURITES",

    select_server: "Select a server",

    news_header: "NEWS",
    no_news: "No news yet",
    news_modal_title: "News",

    login_title: "Sign in",
    login_subtitle: "Log in to your account",
    login_harmoniya: "Sign in with Harmoniya",
    login_discord: "Sign in with Discord",

    nav_back: "Back",
    skin: "Skin",
    launcher: "Launcher",
    nav_logout: "Log out",

    skin_file: "SKIN FILE",
    cape_file: "CAPE",
    no_file: "no file selected",
    arm_model: "ARM MODEL",
    model_classic: "classic",
    model_slim: "slim",
    save: "Save",
    reset_skin: "Reset skin",
    reset_cape: "Reset cape",
    saving: "Saving...",
    saved: "Saved!",
    resetting: "Resetting...",
    reset_done: "Reset",
    browse: "Browse",
    drag_to_rotate: "Drag to rotate",

    install_dir: "INSTALL FOLDER",
    install_dir_desc: "Where modpack files are downloaded and installed.",
    pick: "Browse",
    hide_to_tray: "HIDE TO TRAY",
    hide_to_tray_desc: "Closing the window hides the launcher to the tray instead of quitting.",
    language: "LANGUAGE",
    language_desc: "Launcher interface language.",

    modpack_settings_title: "Modpack settings",
    select_modpack: "Select a modpack",
    no_modpack_settings: "This modpack has no settings.",
    not_set: "not set",
    reset: "Reset",

    preparing: "Preparing…",
    game_starting: "Game is starting…",
    retry: "Retry",
    launch_title: "Launch",
    sign_in_to_play: "Sign in to launch the game.",
    no_manifest: "This modpack has no manifest configured.",

    phase_resolve: "Downloading manifest…",
    phase_download: "Downloading files…",
    phase_verify: "Verifying integrity…",
    phase_extract: "Extracting…",
    phase_resolve_short: "manifest fetch",
    phase_download_short: "download",
    phase_verify_short: "integrity check",
    phase_extract_short: "extraction",
    err_network: "Network error",
    err_integrity: "File corrupted",
    err_extract: "Extraction error",
    err_unknown: "Unknown error",

    updating_title: "Updating",
    update_done: "Done",
    restarting: "Restarting…",
};

// ── Parametric strings ───────────────────────────────────────────────────────

pub fn catalog_error(err: &str) -> String {
    match current() {
        Lang::Uk => format!("Помилка: {err}"),
        Lang::En => format!("Error: {err}"),
    }
}

pub fn files_count(n: usize) -> String {
    match current() {
        Lang::Uk => format!("Файли ({n})"),
        Lang::En => format!("Files ({n})"),
    }
}

pub fn max_skin_size(max: u32) -> String {
    match current() {
        Lang::Uk => format!("Максимальний розмір скіну: {max}×{max}px"),
        Lang::En => format!("Maximum skin size: {max}×{max}px"),
    }
}

pub fn max_cape_size(max: u32) -> String {
    match current() {
        Lang::Uk => format!("Максимальний розмір плаща: {max}×{max}px"),
        Lang::En => format!("Maximum cape size: {max}×{max}px"),
    }
}

pub fn downloading_version(version: &str) -> String {
    match current() {
        Lang::Uk => format!("Завантаження версії {version}…"),
        Lang::En => format!("Downloading version {version}…"),
    }
}

// ── Relative time ("5 хвилин тому" / "5 minutes ago") ───────────────────────

#[derive(Clone, Copy)]
enum Unit {
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

/// Render an elapsed-seconds difference as a coarse relative time in the
/// current language. Bucket thresholds mirror the news panel's original logic.
pub fn relative_time(secs: u64) -> String {
    if secs < 60 {
        return match current() {
            Lang::Uk => "щойно",
            Lang::En => "just now",
        }
        .into();
    }
    let mins = secs / 60;
    if mins < 60 {
        return ago(mins, Unit::Minute);
    }
    let hours = mins / 60;
    if hours < 24 {
        return ago(hours, Unit::Hour);
    }
    let days = hours / 24;
    if days < 7 {
        return ago(days, Unit::Day);
    }
    let weeks = days / 7;
    if weeks < 5 {
        return ago(weeks, Unit::Week);
    }
    let months = days / 30;
    if months < 12 {
        return ago(months, Unit::Month);
    }
    ago(days / 365, Unit::Year)
}

fn ago(n: u64, unit: Unit) -> String {
    match current() {
        Lang::Uk => {
            let (one, few, many) = match unit {
                Unit::Minute => ("хвилину", "хвилини", "хвилин"),
                Unit::Hour => ("годину", "години", "годин"),
                Unit::Day => ("день", "дні", "днів"),
                Unit::Week => ("тиждень", "тижні", "тижнів"),
                Unit::Month => ("місяць", "місяці", "місяців"),
                Unit::Year => ("рік", "роки", "років"),
            };
            format!("{n} {} тому", uk_plural(n, one, few, many))
        }
        Lang::En => {
            let word = match unit {
                Unit::Minute => "minute",
                Unit::Hour => "hour",
                Unit::Day => "day",
                Unit::Week => "week",
                Unit::Month => "month",
                Unit::Year => "year",
            };
            if n == 1 {
                format!("1 {word} ago")
            } else {
                format!("{n} {word}s ago")
            }
        }
    }
}

/// Ukrainian three-form plural: 1/21/31 → one, 2–4/22–24 → few (except
/// 12–14), everything else → many.
fn uk_plural<'a>(n: u64, one: &'a str, few: &'a str, many: &'a str) -> &'a str {
    let n10 = n % 10;
    let n100 = n % 100;
    if n10 == 1 && n100 != 11 {
        one
    } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
        few
    } else {
        many
    }
}
