//! Comments that credit upstream code name the right license (ADR 0011, 0022).
//!
//! HMCL is GPL-3.0-or-later and its files carry a copyright line; Modrinth App
//! is GPL-3.0-only. This project is AGPL-3.0, which makes it easy to write the
//! wrong one: in October 2026 five HMCL attributions said "AGPL-3.0" and had no
//! copyright line.

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Runs of consecutive comment lines, with the line each one starts on.
fn comment_blocks(text: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut current: Option<(usize, String)> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//") {
            let rest = rest.trim_start_matches(['/', '!']);
            let block = current.get_or_insert_with(|| (index + 1, String::new()));
            block.1.push_str(rest.trim());
            block.1.push(' ');
        } else if let Some(block) = current.take() {
            blocks.push(block);
        }
    }
    blocks.extend(current);
    blocks
}

/// What is wrong with one comment block, if it credits upstream code.
fn problem(block: &str) -> Option<&'static str> {
    if block.contains("Axolotl")
        && [".c", ".h", ".rs", ".vue"]
            .iter()
            .any(|ext| block.contains(ext))
        && !block.contains("GPL-3.0-only")
    {
        return Some("Axolotl code names GPL-3.0-only");
    }
    if block.contains("cubiomes-viewer")
        && [".cpp", ".h"].iter().any(|ext| block.contains(ext))
        && !block.contains("GPL-3.0")
    {
        return Some("cubiomes-viewer code names GPL-3.0");
    }
    for (source, license, copyright) in [
        ("cubiomes", "MIT", "Copyright (c) 2020 Cubitect"),
        (
            "fastanvil",
            "MIT OR Apache-2.0",
            "Copyright (c) 2020 Owen Gage",
        ),
        ("XaeroTools", "MIT", "Copyright (c) 2026 Dek"),
    ] {
        if block.contains(source)
            && !block.contains("cubiomes-viewer")
            && [".rs", ".c", ".h"].iter().any(|ext| block.contains(ext))
            && (!block.contains(license) || !block.contains(copyright))
        {
            return Some("map upstream credit needs its license and copyright");
        }
    }
    let cites_hmcl = block.contains("HMCL") && block.contains(".java");
    // A Modrinth credit names a file or a license; "after Modrinth App's
    // browse page (ADR 0022)" follows a design and copies no code.
    let cites_modrinth = block.contains("Modrinth")
        && block.contains("ADR 0022")
        && [".vue", ".ts", ".rs", "GPL"]
            .iter()
            .any(|mark| block.contains(mark));
    if (cites_hmcl || cites_modrinth) && block.contains("AGPL") {
        return Some("upstream code is GPL, not AGPL");
    }
    if cites_hmcl && !block.contains("GPL-3.0-or-later") {
        return Some("an HMCL attribution names GPL-3.0-or-later");
    }
    if cites_hmcl && !block.contains("Copyright") {
        return Some("an HMCL attribution carries the file's copyright line");
    }
    if cites_modrinth && !block.contains("GPL-3.0-only") {
        return Some("a Modrinth App attribution names GPL-3.0-only");
    }
    None
}

fn missing_credit(credit: &str) {
    assert!(problem(credit).is_some(), "{credit}");
}

#[test]
fn complete_map_credits_are_accepted() {
    for credit in [
        "Axolotl bridge.c GPL-3.0-only",
        "cubiomes-viewer map.cpp GPL-3.0",
        "cubiomes generator.h MIT Copyright (c) 2020 Cubitect",
        "fastanvil region.rs MIT OR Apache-2.0 Copyright (c) 2020 Owen Gage",
        "XaeroTools waypoints.rs MIT Copyright (c) 2026 Dek",
    ] {
        assert_eq!(problem(credit), None, "{credit}");
    }
}
#[test]
fn axolotl_guard_probe() {
    missing_credit("Axolotl bridge.c");
}
#[test]
fn viewer_guard_probe() {
    missing_credit("cubiomes-viewer map.cpp");
}
#[test]
fn cubiomes_guard_probe() {
    missing_credit("cubiomes generator.h");
}
#[test]
fn fastanvil_guard_probe() {
    missing_credit("fastanvil region.rs");
}
#[test]
fn xaerotools_guard_probe() {
    missing_credit("XaeroTools waypoints.rs");
}

#[test]
fn the_rules_catch_the_mistakes_they_are_for() {
    let good_hmcl = "Adapted from HMCL (`Skin.java`, Copyright (C) 2020 huangyuhui and \
                     contributors, GPL-3.0-or-later; ADR 0011).";
    assert_eq!(problem(good_hmcl), None);
    assert!(problem("Adapted from HMCL (`Skin.java`, AGPL-3.0; ADR 0011).").is_some());
    assert!(problem("Adapted from HMCL (`Skin.java`, GPL-3.0-or-later; ADR 0011).").is_some());
    assert_eq!(
        problem("After Modrinth App's `card.vue` (GPL-3.0-only; ADR 0022)."),
        None
    );
    assert!(problem("After Modrinth App's `card.vue` (AGPL-3.0; ADR 0022).").is_some());
    assert_eq!(
        problem("Slim wins when the profile has both, as in HMCL."),
        None
    );
    assert_eq!(
        problem("The browse page of Modrinth App, drawn our way (ADR 0022)."),
        None
    );
}

#[test]
fn upstream_attributions_name_the_right_license() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for member in fs::read_dir(&crates).unwrap().flatten() {
        rust_files(&member.path().join("src"), &mut files);
        rust_files(&member.path().join("tests"), &mut files);
    }
    files.sort();
    let this_file = Path::new(file!()).file_name().unwrap();
    let mut problems = Vec::new();
    for file in files {
        if file.file_name() == Some(this_file) {
            continue;
        }
        let text = fs::read_to_string(&file).unwrap();
        for (line, block) in comment_blocks(&text) {
            if let Some(problem) = problem(&block) {
                problems.push(format!("{}:{line}: {problem}", file.display()));
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}
