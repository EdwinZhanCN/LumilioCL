//! Windows executable resources: the icon Explorer, the taskbar and the title
//! bar show, and VERSIONINFO (winresource reads it from the package version).
//! GPUI's `windows-manifest` feature already embeds the manifest, so none is
//! set here (assets/icons/PACKAGING.md §3).

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let icon = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/icons/windows/LumilioCL.ico"
    );
    println!("cargo:rerun-if-changed={icon}");
    // The MSVC main thread gets 1 MB by default, which GPUI's deep element
    // trees can exhaust; Zed links its GPUI app with 8 MB for the same reason.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
    let mut resources = winresource::WindowsResource::new();
    resources.set_icon(icon);
    resources.set("FileDescription", "LumilioCL");
    resources.set("ProductName", "LumilioCL");
    resources.compile().expect("embed Windows resources");
}
