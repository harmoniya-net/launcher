# 03 · Idiomatic Rust

## Verdict: A− — idiomatic, with a few mechanical smells

### Error handling (strong)
- Only **8** `unwrap()`/`expect()` outside tests, plus **10** `mutex.lock().unwrap()`
  (idiomatic — a poisoned lock is unrecoverable). No reckless unwrapping.
- Library crates use `thiserror`-style typed errors where it matters
  (`opys_runtime::InstallError` mapped via `classify` in `pipeline.rs:281`) and
  `anyhow` for opaque flows — the right division.
- Network calls consistently `.map_err(|e| anyhow!("context: {e}"))` so failures
  are legible (`auth/mod.rs`, `services/yggdrasil.rs`, `services/modpacks.rs`).
- Graceful degradation is deliberate: `config::load_json(...).unwrap_or_default()`
  for user files (`state/mod.rs:150-152`), and image/skin fetch failures silently
  `return` rather than crash (`state/skin.rs:69-75`, `viewer.rs:161`).

### [Med] `#[allow(clippy::too_many_arguments)]` — 5 sites
A real signal that data wants bundling, not suppressing:
`server_card.rs:18` (10 args), `settings_modal.rs:91` (`feature_view`),
`:147` (`text_control`), `:302` (`slider_control`), `pipeline.rs:331` (`run_inner`).
- For `server_card`, group the per-frame animation inputs
  (`prev_h`, `target_h`, `banner_opacity`, `shadow_opacity`, `banner`) into a
  `CardFrame` struct — drops it to ~5 params and pairs the values that always
  travel together.
- For the settings builders, pass a small `FieldCtx { modpack_id, name, saved,
  enabled }` borrow instead of 4 separate threaded params.

### [Med] Dead state variables in `markdown.rs`
`in_code` and `in_list` are written but never read; `markdown.rs:278`
(`let _ = (in_code, in_list);`) exists only to silence the unused warning. Either
use them to guard content handling, or delete the fields and the `let _`.

### [Low] Clone-heavy render paths
`ServerList::render` clones `groups`, `modpacks`, `favourites` every frame
(`server_list.rs:277-279`); `SkinForm::save` clones pending skin/cape
(`skin_form.rs:184-185`). Most are forced by GPUI's `read`-then-detach ownership
and are cheap (`Arc`, small `Vec`), so this is **mostly inherent, not a bug**.
Opportunistic win: read-and-map to only the fields needed rather than cloning
whole collections, where the borrow checker allows.

### Good idiomatic patterns (keep)
- `let Some(x) = … else { return; }` guard-lets used consistently across state
  methods (`state/session.rs:11`, `launch_flow.rs:43`, `skin.rs:36`).
- Custom serde `Visitor`s to absorb the CMS's stringly-typed numbers/bools
  (`options.rs:125-176`) — correct and well-scoped.
- `Arc::ptr_eq` for cheap source-identity checks (`viewer.rs:56`); pointer-compare
  caching keyed on URL (`state/mod.rs:119-123`).
- Iterator pipelines over index loops throughout (`catalog.rs:64-75`,
  `modpacks.rs:118-123`, `server_list.rs:112-121`).
- `&'static str` returned from enum label methods (`pipeline.rs:45`,
  `:88`) — no needless allocation.

### [Nit] Small idiom tweaks
- `Provider::scope(&self)` ignores `self` and always returns `OAUTH_SCOPE`
  (`auth/mod.rs:74`) — make it an associated const or drop the param.
- `file_label` (`launch_modal.rs:43`) reimplements basename extraction; prefer a
  shared `Path::file_name` helper (also wanted by `skin_form.rs:87,119`).
- `hash_id` (`server_card.rs:209`) hand-rolls `DefaultHasher` for an element id;
  fine, but a `SharedString` of the modpack id would be simpler and stable.
