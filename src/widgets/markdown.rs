use std::sync::Arc;

use gpui::{
    AnyElement, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement,
    SharedString, Styled, div, img, px,
};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

use crate::theme::Theme;
use crate::widgets::emoji::{self, Seg};

const TEXT_PX: f32 = 14.;

/// Inline children for a word: plain text spans interleaved with Twemoji images.
fn word_children(word: &str) -> Vec<AnyElement> {
    let e = px((TEXT_PX * 1.25).round());
    emoji::segment(word)
        .into_iter()
        .map(|seg| match seg {
            Seg::Text(t) => div().child(t).into_any_element(),
            Seg::Emoji(url) => img(url).w(e).h(e).flex_shrink_0().into_any_element(),
        })
        .collect()
}

/// A non-clickable body word, with emoji rendered as Twemoji images.
fn text_word(word: &str, color: impl Into<Hsla>) -> AnyElement {
    let base = div().text_size(px(TEXT_PX)).text_color(color);
    if emoji::has_emoji(word) {
        base.flex().items_center().children(word_children(word)).into_any_element()
    } else {
        base.child(word.to_string()).into_any_element()
    }
}

/// A clickable link word that opens `url`, with emoji as Twemoji images.
fn link_word(word: &str, idx: usize, id_prefix: &str, url: Arc<String>) -> AnyElement {
    let el = div()
        .id(SharedString::from(format!("{id_prefix}-{idx}")))
        .text_size(px(TEXT_PX))
        .text_color(Theme::accent())
        .cursor_pointer()
        .hover(|s| s.text_color(Theme::text()))
        .on_mouse_down(MouseButton::Left, move |_, _, _cx| {
            let _ = open::that(url.as_str());
        });
    if emoji::has_emoji(word) {
        el.flex().items_center().children(word_children(word)).into_any_element()
    } else {
        el.child(word.to_string()).into_any_element()
    }
}

/// A run of text within a paragraph: either plain or a hyperlink.
enum Run {
    Text(String),
    Link { text: String, url: String },
}

