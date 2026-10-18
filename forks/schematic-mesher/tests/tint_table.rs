//! Table lint: `data/tints.json` and the entity texture names the mesher asks
//! for, checked against the newest real game version in the baked bundle.
//!
//! The bundle is built by `cargo xtask bake-blocks` into
//! `target/block-assets/blocks.dat.z` (or named by `LUMILIO_BLOCK_BUNDLE`); it
//! is not in Git, so without it these tests say so and pass. This is a
//! LumilioCL local change (see `forks/README.md`, plan schematic-studio T5).

use std::path::PathBuf;

use lumilio_block_assets::{BakedPack, Bundle};
use schematic_mesher::{tint_pattern_matches, tint_table_blocks, ResourcePack, TintProvider};

/// The version the lint reads: the newest release in the bundle.
const NEWEST: &str = "26.3";

fn newest_pack() -> Option<BakedPack> {
    let path = std::env::var_os("LUMILIO_BLOCK_BUNDLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/block-assets/blocks.dat.z")
        });
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!(
            "skipped: no baked bundle at {} (cargo xtask bake-blocks)",
            path.display()
        );
        return None;
    };
    Some(Bundle::read(&bytes).unwrap().pack(NEWEST).unwrap())
}

#[test]
fn every_tint_rule_names_a_real_block() {
    let Some(pack) = newest_pack() else { return };
    let blocks: Vec<&String> = pack.assets.blockstates.keys().collect();
    let unknown: Vec<&str> = tint_table_blocks()
        .into_iter()
        .filter(|pattern| !blocks.iter().any(|id| tint_pattern_matches(pattern, id)))
        .collect();
    assert!(unknown.is_empty(), "no {NEWEST} block for {unknown:?}");
}

/// The baker's samples are the colors Minecraft shows in plains, so the
/// sampling picks the game's pixel.
#[test]
fn colormap_samples_are_the_plains_colors() {
    let Some(pack) = newest_pack() else { return };
    let samples = &pack.assets.tint_samples;
    assert_eq!(samples.get("grass"), Some(&[0x91, 0xBD, 0x59]));
    assert_eq!(samples.get("foliage"), Some(&[0x77, 0xAB, 0x2F]));
    assert_eq!(samples.get("dry_foliage"), Some(&[0xA3, 0x75, 0x46]));
    let mut provider = TintProvider::new();
    provider.sample_colormaps(&ResourcePack::from_baked(&pack).colormap_samples);
    assert_eq!(provider.colors().grass, TintProvider::new().colors().grass);
}

/// Every entity texture name written in the mesher's source (not built at
/// run time) is found in the newest pack, directly or through the baker's
/// rename table.
#[test]
fn entity_texture_names_resolve() {
    let Some(pack) = newest_pack() else { return };
    let pack = ResourcePack::from_baked(&pack);
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/mesher");
    let mut missing = Vec::new();
    let mut checked = 0;
    for file in walk(&source) {
        let text = std::fs::read_to_string(&file).unwrap();
        for name in text.split('"').skip(1).step_by(2) {
            let is_name = name.starts_with("entity/")
                && !name.contains(['{', ' ', '\\'])
                && !name.ends_with('/');
            if !is_name {
                continue;
            }
            checked += 1;
            if pack.get_texture(name).is_none() && !GONE.iter().any(|gone| name.starts_with(gone)) {
                missing.push(format!("{name} ({})", file.display()));
            }
        }
    }
    // Most entity textures are named in data/entity_bindings.json now and
    // checked by the entity_binding_lint test; the code keeps the few that
    // composition draws (banner base, villager clothes, pot sides, skins).
    assert!(checked > 5, "only {checked} names found");
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "{NEWEST} has no {missing:#?}");
}

/// Name prefixes whose textures the newest version removed: 26.2 draws signs
/// and hanging signs from block models, and so does the mesher there
/// (`block_model_entities`).
const GONE: &[&str] = &["entity/signs/"];

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    files
}
