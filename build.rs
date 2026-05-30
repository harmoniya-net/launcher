//! Build script: embed the app icon into the Windows executable so Explorer,
//! the taskbar, and the title bar show the Harmoniya logo. No-op on other hosts
//! (Linux/macOS get their icons via the desktop entry / bundle instead).

fn main() {
    #[cfg(windows)]
    embed_windows_icon();
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
