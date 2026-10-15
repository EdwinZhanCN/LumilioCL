//! The instance's mods and resource packs as studio sources (plan
//! schematic-studio, T17, T21): their priority over the bundle, their cache
//! entries, what each provides switched on or off, and the packs added from
//! elsewhere.

use std::path::{Path, PathBuf};

use lumilio_block_assets::{BakedPack, write_bundle};
use sha2::{Digest, Sha256};

use super::super::cache::Keyed;
use super::super::{
    AssetPlan, BundleFile, BundleStore, InstanceAssets, LocalCache, PlannedSource, ProvidedSource,
    SourceRole, add_pack, load, provided, read_added, remove_pack,
};
use super::{baked, zip_at};

/// A client whose stone inherits `cube_all`, as vanilla's does.
fn vanilla(path: &Path) -> PathBuf {
    zip_at(
        path,
        &[
            ("version.json", br#"{"id":"1.20.1","world_version":3465}"#),
            (
                "assets/minecraft/blockstates/stone.json",
                br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
            ),
            (
                "assets/minecraft/models/block/cube_all.json",
                br##"{"textures":{"particle":"#all"},"elements":[{"from":[0,0,0],"to":[16,16,16],
                    "faces":{"north":{"texture":"#all","cullface":"north"}}}]}"##,
            ),
            (
                "assets/minecraft/models/block/stone.json",
                br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/stone"}}"#,
            ),
        ],
    )
}

/// A mod that adds an ore on a vanilla parent and retextures stone.
fn mod_jar(path: &Path) -> PathBuf {
    zip_at(
        path,
        &[
            ("fabric.mod.json", br#"{"id":"gems"}"#),
            (
                "assets/gems/blockstates/ruby_ore.json",
                br#"{"variants":{"":{"model":"gems:block/ruby_ore"}}}"#,
            ),
            (
                "assets/gems/models/block/ruby_ore.json",
                br#"{"parent":"minecraft:block/cube_all","textures":{"all":"gems:block/ruby_ore"}}"#,
            ),
            (
                "assets/minecraft/models/block/stone.json",
                br#"{"parent":"minecraft:block/cube_all","textures":{"all":"gems:block/mossy"}}"#,
            ),
        ],
    )
}

/// A resource pack that retextures stone once more.
fn resource_pack(path: &Path) -> PathBuf {
    zip_at(
        path,
        &[(
            "assets/minecraft/models/block/stone.json",
            br#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/granite"}}"#,
        )],
    )
}

/// `vanilla` published as a bundle and already downloaded into `cache`.
fn downloaded_bundle(dir: &Path, cache: &Path) -> (BundleStore, BundleFile) {
    let jar = vanilla(&dir.join("1.20.1.jar"));
    let bytes = write_bundle([("1.20.1", &baked(&jar))]).unwrap();
    let sha256 = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = BundleFile {
        name: "blocks.dat.z".into(),
        size: bytes.len() as u64,
        sha256,
    };
    let store = BundleStore::new(cache);
    std::fs::create_dir_all(store.path_of(&file).parent().unwrap()).unwrap();
    std::fs::write(store.path_of(&file), bytes).unwrap();
    (store, file)
}

fn bundle_1_20() -> PlannedSource {
    PlannedSource::Bundle {
        version: "1.20.1".into(),
    }
}

fn plan(base: PlannedSource, overlays: &[&PathBuf]) -> AssetPlan {
    let mut sources = vec![base];
    sources.extend(
        overlays
            .iter()
            .map(|path| PlannedSource::Overlay((*path).clone())),
    );
    AssetPlan {
        sources,
        ..AssetPlan::default()
    }
}

fn stone_texture(pack: &BakedPack) -> &str {
    &pack.assets.models["minecraft:block/stone"].textures["all"]
}

/// Resource pack over mod over the bundle's base.
#[test]
fn a_resource_pack_wins_over_a_mod_which_wins_over_the_base() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let cache = LocalCache::new(&cache_dir);
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let granite = resource_pack(&dir.path().join("granite.zip"));
    let (store, file) = downloaded_bundle(dir.path(), &cache_dir);
    let bundle = Some((&store, &file));
    let alone = load(&plan(bundle_1_20(), &[]), &cache, bundle).unwrap();
    assert_eq!(stone_texture(&alone.pack), "minecraft:block/stone");
    let modded = load(&plan(bundle_1_20(), &[&gems]), &cache, bundle).unwrap();
    assert_eq!(stone_texture(&modded.pack), "gems:block/mossy");
    let both = load(&plan(bundle_1_20(), &[&gems, &granite]), &cache, bundle).unwrap();
    assert_eq!(stone_texture(&both.pack), "minecraft:block/granite");
}

