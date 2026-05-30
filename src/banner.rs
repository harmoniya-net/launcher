/// Rewrite a Petal CDN banner URL with explicit `w` and `h` query params
/// so the server returns a pre-cropped image at the display aspect. This lets
/// us use `ObjectFit::Fill` (which respects parent corner_radii) without the
/// visible stretching that `Fill` would otherwise cause on a 3.2:1 source.
pub fn at_size(url: &str, w: u32, h: u32) -> String {
    let (base, query) = match url.split_once('?') {
        Some((b, q)) => (b, q),
        None => return format!("{url}?w={w}&h={h}"),
    };

    let mut pairs: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .filter(|(k, _)| k != "w" && k != "h")
        .collect();
    pairs.push(("w".into(), w.to_string()));
    pairs.push(("h".into(), h.to_string()));

    let mut rebuilt = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in pairs.iter() { rebuilt.append_pair(k, v); }
    format!("{base}?{}", rebuilt.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_w_h() {
        let s = at_size("https://x/y.webp?w=1280&h=400&format=webp", 820, 290);
        assert!(s.contains("format=webp"));
        assert!(s.contains("w=820"));
        assert!(s.contains("h=290"));
        assert!(!s.contains("w=1280"));
    }

    #[test]
    fn adds_when_missing() {
        let s = at_size("https://x/y.webp", 820, 290);
        assert_eq!(s, "https://x/y.webp?w=820&h=290");
    }
}
