//! Overlay bakes kept under `Layout::model_cache()`, so the mods and packs a
//! studio draws with are baked once (plan schematic-studio, T17, T21).
//!
//! A cache entry is [`BakedPack::to_bytes`]: the overlays baked over one
//! bundle pack. Its key is that pack's file and version, then each mod jar
//! and resource pack by name, size and modification time: an instance with two
//! hundred mods would otherwise read every jar whole on each preview just to
//! find the entry. Unpacked resource packs are keyed the same way, file by
//! file. Beside each entry a small `.sources` file names the mods and packs it
//! was made from; an entry whose source was removed or changed is deleted on
//! the next bake. Vanilla assets are never baked here: they are the bundle's
//! (D23).

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lumilio_block_assets::{AssetSource, BUNDLE_FORMAT, BakedPack, bake_over};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::{Scan, SchematicAssetsError};

/// Bumped when the baker's output changes while its container format does
/// not (a new kind of kept texture, say), so stale bakes are not reused.
/// 2: packs keep their parent templates and per-source counts (T17).
const BAKE_REVISION: u32 = 2;

/// How many overlay bakes are kept: one is made per combination of sources
/// switched on (T21), so the least recently used beyond these are deleted.
pub(super) const KEPT_ENTRIES: usize = 8;

/// One source of a cached bake and how its key is taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Keyed {
    /// A mod jar or a resource pack: keyed by name, size and time.
    Stamp(PathBuf),
}

impl Keyed {
    fn path(&self) -> &Path {
        match self {
            Self::Stamp(path) => path,
        }
    }
}

/// A bake, and whether it was read from the cache.
#[derive(Clone, Debug, PartialEq)]
pub struct Baked {
    pub pack: BakedPack,
    pub from_cache: bool,
}

/// What an entry was made from, written beside it.
#[derive(Serialize, Deserialize)]
struct Made {
    /// Each mod or resource pack and its stamp then, in hex.
    stamped: Vec<(PathBuf, String)>,
}

#[derive(Clone, Debug)]
pub struct LocalCache {
    dir: PathBuf,
}

impl LocalCache {
    #[must_use]
    pub fn new(model_cache: &Path) -> Self {
        Self {
            dir: model_cache.join("local"),
        }
    }

    /// Where the bake of `sources` over the base named `base` is kept.
    pub fn entry_over(
        &self,
        base: &str,
        sources: &[Keyed],
    ) -> Result<PathBuf, SchematicAssetsError> {
        let mut key = Sha1::new();
        key.update(b"over:");
        key.update(base.as_bytes());
        key.update([0]);
        for source in sources {
            key.update(identity(source)?);
            key.update([0]);
        }
        Ok(self.dir.join(format!(
            "{BUNDLE_FORMAT}-{BAKE_REVISION}-{}.dat.z",
            hex(&key.finalize())
        )))
    }

    /// The overlays `sources` baked over `base`, which `base_key` names (the
    /// bundle file and version it was read from); only the overlay layer is
    /// kept. Blocking.
    pub fn bake_over(
        &self,
        base_key: &str,
        base: &BakedPack,
        sources: &[Keyed],
    ) -> Result<Baked, SchematicAssetsError> {
        let entry = self.entry_over(base_key, sources)?;
        self.cached_or(&entry, sources, |read| Ok(bake_over(base, read)?))
    }

    fn cached_or(
        &self,
        entry: &Path,
        sources: &[Keyed],
        make: impl FnOnce(&[AssetSource]) -> Result<BakedPack, SchematicAssetsError>,
    ) -> Result<Baked, SchematicAssetsError> {
        self.prune();
        if let Some(pack) = fs::read(entry)
            .ok()
            .and_then(|bytes| BakedPack::from_bytes(&bytes).ok())
        {
            // Used now: the last to go when the cache is trimmed.
            let _ = fs::File::options()
                .write(true)
                .open(entry)
                .and_then(|file| file.set_modified(std::time::SystemTime::now()));
            return Ok(Baked {
                pack,
                from_cache: true,
            });
        }
        let read: Vec<AssetSource> = sources
            .iter()
            .map(|source| AssetSource::at(source.path()))
            .collect();
        let pack = make(&read)?;
        // A cache that cannot be written only costs the next bake.
        if self.store(entry, sources, &pack).is_ok() {
            self.trim();
        }
        Ok(Baked {
            pack,
            from_cache: false,
        })
    }

