//! The vendored viewer (`assets/litematic-viewer/`, ADR 0027), served to the
//! webview from memory. Everything but `viewer.html` is stored gzip-compressed.

use flate2::read::GzDecoder;
use std::io::Read;

const JS: &str = "text/javascript";

macro_rules! gz {
    ($path:literal) => {
        include_bytes!(concat!("../../assets/litematic-viewer/", $path, ".gz")).as_slice()
    };
}

/// Request path, compressed bytes, content type.
const COMPRESSED: &[(&str, &[u8], &str)] = &[
    (
        "/sr/schematic-renderer.es.js",
        gz!("sr/schematic-renderer.es.js"),
        JS,
    ),
    ("/sr/index-39bc63bb.mjs", gz!("sr/index-39bc63bb.mjs"), JS),
    ("/sr/N8AO-1a125dcc.mjs", gz!("sr/N8AO-1a125dcc.mjs"), JS),
    (
        "/sr/GammaCorrectionEffect-dd8cf165.mjs",
        gz!("sr/GammaCorrectionEffect-dd8cf165.mjs"),
        JS,
    ),
    (
        "/sr/TiltShiftPlaneEffect-9256529e.mjs",
        gz!("sr/TiltShiftPlaneEffect-9256529e.mjs"),
        JS,
    ),
    ("/three/three.module.js", gz!("three/three.module.js"), JS),
    ("/three/three.core.js", gz!("three/three.core.js"), JS),
    (
        "/nucleation/nucleation.js",
        gz!("nucleation/nucleation.js"),
        JS,
    ),
    (
        "/nucleation/nucleation-original.js",
        gz!("nucleation/nucleation-original.js"),
        JS,
    ),
    (
        "/nucleation/nucleation_bg.wasm",
        gz!("nucleation/nucleation_bg.wasm"),
        "application/wasm",
    ),
];

pub(super) const VIEWER: &[u8] = include_bytes!("../../assets/litematic-viewer/viewer.html");

/// A vendored file by request path, decompressed.
pub(super) fn find(path: &str) -> Option<(Vec<u8>, &'static str)> {
    if path == "/viewer.html" {
        return Some((VIEWER.to_vec(), "text/html"));
    }
    let (_, bytes, mime) = COMPRESSED.iter().find(|(name, ..)| *name == path)?;
    let mut out = Vec::new();
    GzDecoder::new(*bytes).read_to_end(&mut out).ok()?;
    Some((out, mime))
}

#[cfg(test)]
pub(super) fn paths() -> impl Iterator<Item = &'static str> {
    COMPRESSED.iter().map(|(name, ..)| *name)
}
