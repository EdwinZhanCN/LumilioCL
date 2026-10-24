use super::*;

fn put(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![7u8; bytes]).unwrap();
}

#[test]
fn every_folder_is_counted_once_and_natives_are_cache_not_shared() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path());
    put(&layout.game("a").join("saves/w/level.dat"), 100);
    put(&layout.game("b").join("options.txt"), 20);
    put(&layout.libraries().join("x.jar"), 1000);
    put(&layout.assets().join("objects/ab"), 50);
    put(&layout.natives("1.0").join("lib.dylib"), 300);
    put(&layout.runtimes().join("jdk/bin/java"), 4000);
    put(&dir.path().join("cache/icons/a.png"), 7);
    let usage = measure(&layout);
    assert_eq!(
        usage,
        StorageUsage {
            games: 120,
            shared: 1050,
            runtimes: 4000,
            cache: 307,
        }
    );
    assert_eq!(usage.total(), 5477);
}

#[test]
fn clearing_removes_only_what_can_be_made_again() {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path());
    put(&layout.game("a").join("saves/w/level.dat"), 100);
    put(&layout.libraries().join("x.jar"), 1000);
    put(&layout.runtimes().join("jdk/bin/java"), 4000);
    put(&layout.natives("1.0").join("lib.dylib"), 300);
    put(&dir.path().join("cache/icons/a.png"), 7);
    assert_eq!(clear_cache(&layout).unwrap(), 307);
    let after = measure(&layout);
    assert_eq!(after.cache, 0);
    assert_eq!(
        (after.games, after.shared, after.runtimes),
        (100, 1000, 4000)
    );
    // Nothing left to clear is not an error.
    assert_eq!(clear_cache(&layout).unwrap(), 0);
}

#[test]
fn the_recommendation_is_half_the_memory_within_bounds() {
    assert_eq!(recommended_memory_mb(16_384), 8192);
    assert_eq!(recommended_memory_mb(65_536), 8192);
    assert_eq!(recommended_memory_mb(8192), 4096);
    assert_eq!(recommended_memory_mb(6000), 2560);
    assert_eq!(recommended_memory_mb(4096), 2048);
    assert_eq!(
        recommended_memory_mb(1024),
        1024,
        "everything on a tiny machine"
    );
}

#[cfg(unix)]
#[test]
fn a_link_is_counted_as_itself_and_not_followed() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big");
    put(&big.join("f"), 10_000);
    let folder = dir.path().join("folder");
    fs::create_dir_all(&folder).unwrap();
    std::os::unix::fs::symlink(&big, folder.join("link")).unwrap();
    assert!(directory_size(&folder) < 10_000);
    assert_eq!(directory_size(&dir.path().join("missing")), 0);
}
