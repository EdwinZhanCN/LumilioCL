use std::slice;
use std::time::{Duration, SystemTime};

use super::*;

fn cache() -> (tempfile::TempDir, RecognitionCache) {
    let dir = tempfile::tempdir().unwrap();
    let cache = RecognitionCache::new(dir.path().join("cache/content.sqlite"));
    (dir, cache)
}

fn set_changed(path: &Path, at: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(at)
        .unwrap();
}

#[test]
fn a_hash_is_read_again_only_when_the_file_changes() {
    let (dir, cache) = cache();
    let path = dir.path().join("a.jar");
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
    std::fs::write(&path, b"aaaa").unwrap();
    set_changed(&path, at);
    let first = crate::content::sha1_hex(&path).unwrap();
    assert_eq!(
        cache.sha1s(slice::from_ref(&path), 10),
        vec![Some(first.clone())]
    );

    // Same size and change time: the kept hash answers, the file is not read.
    std::fs::write(&path, b"bbbb").unwrap();
    set_changed(&path, at);
    assert_eq!(
        cache.sha1s(slice::from_ref(&path), 20),
        vec![Some(first.clone())]
    );

    set_changed(&path, at + Duration::from_secs(1));
    let second = crate::content::sha1_hex(&path).unwrap();
    assert_ne!(first, second);
    assert_eq!(cache.sha1s(slice::from_ref(&path), 30), vec![Some(second)]);
    assert_eq!(cache.sha1s(&[dir.path().join("gone.jar")], 40), vec![None]);
}

#[test]
fn a_fingerprint_is_computed_once_per_content() {
    let (dir, cache) = cache();
    let path = dir.path().join("a.jar");
    std::fs::write(&path, b"curse  jar\n").unwrap();
    let murmur2 = crate::content::fingerprint::of_file(&path).unwrap();
    let files = [("s1".to_owned(), path.clone())];
    assert_eq!(cache.fingerprints(&files, 10), vec![Some(murmur2)]);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(cache.fingerprints(&files, 20), vec![Some(murmur2)]);
    assert_eq!(
        cache.fingerprints(&[("s2".to_owned(), path)], 30),
        vec![None]
    );
}

#[test]
fn an_answer_goes_stale_after_its_time_and_is_forgotten_a_week_later() {
    let (_dir, cache) = cache();
    let key = identify_key("source", "s1");
    cache.keep(&[(key.clone(), Some(7_u32), 100)], 1_000);

    let fresh = cache.recall::<Option<u32>>(slice::from_ref(&key), 1_099);
    assert!(fresh[&key].fresh);
    assert_eq!(fresh[&key].value, Some(7));
    let stale = cache.recall::<Option<u32>>(slice::from_ref(&key), 1_100);
    assert!(!stale[&key].fresh);
    assert_eq!(stale[&key].value, Some(7));

    // Any later write forgets what expired more than a week before it.
    let other = project_key("source", "P");
    cache.keep(&[(other.clone(), 1_u32, 100)], 1_100 + EXPIRED_KEPT);
    assert!(
        cache
            .recall::<Option<u32>>(slice::from_ref(&key), 1_100 + EXPIRED_KEPT)
            .contains_key(&key)
    );
    cache.keep(&[(other, 1_u32, 100)], 1_101 + EXPIRED_KEPT);
    assert!(cache.recall::<Option<u32>>(&[key], 0).is_empty());
}

#[test]
fn an_answer_that_no_longer_reads_is_a_miss() {
    let (_dir, cache) = cache();
    let key = project_key("source", "P");
    cache.keep(&[(key.clone(), "not a number", 100)], 0);
    assert!(cache.recall::<u32>(slice::from_ref(&key), 0).is_empty());
    assert_eq!(cache.recall::<String>(&[key], 0).len(), 1);
}

#[test]
fn a_cache_that_cannot_open_still_hashes_and_remembers_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("cache"), b"a file, not a folder").unwrap();
    let cache = RecognitionCache::new(dir.path().join("cache/content.sqlite"));
    let path = dir.path().join("a.jar");
    std::fs::write(&path, b"aaaa").unwrap();
    assert_eq!(
        cache.sha1s(slice::from_ref(&path), 0),
        vec![Some(crate::content::sha1_hex(&path).unwrap())]
    );
    cache.keep(&[(project_key("source", "P"), 1_u32, 100)], 0);
    assert!(
        cache
            .recall::<u32>(&[project_key("source", "P")], 0)
            .is_empty()
    );
}
