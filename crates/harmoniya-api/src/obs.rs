//! Logging / observability helpers shared across the launcher and its crates.
//!
//! The launcher logs through `tracing` to a rolling daily file (see the
//! binary's `init_logging`). The single rule that matters most for diagnosing
//! field reports: **always log an error's full cause chain, never just its top
//! message.**
//!
//! `anyhow::Error`'s default `Display` (`"{e}"`) prints only the outermost
//! context. For a failed request that's the near-useless `"error sending
//! request for url (…)"`, which hides *why* it failed — DNS, a refused
//! connection, a TLS-trust rejection, or a timeout all collapse to the same
//! string. The alternate form `"{e:#}"` walks `source()` and joins every cause
//! with `": "`. [`Chain`] wraps that so call sites read as a structured field:
//!
//! ```ignore
//! tracing::warn!(error = %obs::Chain(&e), "fetch_me failed");
//! ```
//!
//! For this to actually carry the underlying transport error, the chain must be
//! preserved on the way up: use `anyhow::Context` (`.context("…")`), which
//! keeps the source error, rather than `anyhow!("…: {e}")`, which flattens the
//! source into a string and severs the chain.

use std::fmt;

/// Display adapter that renders an [`anyhow::Error`] as its full, `": "`-joined
/// cause chain — equivalent to `format!("{e:#}")` — for use as a `tracing`
/// field:
///
/// ```ignore
/// tracing::warn!(error = %obs::Chain(&e), "request failed");
/// ```
pub struct Chain<'a>(pub &'a anyhow::Error);

impl fmt::Display for Chain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `{:#}` is anyhow's chain formatter: outer context first, then each
        // `source()` joined with ": ".
        write!(f, "{:#}", self.0)
    }
}

/// Render a foreign error's full `": "`-joined `source()` chain into a `String`:
/// `"<display>: <source>: <source.source>: …"`.
///
/// Use this when wrapping a non-`anyhow` error — chiefly [`reqwest::Error`] —
/// into an `anyhow!` *message* so the underlying transport cause survives into
/// the log line. A bare `anyhow!("ctx: {e}")` formats `reqwest::Error` with its
/// plain `Display`, which is only the opaque outer layer
/// (`"error sending request for url (…)"`); the real reason — `dns error`,
/// `tcp connect error: Connection refused`, or `invalid peer certificate:
/// UnknownIssuer` — lives one or more `source()` levels down and is otherwise
/// lost. Folding the chain into the message (rather than attaching it via
/// `.context()`) keeps the outer prefix intact, which the auth-failure
/// classifiers in the launcher match on (`starts_with("refresh:")`, etc.).
pub fn cause_chain<E: std::error::Error + ?Sized>(err: &E) -> String {
    use std::fmt::Write as _;
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        // Skip a cause that the parent already inlined, so we don't print the
        // same text twice for errors whose `Display` repeats their source.
        let text = cause.to_string();
        if !out.ends_with(&text) {
            let _ = write!(out, ": {text}");
        }
        source = cause.source();
    }
    out
}
