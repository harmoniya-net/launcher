# 04 · Code Reuse & Duplication

The highest-leverage section: several mechanical duplications that the refactor
can collapse with low risk. Ordered by payoff.

### [High] View boilerplate — `observe → notify` ×15
Every view repeats this verbatim in `new()`:
```rust
cx.observe(&state, |_, _, cx| cx.notify()).detach();
```
Sites: `hero.rs:70`, `settings_modal.rs:28`, `skin_form.rs:31`,
`server_list.rs:78`, `launch_modal.rs:22`, `news_panel.rs`, `news_modal.rs`,
`user_bar.rs`, `right_panel.rs`, `description.rs`, `login.rs`,
`account/page.rs`, `skin/page.rs:25`, `skin/viewer.rs:103` (custom),
`app.rs:26` (custom). **Proposal:** a tiny helper
`fn observe_repaint(state: &Entity<AppState>, cx: &mut Context<impl Render>)`
or an extension trait `cx.repaint_on(&state)`. Collapses 13 identical sites; the
2 custom observers stay as-is.

### [High] Token-refresh idiom ×3 (+ persist ×N)
The "refresh access token if expired, adopt the rotated tokens" dance is written
three times against the same `coordinated_refresh` primitive (`auth/mod.rs:124`):
- `state/mod.rs:221` `with_access_token` (the most complete version — also
  retries once on failure)
- `skin_form.rs:264` `ensure_token`
- `pipeline.rs:343-351` inline in `run_inner`

And the **adopt-and-persist** tail is copy-pasted:
```rust
if let Some(t) = refreshed { state.tokens = Some(t.clone()); let _ = auth::storage::save(&t); }
```
in `state/session.rs:19` and `state/skin.rs:46`; the same persist logic again in
`skin_form.rs:212-213,249-250` and `launch_flow.rs:110-112`.
**Proposal:** one `harmoniya-api` primitive, e.g.
`auth::with_fresh_access(tokens, |access| async { … }) -> (T, Option<Tokens>)`,
and an `AppState::adopt_tokens(&mut self, Option<Tokens>)` for the persist tail.
Removes the most consequential duplication in the codebase.

### [High] `Callback` type alias ×4
`type … = Arc<dyn Fn(&mut App) + 'static>` declared in `widgets/modal.rs:11`
(`CloseCallback`), `settings_modal.rs:15`, `news_modal.rs:8`,
`launch_modal.rs:13`. **Proposal:** define once (e.g. `widgets::modal::OnClose`)
and re-export; delete the three copies.

### [Med] Toggle switch ×2
The pill-and-knob switch is built twice, pixel-identical (w38×h22 track, w16 knob,
`ml(20|2)`, `rgb(0xffffff)` knob, accent/raised track):
`settings_modal.rs:104-127` (`feature_view`) and `skin/page.rs:266-290`
(`close_to_tray_section`). **Proposal:** `widgets::toggle_switch(on, on_click)`.

### [Med] Markdown per-word rendering ×2
`flush_para` (`markdown.rs:81-113`) and the blockquote arm (`:195-227`) run the
same `Run → split_whitespace → text_word/link_word` loop. **Proposal:** extract
`fn runs_to_items(runs, color) -> Vec<AnyElement>` and call it from both.

### [Med] `now_ms()` ×2
Identical UNIX-millis helper in `pipeline.rs:127` and `auth/mod.rs:27`. Define
once in `harmoniya-api` and reuse (crate boundary, see §1).

### [Med] "Enabled-gated clickable" idiom (pervasive)
`if enabled { base.cursor_pointer().on_mouse_down(…) } else { base /*opacity*/ }`
recurs across `settings_modal.rs` (`step_btn:379`, select chip `:415`,
`path_control:455`), `skin_form.rs` (`action_button:425`, `reset_link:444`,
`file_field`). **Proposal:** a `clickable_when(enabled, on_click)` element
extension to centralize the gate (and the disabled-opacity convention).

### [Low] Modpack → status logic ×2
`hero.rs:98-106` derives `PlayState`; `server_card.rs:199-207` derives
`ResolvedStatus`. Different outputs, same inputs (`maintaining`, `status`).
Consider a single `Modpack::availability()` returning a shared enum both views
map to UI from — keeps the maintenance/offline/online rule in one place.

### [Low] Skin/cape source plumbing
`viewer.rs` has parallel `skin_source`/`cape_source` (`:63,73`) and the two
arms of `load_source` (`:158,181`) differ only in fetch path. A `decode_store`
tail closure removes ~25 lines.
