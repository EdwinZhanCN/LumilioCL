use lumilio_block_assets::BakedPack;

use super::super::LocalCache;
use super::super::cache::Keyed;
use super::{baked, client_jar, zip_at};

/// A resource pack that retextures stone with `texture`.
fn pack_at(path: &std::path::Path, texture: &str) -> std::path::PathBuf {
    let model = format!(r#"{{"textures":{{"all":"minecraft:block/{texture}"}}}}"#);
    zip_at(
        path,
        &[("assets/minecraft/models/block/stone.json", model.as_bytes())],
    )
}

#[test]
fn a_second_preview_reads_the_cache_instead_of_baking_again() {
    let dir = tempfile::tempdir().unwrap();
    let base = baked(&client_jar(&dir.path().join("26.3.jar"), "26.3", "stone"));
    let pack = pack_at(&dir.path().join("dirt.zip"), "dirt");
    let cache = LocalCache::new(&dir.path().join("cache"));
    let sources = [Keyed::Stamp(pack)];

    let first = cache.bake_over("bundle:26.3", &base, &sources).unwrap();
    assert!(!first.from_cache);
    assert!(
        first
            .pack
            .assets
            .models
            .contains_key("minecraft:block/stone")
    );

    // Replace the entry with a different pack: a bake would not return it.
    let entry = cache.entry_over("bundle:26.3", &sources).unwrap();
    let mut planted = first.pack.clone();
    planted.assets.models.clear();
    std::fs::write(&entry, planted.to_bytes().unwrap()).unwrap();
    let second = cache.bake_over("bundle:26.3", &base, &sources).unwrap();
    assert!(second.from_cache);
    assert_eq!(second.pack, planted);
}

#[test]
fn the_cache_is_keyed_by_the_base_and_each_overlay() {
    let dir = tempfile::tempdir().unwrap();
    let dirt = pack_at(&dir.path().join("dirt.zip"), "dirt");
    let granite = pack_at(&dir.path().join("granite.zip"), "granite");
    let cache = LocalCache::new(&dir.path().join("cache"));
    let alone = cache
        .entry_over("bundle:26.3", &[Keyed::Stamp(dirt.clone())])
        .unwrap();
    let over_another_base = cache
        .entry_over("bundle:1.20.1", &[Keyed::Stamp(dirt.clone())])
        .unwrap();
    let layered = cache
        .entry_over(
            "bundle:26.3",
            &[Keyed::Stamp(dirt.clone()), Keyed::Stamp(granite)],
        )
        .unwrap();
    assert_ne!(alone, over_another_base);
    assert_ne!(alone, layered);

    // The same name with other contents is another entry.
    pack_at(&dirt, "a much longer texture name");
    assert_ne!(
        cache
            .entry_over("bundle:26.3", &[Keyed::Stamp(dirt)])
            .unwrap(),
        alone
    );
}

#[test]
fn a_damaged_entry_is_baked_again() {
    let dir = tempfile::tempdir().unwrap();
    let base = baked(&client_jar(&dir.path().join("client.jar"), "26.3", "stone"));
    let pack = pack_at(&dir.path().join("dirt.zip"), "dirt");
    let cache = LocalCache::new(&dir.path().join("cache"));
    let sources = [Keyed::Stamp(pack)];
    let entry = cache.entry_over("bundle:26.3", &sources).unwrap();
    std::fs::create_dir_all(entry.parent().unwrap()).unwrap();
    std::fs::write(&entry, b"damaged").unwrap();
    let baked = cache.bake_over("bundle:26.3", &base, &sources).unwrap();
    assert!(!baked.from_cache);
    assert!(BakedPack::from_bytes(&std::fs::read(&entry).unwrap()).is_ok());
}

/// T21: one bake is kept per combination of sources switched on, and only
/// the most recently used of them: an entry read again stays while older
/// ones beyond the limit go, sidecar and all.
#[test]
fn only_the_most_recently_used_bakes_are_kept() {
    use super::super::cache::KEPT_ENTRIES;
    let dir = tempfile::tempdir().unwrap();
    let base = baked(&client_jar(&dir.path().join("26.3.jar"), "26.3", "stone"));
    let cache = LocalCache::new(&dir.path().join("cache"));
    let sources: Vec<[Keyed; 1]> = (0..=KEPT_ENTRIES)
        .map(|n| {
            [Keyed::Stamp(pack_at(
                &dir.path().join(format!("{n}.zip")),
                "dirt",
            ))]
        })
        .collect();
    let entry = |n: usize| cache.entry_over("bundle:26.3", &sources[n]).unwrap();
    let past =
        |n: u64| std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000 + n);
    for (n, sources) in sources.iter().enumerate().take(KEPT_ENTRIES) {
        cache.bake_over("bundle:26.3", &base, sources).unwrap();
        // Made in order, long ago: the times decide, not the clock's grain.
        let file = std::fs::File::options().write(true).open(entry(n)).unwrap();
        file.set_modified(past(n as u64)).unwrap();
    }
    assert!(
        cache
            .bake_over("bundle:26.3", &base, &sources[0])
            .unwrap()
            .from_cache
    );
    cache
        .bake_over("bundle:26.3", &base, &sources[KEPT_ENTRIES])
        .unwrap();
    assert!(entry(0).exists(), "used again: kept");
    assert!(!entry(1).exists(), "the least recently used goes");
    assert!(
        !entry(1).with_extension("sources").exists(),
        "with its sidecar"
    );
    for n in 2..=KEPT_ENTRIES {
        assert!(entry(n).exists(), "entry {n} stays");
    }
}
