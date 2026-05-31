# Harmoniya Launcher — Code-Quality Audit (Part 2: Structure)

A structural audit of the launcher workspace, complementing the Part-1 naming
pass. Goal: a map for the upcoming refactor — module boundaries, file size,
idiomatic Rust, reuse, and best practices.

## Scope & method

- **Codebase:** ~8.3k LOC Rust. One binary (`src/`) + 3 library crates
  (`harmoniya-api`, `harmoniya-launch`, `mc-skin`). GPUI UI.
- **Method:** every file ≥150 LOC read in full; all of `src/state` and the
  crate internals read; the rest reviewed by signature. Findings cite
  `path:line` and quote the construct. No code was changed for this audit.
- **Severity:** `High` (do before/with refactor) · `Med` (worth doing) ·
  `Low`/`Nit` (opportunistic).

## Documents

| # | Document | Focus |
|---|----------|-------|
| 1 | [01-module-boundaries.md](01-module-boundaries.md) | Crate/module responsibilities & leaks |
| 2 | [02-file-size-decomposition.md](02-file-size-decomposition.md) | Files >150 LOC + split seams |
| 3 | [03-idiomatic-rust.md](03-idiomatic-rust.md) | Error handling, params, clones, combinators |
| 4 | [04-code-reuse-duplication.md](04-code-reuse-duplication.md) | Duplication catalog + dedup proposals |
| 5 | [05-best-practices.md](05-best-practices.md) | Theme bypass, magic values, pub surface |

## Scorecard

| Dimension | Grade | One-line verdict |
|-----------|:-----:|------------------|
| Module boundaries | **A−** | Crate split + state-by-concern is exemplary; one feature misplaced. |
| File size | **B−** | 7 files materially over target; clear seams exist for each. |
| Idiomatic Rust | **A−** | Strong error handling, good async; a few `too_many_arguments` builders. |
| Code reuse | **B** | Several mechanical duplications (token refresh, view boilerplate, toggle). |
| Best practices | **B+** | Mostly clean; theme-bypass colors + magic config names recur. |

Overall the codebase is **well above average**: thoughtfully layered, heavily
documented, and idiomatic. The refactor is about *consolidation*, not rescue.

## Prioritized backlog (the high-leverage items)

1. **[High]** Extract a shared view helper for the 15× `observe→notify`
   boilerplate and hoist the 4× `Callback` alias → §4.
2. **[High]** Unify the token-refresh idiom duplicated across state/skin_form/
   pipeline into one `harmoniya-api` primitive → §4.
3. **[High]** Move the Launcher-settings feature out of `views/skin/page.rs`
   into its own module → §1, §2.
4. **[Med]** Split `settings_modal.rs` (515) and `skin_form.rs` (453) into
   view + control-widgets → §2.
5. **[Med]** Promote the 3 recurring hex colors (`0x0e0d0f`, `0x4a4850`,
   `0xffffff`) to `Theme` tokens; add a `toggle_switch` widget → §4, §5.
6. **[Med]** De-duplicate `markdown.rs` per-word rendering + drop its dead
   state vars → §3, §4.

## Notes for the refactor

- These crates are `publish = false`; renames/moves are low-risk (git + a
  passing `cargo check --workspace` is the safety net).
- The CMS-facing serde structs in `harmoniya-api` mirror the live petal schema —
  field names and `#[serde(rename)]` must not change without a schema deploy.
- Animation/easing math (`server_list.rs`, `server_card.rs`, `mc-skin`) is
  intricate but correct and well-commented; treat as *extract-only*, don't
  rewrite the algorithms during a structural pass.
