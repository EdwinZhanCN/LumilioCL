use super::prune_disk;
use std::time::{Duration, SystemTime};

fn file(dir: &std::path::Path, name: &str, age: Duration) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, b"x").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(SystemTime::now() - age)
        .unwrap();
    path
}

#[test]
fn the_disk_keeps_the_newest_textures_and_drops_abandoned_partial_files() {
    let dir = tempfile::tempdir().unwrap();
    // 130 textures, the first the oldest.
    let textures: Vec<_> = (0..130)
        .map(|ix| {
            file(
                dir.path(),
                &format!("{ix:03}.png"),
                Duration::from_secs(1000 - ix),
            )
        })
        .collect();
    let abandoned = file(dir.path(), "old.123.tmp", Duration::from_secs(2 * 60 * 60));
    let writing = file(dir.path(), "new.456.tmp", Duration::from_secs(5));
    prune_disk(dir.path());
    assert!(!textures[0].exists() && !textures[1].exists());
    assert!(textures[2..].iter().all(|path| path.exists()));
    assert!(
        !abandoned.exists(),
        "a temporary file left over an hour goes"
    );
    assert!(writing.exists(), "a write in progress is left alone");
}
