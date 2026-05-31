//! Splits text into plain runs and emoji, mapping each emoji to its Twemoji
//! image URL. We render emoji as Twemoji PNGs (fetched from the CDN, cached by
//! GPUI's image layer) instead of relying on whatever color-emoji font happens
//! to be installed.

use gpui::{Div, ParentElement, Styled, div, img, px};
use unicode_segmentation::UnicodeSegmentation;

/// Maintained Twemoji fork; 72×72 PNGs keyed by dash-joined codepoints.
const CDN: &str = "https://cdn.jsdelivr.net/gh/jdecked/twemoji@15.1.0/assets/72x72";

/// A piece of a string: plain text, or an emoji resolved to an image URL.
pub enum Segment {
    Text(String),
    Emoji(String),
}

/// True if the string contains at least one emoji grapheme.
pub fn has_emoji(input: &str) -> bool {
    input.graphemes(true).any(|g| emojis::get(g).is_some())
}

/// Split into alternating text / emoji segments, in order.
pub fn segment(input: &str) -> Vec<Segment> {
    let mut segs = Vec::new();
    let mut text = String::new();
    for g in input.graphemes(true) {
        if emojis::get(g).is_some() {
            if !text.is_empty() {
                segs.push(Segment::Text(std::mem::take(&mut text)));
            }
            segs.push(Segment::Emoji(twemoji_url(g)));
        } else {
            text.push_str(g);
        }
    }
    if !text.is_empty() {
        segs.push(Segment::Text(text));
    }
    segs
}

/// A single, non-wrapping line of text with emoji rendered as Twemoji images
/// (sized to `size`). The caller styles color / weight / width on the returned
/// row; `text_size` is set here so emoji and text stay coherent. Overflowing
/// content is clipped — intended for already-truncated previews.
pub fn line(text: &str, size: f32) -> Div {
    let glyph = px((size * 1.2).round());
    let mut row = div().flex().items_center().gap(px(2.)).overflow_hidden().text_size(px(size));
    for seg in segment(text) {
        row = match seg {
            Segment::Text(t) => row.child(div().flex_shrink_0().child(t)),
            Segment::Emoji(url) => row.child(img(url).w(glyph).h(glyph).flex_shrink_0()),
        };
    }
    row
}

/// Build the Twemoji filename URL for one emoji grapheme. Mirrors Twemoji's own
/// rule: drop the U+FE0F variation selector unless the sequence is a ZWJ
/// sequence (where it's significant).
fn twemoji_url(grapheme: &str) -> String {
    let has_zwj = grapheme.contains('\u{200d}');
    let code = grapheme
        .chars()
        .filter(|&c| has_zwj || c != '\u{fe0f}')
        .map(|c| format!("{:x}", c as u32))
        .collect::<Vec<_>>()
        .join("-");
    format!("{CDN}/{code}.png")
}
