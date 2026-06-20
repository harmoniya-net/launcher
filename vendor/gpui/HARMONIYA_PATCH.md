# Vendored gpui 0.2.2 — Harmoniya patch

This is an unmodified copy of the crates.io `gpui` 0.2.2 source (the `examples/`
directory is removed to keep the tree small) **except** for one Windows-only
change, applied via `[patch.crates-io]` in the workspace root `Cargo.toml`.

## The change

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

Search the source for `HARMONIYA PATCH` to find the exact lines.

## Why vendored

`gpui` 0.2.2 is the newest published version, and zed `main` / `gpui-ce` /
`gpui-unofficial` all carry the identical unfixed code — there is no fixed
release to upgrade to. When the fix lands in a published gpui (or we upstream it
to gpui-ce), delete `vendor/gpui` and the `[patch.crates-io]` block.
