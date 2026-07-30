<p align="center">
  <img src="assets/logo.png" width="96" alt="Harmoniya logo">
</p>

<h1 align="center">Harmoniya Launcher</h1>

<p align="center">
  Native desktop launcher for the <a href="https://harmoniya.net">Harmoniya</a> Minecraft community — built with Rust and <a href="https://www.gpui.rs">GPUI</a>.
</p>

<p align="center">
  <a href="https://github.com/harmoniya-net/launcher/actions/workflows/ci.yml"><img src="https://github.com/harmoniya-net/launcher/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/harmoniya-net/launcher/releases/latest"><img src="https://img.shields.io/github/v/release/harmoniya-net/launcher" alt="Latest release"></a>
</p>

## Download

| Platform | Link |
|---|---|
| Windows | https://launcher.harmoniya.net/windows |
| Linux | https://launcher.harmoniya.net/linux |
| macOS (Apple Silicon) | https://launcher.harmoniya.net/macos |
| macOS (Intel) | https://launcher.harmoniya.net/macos-intel |

Or grab a specific build from [Releases](https://github.com/harmoniya-net/launcher/releases/latest) directly. The launcher checks for updates on startup and updates itself in place.

## What it does

Harmoniya Launcher installs and launches the community's Minecraft modpacks: sign in with a Microsoft/Mojang account, pick a modpack, press play. Installs and updates (via [`opys-runtime`](https://docs.rs/opys-runtime)) only touch the files that actually changed. It's a native Rust port of the community's web app, so the launch flow, modpack catalog, and account UI mirror the site.

## Building from source

```sh
cargo build --release        # optimized, matches the shipped binary
cargo run --profile fast     # release-speed runtime, much faster to compile — use this while iterating
```

Linux needs a few system libraries GPUI links against:

```sh
sudo apt install pkg-config cmake clang libxcb1-dev libxcb-xkb-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libxau-dev libxdmcp-dev
```

Windows and macOS build with just the standard MSVC / Xcode toolchains — no extra setup.

## Architecture

A Cargo workspace: the binary (`src/`) plus three GPUI-free library crates —

- **`crates/harmoniya-api`** — backend client: HTTP, config/data-dir persistence, OAuth2 + keyring auth, CMS/account/modpack services.
- **`crates/harmoniya-launch`** — the install → spawn pipeline and running-game registry, built on `opys-runtime`.
- **`crates/mc-skin`** — pure-`image` Minecraft skin rendering (head icons + the viewer rasterizer).

Inside `src/`: `shell/` (OS glue — tray, window show/hide, single-instance, self-update), `state/` (the single `AppState` entity), `views/` (GPUI UI), `widgets/`.

`vendor/gpui` carries a couple of small patches on top of upstream GPUI — see [`vendor/gpui/HARMONIYA_PATCH.md`](vendor/gpui/HARMONIYA_PATCH.md).

## Releases

Tagging `vX.Y.Z` and pushing the tag builds and publishes native Linux, Windows, and macOS (arm64 + x86_64) binaries via [`.github/workflows/release.yml`](.github/workflows/release.yml).
