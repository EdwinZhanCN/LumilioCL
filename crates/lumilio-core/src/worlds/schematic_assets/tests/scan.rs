//! What a source holds, read without baking it (plan schematic-studio, T21).

use super::super::{LocalCache, Scan};
use super::zip_at;

#[test]
fn a_scan_counts_blocks_assets_and_models_a_custom_loader_draws() {
    let dir = tempfile::tempdir().unwrap();
    let jar = zip_at(
        &dir.path().join("machines.jar"),
        &[
            ("META-INF/neoforge.mods.toml", b"modId=\"machines\""),
            ("assets/machines/blockstates/press.json", b"{}"),
            ("assets/machines/blockstates/pipe.json", b"{}"),
            (
                "assets/machines/models/block/press.json",
                br#"{"loader":"neoforge:obj","model":"machines:models/press.obj"}"#,
            ),
            (
                "assets/machines/models/block/pipe.json",
                br#"{"loader":"neoforge:composite","children":{}}"#,
            ),
            (
                "assets/machines/models/block/casing.json",
                br#"{"parent":"minecraft:block/cube_all","textures":{"loader":"machines:block/x"}}"#,
            ),
            ("assets/machines/textures/block/casing.png", b"png"),
            ("assets/machines/lang/en_us.json", b"{}"),
            ("data/machines/recipes/press.json", b"{}"),
        ],
    );
    let cache = LocalCache::new(&dir.path().join("cache"));
    assert_eq!(
        cache.scan(&jar),
        Scan {
            blocks: 2,
            assets: 6,
            // `loader` under `textures` is a texture name, not a loader.
            custom_models: 2,
            unreadable: false,
        }
    );
}

#[test]
fn a_scan_is_kept_until_its_source_changes() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("pack.zip");
    zip_at(&pack, &[("assets/minecraft/blockstates/stone.json", b"{}")]);
    let cache = LocalCache::new(&dir.path().join("cache"));
    assert_eq!(cache.scan(&pack).blocks, 1);
    let kept: Vec<_> = std::fs::read_dir(dir.path().join("cache/local"))
        .unwrap()
        .filter_map(Result::ok)
        .collect();
    assert_eq!(kept.len(), 1, "one file per source");

    zip_at(
        &pack,
        &[
            ("assets/minecraft/blockstates/stone.json", b"{}"),
            ("assets/minecraft/blockstates/dirt.json", b"{}"),
        ],
    );
    assert_eq!(cache.scan(&pack).blocks, 2, "read again once it changed");
    assert_eq!(
        std::fs::read_dir(dir.path().join("cache/local"))
            .unwrap()
            .count(),
        1,
        "the changed source's scan replaced the old one"
    );
}

#[test]
fn a_folder_pack_is_scanned_and_a_file_that_is_no_zip_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let unpacked = dir.path().join("Unpacked");
    std::fs::create_dir_all(unpacked.join("assets/minecraft/blockstates")).unwrap();
    std::fs::write(
        unpacked.join("assets/minecraft/blockstates/stone.json"),
        b"{}",
    )
    .unwrap();
    std::fs::write(unpacked.join("pack.mcmeta"), b"{}").unwrap();
    let broken = dir.path().join("broken.jar");
    std::fs::write(&broken, b"not a zip").unwrap();
    let cache = LocalCache::new(&dir.path().join("cache"));
    assert_eq!(
        (cache.scan(&unpacked).blocks, cache.scan(&unpacked).assets),
        (1, 1)
    );
    assert!(cache.scan(&broken).unreadable);
    assert!(cache.scan(&dir.path().join("missing.zip")).unreadable);
}
