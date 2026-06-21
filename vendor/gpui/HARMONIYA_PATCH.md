# Vendored gpui 0.2.2 — Harmoniya patch

This is an unmodified copy of the crates.io `gpui` 0.2.2 source (the `examples/`
directory is removed to keep the tree small) **except** for the changes below,
applied via `[patch.crates-io]` in the workspace root `Cargo.toml`.

Search the source for `HARMONIYA PATCH` to find every modified line.

## Change 1 — DirectWrite font-collection crash (Windows)

`src/platform/windows/direct_write.rs` — `DirectWriteTextSystem::new`

Upstream builds the system font collection with a bare `?`:

```rust
components.factory.GetSystemFontCollection(false, &mut result, true)?;
```

On some Windows installs this fails with `ERROR_MORE_DATA` (`0x800700EA`) —
caused by a corrupt Windows Font Cache or stale `HKLM\…\Fonts` registry entries
pointing at missing/garbage font files. Because it's a bare `?`, the whole text
system fails to construct and the app **panics during `Application::new()`**,
before any of our code runs ("Error creating DirectWriteTextSystem").

The launcher only ever renders bundled fonts (Inter + Twemoji images), so the
system font collection is just fallback/enumeration we never exercise. The patch
makes that call non-fatal: on failure it logs and falls back to the (empty)
custom font collection, so the app starts normally with bundled fonts instead of
crashing.

## Change 2 — hide-to-tray on Wayland (Linux)

Adds a `PlatformWindow::set_hidden(bool)` method (default no-op) exposed as
`Window::set_window_hidden`, implemented for the Wayland backend:

- `src/platform.rs` — trait method with a no-op default.
- `src/window.rs` — `Window::set_window_hidden` wrapper.
- `src/platform/linux/wayland/window.rs` — the real implementation, plus a
  `hidden` flag on `WaylandWindowState`.

The launcher hides its window to the system tray on close. The old path called
`xdg_toplevel.set_minimized`, but **wlroots compositors (Hyprland, Sway, …)
ignore minimize requests** — so the window simply never disappeared.

`set_hidden(true)` instead unmaps the surface by attaching a null buffer and
committing; while hidden, `draw`/`completed_frame` skip presenting so a stray
redraw can't re-map it. `set_hidden(false)` does an empty commit, which makes
the compositor send a fresh `xdg_surface.configure`; because we reset
`acknowledged_first_configure`, that runs the same frame → draw → buffer-attach
sequence as initial window creation, re-mapping the window. The `wl_surface`
(and the Blade/Vulkan swapchain built on it) is never destroyed, so the window
identity and renderer survive the cycle. This works on every compositor,
including wlroots.

## Why vendored

`gpui` 0.2.2 is the newest published version, and zed `main` / `gpui-ce` /
`gpui-unofficial` all carry the identical unfixed code — there is no fixed
release to upgrade to. When both changes land in a published gpui (the
DirectWrite fix upstreamed, and a portable window hide/show API), delete
`vendor/gpui` and the `[patch.crates-io]` block.
