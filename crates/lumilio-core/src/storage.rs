//! What the launcher's own folder holds, and the parts that can be cleared.

use std::fs;
use std::io;
use std::path::Path;

use crate::layout::Layout;

/// Sizes in bytes of the launcher's folders.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageUsage {
    /// Every instance's own folder: mods, saves, options.
    pub games: u64,
    /// Game files shared between instances: libraries, versions, assets.
    pub shared: u64,
    /// Java runtimes the launcher owns.
    pub runtimes: u64,
    /// What can be thrown away and made again: extracted natives and `cache/`.
    pub cache: u64,
}

impl StorageUsage {
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.games + self.shared + self.runtimes + self.cache
    }
}

/// The folder for anything that can be fetched or made again.
fn cache_dir(layout: &Layout) -> std::path::PathBuf {
    layout.root().join("cache")
}

fn natives_dir(layout: &Layout) -> std::path::PathBuf {
    layout.meta().join("natives")
}

/// Total size of the files under `path`. Symbolic links count as themselves,
/// never as what they point to, so a link cannot make a folder look huge or
/// lead the walk out of it. A missing folder is 0.
#[must_use]
pub fn directory_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        return metadata.len();
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| directory_size(&entry.path()))
        .sum()
}

/// Measures the folders. Walks the disk: call off the interface thread.
#[must_use]
pub fn measure(layout: &Layout) -> StorageUsage {
    let natives = directory_size(&natives_dir(layout));
    StorageUsage {
        games: directory_size(&layout.profiles()),
        shared: directory_size(&layout.meta()).saturating_sub(natives),
        runtimes: directory_size(&layout.runtimes()),
        cache: directory_size(&cache_dir(layout)) + natives,
    }
}

/// Deletes what can be made again and returns how many bytes that freed.
/// Games, shared game files and Java runtimes are never touched; extracted
/// natives are rebuilt at the next start of a game that needs them.
pub fn clear_cache(layout: &Layout) -> io::Result<u64> {
    let mut freed = 0;
    for folder in [cache_dir(layout), natives_dir(layout)] {
        freed += directory_size(&folder);
        match fs::remove_dir_all(&folder) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(freed)
}

/// The machine's installed memory in MB, if the system will say. Best effort:
/// `None` means "unknown", never "none".
#[must_use]
pub fn total_memory_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024)
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let bytes: u64 = String::from_utf8(output.stdout).ok()?.trim().parse().ok()?;
        Some(bytes / (1024 * 1024))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// A sensible largest heap for the game on a machine with `total_mb`: half
/// the memory, at most 8 GB, at least 2 GB (or everything on a small
/// machine), in steps of 512 MB.
#[must_use]
pub fn recommended_memory_mb(total_mb: u64) -> u64 {
    let half = (total_mb / 2).min(8192);
    let stepped = half / 512 * 512;
    stepped.max(2048.min(total_mb))
}

#[cfg(test)]
mod tests {
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
}