/// Render a small subset of markdown into GPUI elements.
/// Links are rendered as clickable spans that open in the system browser.
pub fn render(source: &str) -> AnyElement {
    let mut out: Vec<AnyElement> = Vec::new();
    // Current paragraph as a sequence of runs
    let mut runs: Vec<Run> = Vec::new();
    // Plain-text accumulation buffer (flushed into a Run on link start/end)
    let mut buf = String::new();
    // URL of the currently open link tag, if any
    let mut link_url: Option<String> = None;

    let mut heading: Option<HeadingLevel> = None;
    let mut in_code = false;
    let mut in_blockquote = false;
    let mut list_items: Vec<String> = Vec::new();
    let mut in_list = false;

    // Flush runs into a paragraph div and push to `out`.
    // Each Run gets split into per-word flex items so flex_wrap can break
    // long text at word boundaries (flex_wrap only wraps items, not text
    // inside a single item, so a one-div-per-run approach would overflow).
    let flush_para = |runs: &mut Vec<Run>, out: &mut Vec<AnyElement>, blockquote: bool| {
        if runs.is_empty() { return; }
        let mut items: Vec<AnyElement> = Vec::new();
        let mut idx = 0usize;
        for run in std::mem::take(runs) {
            match run {
                Run::Text(t) => {
                    let color = if blockquote { Theme::text_faint() } else { Theme::text_secondary() };
                    for word in t.split_whitespace() {
                        items.push(text_word(word, color));
                        idx += 1;
                    }
                }
                Run::Link { text, url } => {
                    let url = Arc::new(url);
                    for word in text.split_whitespace() {
                        let url = url.clone();
                        items.push(link_word(word, idx, "md-link", url));
                        idx += 1;
                    }
                }
            }
        }
        out.push(
            div()
                .flex()
                .flex_wrap()
                .gap_x(px(4.))
                .gap_y(px(4.))
                .children(items)
                .into_any_element(),
        );
    };

    let parser = Parser::new(source);

    for event in parser {
        match event {
            // ── Block opens ──────────────────────────────────────────────
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => heading = Some(level),
                Tag::CodeBlock(_) => { in_code = true; }
                Tag::BlockQuote(_) => { in_blockquote = true; }
                Tag::List(_) => { in_list = true; list_items.clear(); }
                Tag::Item => buf.clear(),
                Tag::Link { dest_url, .. } => {
                    // Flush any preceding plain text into a Text run
                    if !buf.is_empty() {
                        runs.push(Run::Text(std::mem::take(&mut buf)));
                    }
                    link_url = Some(dest_url.to_string());
                }
                _ => {}
            },

            // ── Block closes ─────────────────────────────────────────────
            Event::End(end) => match end {
                TagEnd::Link => {
                    if let Some(url) = link_url.take() {
                        runs.push(Run::Link { text: std::mem::take(&mut buf), url });
                    }
                }
                TagEnd::Paragraph => {
                    if !buf.is_empty() {
                        runs.push(Run::Text(std::mem::take(&mut buf)));
                    }
                    flush_para(&mut runs, &mut out, in_blockquote);
                    in_blockquote = false;
                }
                TagEnd::Heading(_) => {
                    if !buf.is_empty() {
                        let text = std::mem::take(&mut buf);
                        let el = match heading.unwrap_or(HeadingLevel::H1) {
                            HeadingLevel::H1 => div()
                                .text_size(px(22.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::text()),
                            HeadingLevel::H2 => div()
                                .text_size(px(18.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::text()),
                            _ => div()
                                .text_size(px(15.))
                                .font_weight(FontWeight::BOLD)
                                .text_color(Theme::text()),
                        };
                        out.push(el.child(text).into_any_element());
                    }
                    heading = None;
                }
                TagEnd::CodeBlock => {
                    if !buf.is_empty() {
                        let text = std::mem::take(&mut buf);
                        out.push(
                            div()
                                .px(px(16.))
                                .py(px(12.))
                                .rounded(Theme::radius_block())
                                .bg(gpui::hsla(0.0, 0.0, 1.0, 0.06))
                                .font_family("monospace")
                                .text_size(px(13.))
                                .text_color(Theme::text_secondary())
                                .child(text)
                                .into_any_element(),
                        );
                    }
                    in_code = false;
                }
                TagEnd::BlockQuote(_) => {
                    if !buf.is_empty() {
                        runs.push(Run::Text(std::mem::take(&mut buf)));
                    }
                    // Wrap blockquote paragraph runs in a border-left div.
                    // Same per-word splitting as flush_para so long lines wrap.
                    if !runs.is_empty() {
                        let mut items: Vec<AnyElement> = Vec::new();
                        let mut idx = 0usize;
                        for run in std::mem::take(&mut runs) {
                            match run {
                                Run::Text(t) => {
                                    for word in t.split_whitespace() {
                                        items.push(text_word(word, Theme::text_faint()));
                                        idx += 1;
                                    }
                                }
                                Run::Link { text, url } => {
                                    let url = Arc::new(url);
                                    for word in text.split_whitespace() {
                                        items.push(link_word(word, idx, "md-bq-link", url.clone()));
                                        idx += 1;
                                    }
                                }
                            }
                        }
                        out.push(
                            div()
                                .pl(px(12.))
                                .border_l_2()
                                .border_color(Theme::accent())
                                .flex()
                                .flex_wrap()
                                .gap_x(px(4.))
                                .gap_y(px(4.))
                                .children(items)
                                .into_any_element(),
                        );
                    }
                }
                TagEnd::Item => {
                    list_items.push(std::mem::take(&mut buf));
                }
                TagEnd::List(_) => {
                    in_list = false;
                    let items = std::mem::take(&mut list_items);
                    out.push(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .pl(px(20.))
                            .children(items.into_iter().map(|i| {
                                div()
                                    .text_size(px(14.))
                                    .text_color(Theme::text_secondary())
                                    .child(format!("• {i}"))
                            }))
                            .into_any_element(),
                    );
                }
                _ => {}
            },

            // ── Content ──────────────────────────────────────────────────
            Event::Text(t) => buf.push_str(&t),
            Event::Code(c) => {
                buf.push('`');
                buf.push_str(&c);
                buf.push('`');
            }
            Event::SoftBreak | Event::HardBreak => buf.push(' '),
            Event::Rule => {
                if !buf.is_empty() {
                    runs.push(Run::Text(std::mem::take(&mut buf)));
                }
                flush_para(&mut runs, &mut out, false);
                out.push(
                    div()
                        .h(px(1.))
                        .w_full()
                        .bg(Theme::surface_raised())
                        .my(px(12.))
                        .into_any_element(),
                );
            }
            _ => {}
        }

        let _ = (in_code, in_list);
    }

    // Flush any trailing content
    if !buf.is_empty() {
        runs.push(Run::Text(buf));
    }
    flush_para(&mut runs, &mut out, false);

    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .text_size(px(14.))
        .children(out)
        .into_any_element()
}
