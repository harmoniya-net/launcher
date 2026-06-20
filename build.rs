//! Build script: embed the app icon into the Windows executable so Explorer,
//! the taskbar, and the title bar show the Harmoniya logo. No-op on other hosts
//! (Linux/macOS get their icons via the desktop entry / bundle instead).

fn main() {
    #[cfg(windows)]
    embed_windows_icon();

    // Cross-compiling to Windows from another host: the native icon path above
    // (winresource) and gpui's manifest are both host-gated, so embed the icon +
    // manifest here instead. Only on a cross build — a native Windows build gets
    // them the usual way, and embedding twice would conflict.
    let target_windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    if target_windows && !cfg!(windows) {
        embed_resources_cross();
    }
}

/// Embed the app manifest and icon when cross-linking to Windows.
///
/// The manifest goes through the (lld-)link linker flags (no resource compiler
/// needed). The icon needs a compiled resource, so we generate a tiny `.rc`
/// referencing the committed `.ico` and compile it with `llvm-rc` (LLVM's
/// rc.exe-compatible resource compiler, which runs natively on Linux), then
/// hand the resulting `.res` to the linker.
fn embed_resources_cross() {
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");

    // Manifest (comctl32 v6 + DPI) via linker flags.
    let manifest = format!("{manifest_dir}/assets/windows/harmoniya.manifest.xml");
    println!("cargo:rerun-if-changed={manifest}");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{manifest}");

    // Icon via a compiled .res.
    let ico = format!("{manifest_dir}/assets/windows/harmoniya.ico");
    println!("cargo:rerun-if-changed={ico}");
    let rc_path = PathBuf::from(&out_dir).join("harmoniya_icon.rc");
    let res_path = PathBuf::from(&out_dir).join("harmoniya_icon.res");
    // Resource id 1, type ICON → becomes the application (lowest-id) icon.
    std::fs::write(&rc_path, format!("1 ICON \"{ico}\"\n")).expect("write .rc");

    let llvm_rc = std::env::var("LLVM_RC").unwrap_or_else(|_| "llvm-rc".to_string());
    match Command::new(&llvm_rc)
        .arg("/FO")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
    {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
        // Icon is cosmetic — never fail the build over it, just warn.
        Ok(s) => println!("cargo:warning=llvm-rc failed ({s}); building without an app icon"),
        Err(e) => {
            println!("cargo:warning=llvm-rc not found ({e}); building without an app icon (set LLVM_RC)")
        }
    }
}

/// Generate a multi-size `.ico` from `assets/logo.png` and embed it as the
/// executable's default icon resource. Runs only when building on Windows; the
/// build dependencies it uses are gated to `cfg(windows)` in Cargo.toml.
#[cfg(windows)]
fn embed_windows_icon() {
    use std::path::PathBuf;

    println!("cargo:rerun-if-changed=assets/logo.png");

    let src = image::open("assets/logo.png")
        .expect("decode assets/logo.png")
        .to_rgba8();

    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let resized =
            image::imageops::resize(&src, size, size, image::imageops::FilterType::Lanczos3);
        let entry = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&entry).expect("encode .ico entry"));
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("harmoniya.ico");
    dir.write(std::fs::File::create(&out).expect("create .ico"))
        .expect("write .ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(out.to_str().expect("ico path is utf-8"));
    res.compile().expect("embed windows resources");
}
