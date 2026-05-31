# 01 · Module Boundaries & Separation

## Verdict: A− — strong layering, one misplaced feature

The workspace boundaries are genuinely good and should be preserved as-is.

## What's well-separated (keep)

- **Crate split is clean and acyclic.** `harmoniya-api` (backend client/auth) →
  `harmoniya-launch` (install/launch pipeline, depends on api) → `mc-skin`
  (pure rasterizer, zero project deps). The binary composes all three. Each
  crate has a focused `description` in its manifest and earns its boundary.
- **`mc-skin` is a model citizen:** pure function of its inputs
  (`render.rs:5-7` documents this), no GPUI/network leakage. The GPUI wrapper
  lives correctly in the binary (`views/skin/viewer.rs`).
- **State-by-concern is exemplary.** `AppState` is one entity whose behavior is
  split across `state/{session,catalog,launch_flow,skin,ui}.rs`, each an
  `impl AppState` block with a one-line module doc (`state/mod.rs:1-7`). The
  split is coherent and non-overlapping — a great pattern to keep.
- **`pipeline.rs` boundary is right:** it owns the `opys_runtime` translation
  (progress `Tracker`, error `classify`) so the UI only ever sees the launcher's
  own `LaunchState`/`LaunchMsg` types — the runtime crate never leaks upward.

## Boundary issues

### [High] Launcher settings live inside the Skin page
`views/skin/page.rs` (368 LOC) is really two features welded together: the
`SkinView` shell (nav sidebar + tab routing, `:15-151`) **and** the entire
Launcher-settings UI (`launcher_settings` `:155-262`, `close_to_tray_section`
`:265-319`). The settings feature has nothing to do with skins; it's only here
because both are reachable from the same side-nav. **Move** `launcher_settings`
+ `close_to_tray_section` into `views/skin/launcher_settings.rs` (or a top-level
`views/settings/`), leaving `page.rs` as the thin tab container. This also fixes
the file-size finding in §2.

### [Med] Two view layers reach across the `account`↔`skin` boundary
`views/skin/page.rs:13` imports `crate::views::account::user_bar::UserBar`, and
`account` and `skin` both depend on widgets. `UserBar` is shared chrome, not an
"account" view — it belongs in `widgets/` or a `views/common/`. Small move,
clarifies that the `account`/`skin` sibling modules shouldn't depend on each
other's internals.

### [Med] `now_ms()` duplicated across crate boundaries
Identical helper defined in `harmoniya-launch/pipeline.rs:127` **and**
`harmoniya-api/auth/mod.rs:27`. A clock helper is a `harmoniya-api` (or a tiny
shared util) concern; define once and reuse. See §4.

### [Low] `widgets/` vs view-local helpers is slightly arbitrary
Genuinely reusable building blocks (`modal`, `icon`, `emoji`, `markdown`) sit in
`widgets/`, but equally-reusable ones live as private free fns inside views:
`toggle_switch` (two copies, §4), `action_button`/`reset_link`/`field_card`
(`settings_modal.rs`, `skin_form.rs`), `nav_item` (`skin/page.rs`). As the
refactor extracts shared widgets (§4), route them through `widgets/` so the
boundary means "reusable UI" consistently.

## Module-doc coverage

Most modules carry a `//!` header explaining intent (`state/*`, `pipeline.rs`,
`render.rs`, `tray.rs`, `options.rs`, `viewer.rs`). A few public modules lack
one — `app.rs`, `theme.rs`, several `views/account/*`. Low priority, but worth a
one-liner each during the pass for consistency.

## Platform abstraction (good example)
`shell/tray.rs` cleanly isolates 3 backends behind one API via `#[cfg]` submodules
(`linux` ksni / `desktop` tray-icon / no-op fallback), with `pub use` selecting
the right `spawn`. `shell/window_ctl.rs` follows the same shape. This is the
right model for the platform code; the only nit is file size (§2).