/// Without the bundle there is nothing to draw with: no base of any other
/// kind is baked (D23).
#[test]
fn a_plan_without_the_bundle_loads_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let cache = LocalCache::new(&dir.path().join("cache"));
    let gems = mod_jar(&dir.path().join("gems.jar"));
    assert!(load(&plan(bundle_1_20(), &[&gems]), &cache, None).is_err());
    assert!(load(&AssetPlan::default(), &cache, None).is_err());
}

/// D13's follow-up: over a bundle base, a mod block whose model inherits a
/// vanilla parent has geometry, as it has over the client jar.
#[test]
fn a_mod_block_on_a_vanilla_parent_draws_over_the_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let (store, file) = downloaded_bundle(dir.path(), &cache_dir);
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let baked = load(
        &plan(bundle_1_20(), &[&gems]),
        &LocalCache::new(&cache_dir),
        Some((&store, &file)),
    )
    .unwrap();
    let ore = &baked.pack.assets.models["gems:block/ruby_ore"];
    assert_eq!(ore.elements.len(), 1, "the parent's cube");
    assert_eq!(ore.textures["all"], "gems:block/ruby_ore");
}

fn entries(cache: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(cache.join("local"))
        .map(|listing| {
            listing
                .filter_map(Result::ok)
                .map(|item| item.path())
                .collect()
        })
        .unwrap_or_default();
    found.retain(|path| path.extension().is_some_and(|extension| extension == "z"));
    found.sort();
    found
}

/// An entry made with a mod or pack goes when the source is deleted, when
/// it changes, or when an added pack is taken off the list.
#[test]
fn removing_a_source_drops_its_cache_entry() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let cache = LocalCache::new(&cache_dir);
    let base = baked(&vanilla(&dir.path().join("client.jar")));
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let granite = resource_pack(&dir.path().join("granite.zip"));
    let with_both = [Keyed::Stamp(gems.clone()), Keyed::Stamp(granite.clone())];
    let with_mod = [Keyed::Stamp(gems.clone())];
    let bake = |sources: &[Keyed]| cache.bake_over("bundle:1.20.1", &base, sources).unwrap();
    let entry = |sources: &[Keyed]| cache.entry_over("bundle:1.20.1", sources).unwrap();

    bake(&with_both);
    assert_eq!(entries(&cache_dir), [entry(&with_both)]);
    std::fs::remove_file(&granite).unwrap();
    assert!(!bake(&with_mod).from_cache);
    assert_eq!(
        entries(&cache_dir),
        [entry(&with_mod)],
        "the entry made with the deleted pack is gone"
    );

    // Taken off the list while the file stays: `forget` drops it.
    cache.forget(&gems);
    assert!(entries(&cache_dir).is_empty());
    assert!(!bake(&with_mod).from_cache);
    assert!(bake(&with_mod).from_cache);
}

/// What `instance` offers: `overlays`, the packs `added`, and those of them
/// `on`.
fn offering(overlays: &[&PathBuf], added: &[&PathBuf], on: &[&PathBuf]) -> InstanceAssets {
    let owned = |paths: &[&PathBuf]| paths.iter().map(|path| (*path).clone()).collect();
    InstanceAssets {
        game_version: "1.20.1".into(),
        overlays: owned(overlays),
        added: owned(added),
        switched_on: owned(on),
        ..InstanceAssets::default()
    }
}

/// A library mod without assets and a `.jar` that is no zip bake on without
/// an error, and the overview leaves them out.
#[test]
fn a_jar_that_is_no_resource_source_is_skipped_quietly() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let (store, file) = downloaded_bundle(dir.path(), &cache_dir);
    let library = zip_at(
        &dir.path().join("library.jar"),
        &[("com/example/Lib.class", b"code")],
    );
    let broken = dir.path().join("broken.jar");
    std::fs::write(&broken, b"not a zip").unwrap();
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let all = [&broken, &gems, &library];
    let plan = plan(bundle_1_20(), &all);
    let cache = LocalCache::new(&cache_dir);
    let baked = load(&plan, &cache, Some((&store, &file))).unwrap();
    assert!(baked.pack.assets.blockstates.contains_key("gems:ruby_ore"));
    assert_eq!(
        provided(&plan, &baked.pack, &offering(&all, &[], &all), |path| {
            cache.scan(path)
        }),
        [
            ProvidedSource {
                role: SourceRole::Base {
                    version: "1.20.1".into(),
                },
                blocks: 1,
                assets: 3,
                custom_models: 0,
                enabled: true,
            },
            ProvidedSource {
                role: SourceRole::Mod(gems),
                blocks: 1,
                assets: 3,
                custom_models: 0,
                enabled: true,
            },
        ]
    );
}

