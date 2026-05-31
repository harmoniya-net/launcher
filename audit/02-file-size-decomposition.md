# 02 · File Size & Decomposition

Target ≈150 LOC/file. The codebase median is well under that; the long tail
below is where the refactor should focus. LOC as of audit.

| File | LOC | Over? | Recommended action |
|------|----:|:-----:|--------------------|
| `views/account/settings_modal.rs` | 515 | ▲▲▲ | Split: view vs control widgets |
| `mc-skin/render.rs` | 491 | ▲▲▲ | Optional 3-way split (cohesive) |
| `views/skin/skin_form.rs` | 453 | ▲▲▲ | Split: view vs save/reset vs widgets |
| `harmoniya-launch/pipeline.rs` | 400 | ▲▲ | Split: progress tracker vs run flow |
| `views/account/server_list.rs` | 376 | ▲▲ | Extract easing/animation helpers |
| `views/skin/page.rs` | 368 | ▲▲ | Move out launcher settings (§1) |
| `views/skin/viewer.rs` | 339 | ▲ | Trim `load_source` duplication |
| `views/account/hero.rs` | 296 | ▲ | Extract `play_button` |
| `widgets/markdown.rs` | 294 | ▲ | Extract `runs_to_items` (§4) |
| `services/options.rs` | 289 | ▲ | Acceptable (types+resolve+tests) |
| `views/account/launch_modal.rs` | 283 | ▲ | Acceptable (cohesive sub-views) |
| `views/account/news_panel.rs` | 253 | ◦ | Borderline — leave |
| `shell/tray.rs` | 239 | ◦ | Optional per-OS submodule files |
| `views/account/server_card.rs` | 214 | ◦ | Bundle anim args into a struct |

Everything below 200 is at or near target and needs no action.

## Concrete split seams

### `settings_modal.rs` (515) — [High]
Two clear strata. **(a) View + self-methods** (the `SettingsModal` struct,
`field_view` `:44`, `feature_view` `:92`, `text_control` `:148`, `Render`) stay.
**(b) The stateless control builders** — `field_card` `:297`, `slider_control`
`:302`, `step_btn` `:358`, `select_control` `:394`, `path_control` `:434`,
`header_text` `:283` — move to `settings_controls.rs`. Cuts the file roughly in
half and isolates the `#[allow(clippy::too_many_arguments)]` builders.

### `skin_form.rs` (453) — [High]
Three strata: the `Render` impl (`:40-175`); the async `save`/`reset` methods
(`:179-262`, near-duplicates — see §4); and the free widget builders
`file_field` `:291`, `model_field` `:345`, `radio` `:371`, `action_button`
`:409`, `reset_link` `:433`. Extract the widgets to `skin_form_widgets.rs`; the
save/reset dedup (§4) shrinks the rest.

### `render.rs` (491) — [Low, optional]
One cohesive, heavily-documented algorithm — splitting is *optional*. If desired:
`geometry.rs` (`Vec3`, `rotate_*`, `Part`, `Vertex`, `Tri`, `part_triangles`),
`parts.rs` (`parts_for` `:84`, `cape_part` `:318`), `raster.rs` (`rasterize`,
`smooth_alpha`, `downsample_aa`). Keep the public API in `lib.rs`. Do **not**
touch the math.

### `pipeline.rs` (400) — [Med]
Natural seam between the **progress model** (`Tracker` `:194`, `FileState`,
`apply`/`snapshot`/`percent` `:207-279`) and the **launch flow** (`run` `:304`,
`run_inner` `:332`, `classify`, `launch_vars`, `uuid_from_ygg`). Move the tracker
to `pipeline/progress.rs`. The public phase/state types can stay in the parent.

### `server_list.rs` (376) — [Med]
The easing math is reusable and self-contained: `ease_in_out` `:63`,
`CardHeight` `:48`, `CardVisual` `:55`, `target_height` `:83`, plus the
height-consts `:20-41`. Extract to `card_anim.rs`, leaving `render_group`/`Render`
focused on layout.

### `hero.rs` (296) — [Med]
The play button is ~120 LOC of inline branching (`:129-212`, running / playable /
disabled). Extract `play_button(state, modpack, …) -> AnyElement` plus its
helpers `btn_shape` `:23`, `btn_content` `:33` and consts into `play_button.rs`.

### `viewer.rs` (339) — [Low]
Cohesive widget; the only fat is `load_source` `:155-219`, whose `Preview` and
`Url` arms share an identical decode-and-store tail. Factor that tail (§4); no
module split needed.

### `tray.rs` (239) — [Low, optional]
Already split by `#[cfg]` into `linux`/`desktop` modules. If you want every file
<150, promote them to `tray/linux.rs` + `tray/desktop.rs`. Current single-file
form is idiomatic and fine.
