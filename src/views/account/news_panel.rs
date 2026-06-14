use gpui::{
    Context, Entity, FontWeight, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Render, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use harmoniya_api::services::modpacks::ModpackAnnouncement;
use crate::state::AppState;
use crate::theme::Theme;
use crate::widgets::icon::icon;

pub struct NewsPanel { state: Entity<AppState> }

impl NewsPanel {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        crate::views::observe_repaint(&state, cx);
        Self { state }
    }
}

impl Render for NewsPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut items: Vec<ModpackAnnouncement> = self.state.read(cx)
            .selected_modpack()
            .map(|m| m.announcements.clone())
            .unwrap_or_default();
        items.sort_by(|a, b| b.date.cmp(&a.date));

        let state = self.state.clone();

        let body = if items.is_empty() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px(px(16.))
                .py(px(24.))
                .text_size(px(13.))
                .text_color(Theme::text_faint())
                .child(crate::i18n::t().no_news)
                .into_any_element()
        } else {
            div()
                .id("news-scroll")
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .min_h(px(0.))
                .p(px(8.))
                .overflow_y_scroll()
                .children(
                    items
                        .into_iter()
                        .enumerate()
                        .map(|(i, a)| news_item(a, &state, i)),
                )
                .into_any_element()
        };

        div()
            .flex()
            .flex_1()
            .h_full()
            .min_w(px(0.))
            .min_h(px(0.))
            .flex_col()
            .bg(Theme::surface())
            .rounded(Theme::radius_panel())
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(24.))
                    .py(px(14.))
                    .border_b_1()
                    .border_color(Theme::surface_raised())
                    .child(icon("icons/newspaper.svg", 14., Theme::text_faint()))
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_faint())
                            .child(crate::i18n::t().news_header),
                    ),
            )
            .child(body)
    }
}

fn news_item(a: ModpackAnnouncement, state: &Entity<AppState>, idx: usize) -> gpui::AnyElement {
    let (title, excerpt) = parse_body(&a.body);
    // GPUI doesn't wrap or ellipsize a single text run, so cap to one visible
    // line based on the ~252px content width inside the 280px panel.
    let title = truncate(&title, 30);
    let excerpt = truncate(&excerpt, 34);
    let date_label = relative_time(&a.date);
    let body = a.body.clone();
    let state = state.clone();
    let id = SharedString::from(format!("news-{idx}"));

    let date_row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .child(icon("icons/clock.svg", 11., Theme::text_muted()))
                .child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(Theme::text_muted())
                        .child(date_label),
                ),
        )
        .child(icon("icons/arrow-up-right.svg", 14., Theme::text_faint()));

    let mut item = div()
        .id(id)
        .flex()
        .flex_col()
        .flex_shrink_0()
        .w_full()
        .gap(px(4.))
        .p(px(10.))
        .rounded(Theme::radius_card())
        .cursor_pointer()
        .hover(|s| s.bg(Theme::surface_raised()))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            state.update(cx, |s, cx| s.open_news(body.clone(), cx));
        })
        .child(date_row)
        .child(
            crate::widgets::emoji::line(&title, 14.)
                .w_full()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Theme::text()),
        );
    if !excerpt.is_empty() {
        item = item.child(
            crate::widgets::emoji::line(&excerpt, 12.).w_full().text_color(Theme::text_faint()),
        );
    }
    item.into_any_element()
}

fn truncate(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    let cut: String = s.chars().take(max_chars).collect();
    format!("{}…", cut.trim_end())
}

fn parse_body(body: &str) -> (String, String) {
    let lines: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('@'))
        .collect();
    if lines.is_empty() { return (String::new(), String::new()); }
    let title = strip_md(lines[0]);
    let rest = strip_md(&lines[1..].join(" "));
    (title, rest)
}

fn strip_md(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '#' | '*' | '_' | '`' | '>'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Parse the announcement date and render it as a localized relative time
/// (the formatting itself lives in `i18n::relative_time`).
fn relative_time(date_str: &str) -> String {
    let now = OffsetDateTime::now_utc();

    let dt = if let Ok(dt) = OffsetDateTime::parse(date_str, &Rfc3339) {
        dt
    } else if date_str.len() >= 10 {
        match parse_date_only(&date_str[..10]) {
            Some(dt) => dt,
            None => return date_str.to_string(),
        }
    } else {
        return date_str.to_string();
    };

    let diff = now - dt;
    let secs = diff.whole_seconds().max(0) as u64;
    crate::i18n::relative_time(secs)
}

fn parse_date_only(s: &str) -> Option<OffsetDateTime> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 { return None; }
    let y = parts[0].parse::<i32>().ok()?;
    let m = parts[1].parse::<u8>().ok()?;
    let d = parts[2].parse::<u8>().ok()?;
    let month = match m {
        1 => time::Month::January,   2 => time::Month::February,
        3 => time::Month::March,     4 => time::Month::April,
        5 => time::Month::May,       6 => time::Month::June,
        7 => time::Month::July,      8 => time::Month::August,
        9 => time::Month::September, 10 => time::Month::October,
        11 => time::Month::November, 12 => time::Month::December,
        _ => return None,
    };
    let date = time::Date::from_calendar_date(y, month, d).ok()?;
    Some(date.midnight().assume_utc())
}