/// The list: the base, each mod and pack on offer in order with its counts
/// and whether it is switched on, and an added pack whose file is gone, so it
/// can be taken off.
#[test]
fn the_list_shows_each_source_on_or_off_and_what_it_provides() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let (store, file) = downloaded_bundle(dir.path(), &cache_dir);
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let granite = resource_pack(&dir.path().join("granite.zip"));
    let gone = dir.path().join("gone.zip");
    let offered = offering(&[&gems, &granite], &[&granite, &gone], &[&gems]);
    let plan = plan(bundle_1_20(), &[&gems]);
    let cache = LocalCache::new(&cache_dir);
    let baked = load(&plan, &cache, Some((&store, &file))).unwrap();
    let roles: Vec<(SourceRole, u32, u32, bool)> =
        provided(&plan, &baked.pack, &offered, |path| cache.scan(path))
            .into_iter()
            .map(|source| (source.role, source.blocks, source.assets, source.enabled))
            .collect();
    assert_eq!(
        roles,
        [
            (
                SourceRole::Base {
                    version: "1.20.1".into(),
                },
                1,
                3,
                true
            ),
            (SourceRole::Mod(gems), 1, 3, true),
            (
                SourceRole::Added {
                    path: granite,
                    missing: false
                },
                0,
                1,
                false
            ),
            (
                SourceRole::Added {
                    path: gone,
                    missing: true
                },
                0,
                0,
                false
            ),
        ]
    );
}

/// T21: a mod switched off is listed with what it would provide, but the
/// preview is drawn without it.
#[test]
fn a_source_switched_off_is_listed_but_not_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let cache_dir = dir.path().join("cache");
    let (store, file) = downloaded_bundle(dir.path(), &cache_dir);
    let gems = mod_jar(&dir.path().join("gems.jar"));
    let granite = resource_pack(&dir.path().join("granite.zip"));
    let offered = offering(&[&gems, &granite], &[], &[&granite]);
    let plan = super::super::resolve(
        &offered,
        Some(3465),
        super::super::BundleReach::Index(
            &super::super::BundleIndex::parse(
                super::rules::index_json(&file.sha256, file.size).as_bytes(),
            )
            .unwrap(),
        ),
    );
    assert_eq!(plan.overlays(), [granite.as_path()]);
    let cache = LocalCache::new(&cache_dir);
    let baked = load(&plan, &cache, Some((&store, &file))).unwrap();
    assert!(!baked.pack.assets.blockstates.contains_key("gems:ruby_ore"));
    assert_eq!(stone_texture(&baked.pack), "minecraft:block/granite");
    let listed = provided(&plan, &baked.pack, &offered, |path| cache.scan(path));
    let gems_row = listed
        .iter()
        .find(|source| source.role == SourceRole::Mod(gems.clone()))
        .expect("an overlay switched off is still listed");
    assert_eq!(
        (gems_row.blocks, gems_row.assets, gems_row.enabled),
        (1, 3, false)
    );
}

/// Adding keeps only the path, once; a file that is no zip is refused.
#[test]
fn an_added_pack_is_a_path_in_launcher_data() {
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("profile/schematic-packs.json");
    let pack = resource_pack(&dir.path().join("Faithful.zip"));
    let notes = dir.path().join("notes.txt");
    std::fs::write(&notes, b"hi").unwrap();

    assert!(read_added(&list).is_empty());
    assert!(add_pack(&list, &notes).is_err());
    assert!(add_pack(&list, Path::new("relative.zip")).is_err());
    add_pack(&list, &pack).unwrap();
    assert_eq!(add_pack(&list, &pack).unwrap(), std::slice::from_ref(&pack));
    assert_eq!(read_added(&list), std::slice::from_ref(&pack));
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        3,
        "nothing was copied beside the pack"
    );
    assert!(remove_pack(&list, &pack).unwrap().is_empty());
    assert!(pack.exists(), "taking it off leaves the file alone");
}
