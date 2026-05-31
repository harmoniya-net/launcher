# 05 · Best Practices (in context)

## Verdict: B+ — mostly clean; a few recurring shortcuts

### [Med] Hardcoded colors bypass `Theme` — 29 sites
`Theme` (`src/theme.rs`) is the single source of palette truth, but **29**
`rgb(0x…)` literals sidestep it. Three appear often enough to be tokens:

| Hex | ×  | Meaning | Proposed token |
|-----|---:|---------|----------------|
| `0xffffff` | 7 | pure-white knob / hover | `Theme::on_accent()` / `text_strong()` |
| `0x4a4850` | 5 | row hover background | `Theme::surface_hover()` |
| `0x0e0d0f` | 5 | text on light/accent fills | `Theme::on_light()` |

`0x0e0d0f` alone recurs in `settings_modal.rs:413`, `launch_modal.rs:230`,
`server_card.rs:163`, `hero.rs:20` (`LABEL_DARK`). **Proposal:** add the three
tokens to `Theme` and replace the literals; leave genuinely one-off shades
(gradients, status pings) inline. Makes a future theme/dark-mode change tractable.

### [Med] Magic config filenames repeated
`"settings.json"` (×5), `"selection.json"` (×2), `"favourites.json"` (×2) appear
as string literals across `state/catalog.rs:22,28,113`, `state/ui.rs:38,46`,
`state/mod.rs:150-152`. A typo in one persists/loads the wrong file silently.
**Proposal:** `const` names (or a `ConfigFile` enum) in `harmoniya-api::config`,
and a `AppState::persist_settings()` wrapper so the save call isn't re-spelled.

### [Low] `AppState` exposes 25+ `pub` fields
`state/mod.rs:86-126` makes the entire model `pub`. Views mutate through methods
(good), but nothing enforces it — any view *could* poke `launch_state` directly.
Most fields are legitimately read across views, so a blanket `pub(crate)` is the
pragmatic floor; consider tightening the few that are write-only-internally
(`launch_task`, `running_task`, `pending_login_task`) to `pub(crate)` or private.

### [Low] Secret handling
Access/refresh tokens live in memory (`Tokens`) and the OS keyring via
`keyring` (`auth/storage.rs`) — correct. They do flow into `tracing` only on
decode-error paths with the body attached (`auth/mod.rs:156`). Acceptable for a
desktop app, but consider redacting token bodies from error logs before any
log-upload feature ships.

### [Low] Blocking work off the UI thread (done right — noted as a *good* practice)
Sync/blocking operations are correctly dispatched off the GPUI loop:
the OAuth loopback listener via `std::thread::spawn(...).join()`
(`auth/mod.rs:58`), the tokio bridge `http::on_tokio` for all network calls,
D-Bus tray setup on its own thread (`tray.rs:120`). No blocking-on-async found.
`std::fs::read` in `skin_form.rs:88,120` runs inside the picker mouse handler on
the UI thread — small local files, low risk, but the read could move into the
existing `on_tokio` block alongside the upload.

### [Low] `#[allow(dead_code)]` / warnings
Build emits 3 dead-code warnings (e.g. `AccountView.state` field never read).
Pre-existing, harmless, but worth clearing during the pass so warnings stay at
zero and real ones aren't lost in noise.

### Practices worth calling out as exemplary (keep)
- **Concurrency correctness:** `coordinated_refresh` (`auth/mod.rs:108-136`)
  solves refresh-token rotation races with a documented mutex + cache. Excellent.
- **Layout stability:** group heights reserved so selection/hover never reflow
  the list (`server_list.rs:140-150`) — thoughtful UX-driven invariant.
- **Comments explain *why*,** not what: `render.rs` UV-bias epsilon `:228-232`,
  the per-frame static easing rationale `server_list.rs:50-53`. Maintain this bar.
- **Tests** cover the gnarly CMS-shape deserialization (`options.rs:231-289`).
  The refactor should *add* coverage for the extracted token-refresh helper (§4).
