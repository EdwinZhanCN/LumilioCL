//! Embeds the release notes (`release-notes/<version>.md`, with an optional
//! `<version>.en.md`) so the launcher shows them after an update without
//! reaching the network (`release_notes` module).

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let dir = Path::new(&manifest).join("../../release-notes");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut versions: Vec<String> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    let version = name.strip_suffix(".md")?;
                    (version.starts_with(|c: char| c.is_ascii_digit()) && !version.ends_with(".en"))
                        .then(|| version.to_owned())
                })
                .collect()
        })
        .unwrap_or_default();
    versions.sort();
    let mut out = String::from("&[\n");
    for version in &versions {
        let chinese = dir.join(format!("{version}.md")).display().to_string();
        let english = dir.join(format!("{version}.en.md"));
        let english = if english.exists() {
            format!("Some(include_str!({:?}))", english.display().to_string())
        } else {
            "None".to_owned()
        };
        writeln!(
            out,
            "    ReleaseNotes {{ version: {version:?}, chinese: include_str!({chinese:?}), english: {english} }},"
        )
        .expect("writing to a String");
    }
    out.push(']');
    let target =
        Path::new(&std::env::var("OUT_DIR").expect("set by cargo")).join("release_notes.rs");
    std::fs::write(target, out).expect("write the embedded release notes");
}