    fn store(
        &self,
        entry: &Path,
        sources: &[Keyed],
        pack: &BakedPack,
    ) -> Result<(), SchematicAssetsError> {
        fs::create_dir_all(&self.dir)?;
        let mut stamped = Vec::new();
        for source in sources {
            let Keyed::Stamp(path) = source;
            stamped.push((path.clone(), hex(&identity(source)?)));
        }
        let made = serde_json::to_vec(&Made { stamped })
            .map_err(|error| SchematicAssetsError::Index(error.to_string()))?;
        let partial = entry.with_extension("partial");
        fs::write(&partial, pack.to_bytes()?)?;
        fs::rename(&partial, entry)?;
        fs::write(entry.with_extension("sources"), made)?;
        Ok(())
    }

    /// What the mod or resource pack `source` holds, read without baking it
    /// and kept beside the entries (T21). Blocking.
    #[must_use]
    pub fn scan(&self, source: &Path) -> Scan {
        super::scan::scan(&self.dir, source)
    }

    /// Deletes the entries made from a mod or resource pack that is gone or
    /// changed since: they can never be asked for again.
    pub fn prune(&self) {
        self.drop_entries(|made| {
            made.stamped.iter().any(|(path, then)| {
                identity(&Keyed::Stamp(path.clone())).is_ok_and(|now| hex(&now) != *then)
                    || !path.exists()
            })
        });
    }

    /// Deletes the entries made with `source`, which was taken off the
    /// overlays though its file is still there.
    pub fn forget(&self, source: &Path) {
        self.drop_entries(|made| made.stamped.iter().any(|(path, _)| path == source));
    }

    /// Deletes the least recently used entries beyond [`KEPT_ENTRIES`].
    fn trim(&self) {
        let Ok(listing) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut entries: Vec<(std::time::SystemTime, PathBuf)> = listing
            .filter_map(Result::ok)
            .map(|item| item.path())
            .filter(|path| path.to_string_lossy().ends_with(".dat.z"))
            .filter_map(|path| {
                let used = fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
                Some((used, path))
            })
            .collect();
        entries.sort_by_key(|(used, _)| std::cmp::Reverse(*used));
        for (_, entry) in entries.into_iter().skip(KEPT_ENTRIES) {
            let _ = fs::remove_file(entry.with_extension("sources"));
            let _ = fs::remove_file(&entry);
        }
    }

    fn drop_entries(&self, stale: impl Fn(&Made) -> bool) {
        let Ok(listing) = fs::read_dir(&self.dir) else {
            return;
        };
        for item in listing.filter_map(Result::ok) {
            let sidecar = item.path();
            if sidecar
                .extension()
                .is_none_or(|extension| extension != "sources")
            {
                continue;
            }
            let Some(made) = fs::read(&sidecar)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Made>(&bytes).ok())
            else {
                continue;
            };
            if stale(&made) {
                // `<key>.dat.sources` sits beside `<key>.dat.z`.
                let _ = fs::remove_file(sidecar.with_extension("z"));
                let _ = fs::remove_file(&sidecar);
            }
        }
    }
}

/// What a picture is drawn with beyond the bundle (T21): the sources
/// switched on for it, in order, each by its stamp, as a short hex suffix
/// (`-` and 16 digits); empty when none are on. A thumbnail kept under one
/// look is not shown for another. Blocking.
pub(crate) fn look(switched_on: &[PathBuf]) -> String {
    if switched_on.is_empty() {
        return String::new();
    }
    let mut key = Sha1::new();
    for path in switched_on {
        match identity(&Keyed::Stamp(path.clone())) {
            Ok(stamp) => key.update(stamp),
            // A source that is gone is drawn without; its name still keeps
            // this look apart from the bundle alone.
            Err(_) => key.update(path.to_string_lossy().as_bytes()),
        }
    }
    format!("-{}", hex(&key.finalize()[..8]))
}

/// The part of an entry's key one source gives.
pub(super) fn identity(source: &Keyed) -> Result<[u8; 20], SchematicAssetsError> {
    let Keyed::Stamp(path) = source;
    let mut key = Sha1::new();
    stamp(path, &mut key)?;
    Ok(key.finalize().into())
}

/// Name, size and modification time of a file, or of every file under a
/// directory in path order.
fn stamp(path: &Path, key: &mut Sha1) -> Result<(), SchematicAssetsError> {
    let mut files = Vec::new();
    if path.is_dir() {
        let mut stack = vec![path.to_owned()];
        while let Some(dir) = stack.pop() {
            for item in fs::read_dir(&dir)? {
                let item = item?;
                if item.file_type()?.is_dir() {
                    stack.push(item.path());
                } else {
                    files.push(item.path());
                }
            }
        }
        files.sort();
    } else {
        files.push(path.to_owned());
    }
    for file in files {
        let meta = fs::metadata(&file)?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |time| time.as_nanos());
        key.update(file.to_string_lossy().as_bytes());
        key.update(meta.len().to_le_bytes());
        key.update(modified.to_le_bytes());
    }
    Ok(())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
