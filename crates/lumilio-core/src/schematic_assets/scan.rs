//! What a mod or resource pack holds, read without baking it (plan
//! schematic-studio, T21): the studio lists every source with its counts
//! whether it is switched on or not. A model that names a custom `loader`
//! (Forge's and NeoForge's OBJ, composite and the like) is drawn by mod
//! code, not by its JSON, so the baker cannot draw it: those are counted.
//! A scan is kept beside the overlay bakes, one file per source, with the
//! source's name, size and time it was read at: a changed source is read
//! again and its file replaced.

use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use sha1::{Digest, Sha1};

use super::SchematicAssetsError;
use super::cache::{Keyed, hex, identity};

/// The counts of one source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scan {
    /// Blockstate files: the blocks it adds or redraws.
    pub blocks: u32,
    /// Blockstate, model and texture files.
    pub assets: u32,
    /// Models a custom loader draws, which the baker cannot.
    pub custom_models: u32,
    /// Not a zip or folder of resources at all.
    pub unreadable: bool,
}

/// A kept scan and the stamp of the source it was read from.
#[derive(Serialize, Deserialize)]
struct Kept {
    stamp: String,
    scan: Scan,
}

/// The scan of `source`, from `dir` when kept there for the source as it is
/// now. Blocking.
pub(super) fn scan(dir: &Path, source: &Path) -> Scan {
    let Ok(stamp) = identity(&Keyed::Stamp(source.to_owned())).map(|key| hex(&key)) else {
        // Gone or unreadable: nothing to keep.
        return Scan {
            unreadable: true,
            ..Scan::default()
        };
    };
    let name = hex(&Sha1::digest(source.to_string_lossy().as_bytes()));
    let kept = dir.join(format!("scan-{name}.json"));
    if let Some(found) = fs::read(&kept)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Kept>(&bytes).ok())
        .filter(|found| found.stamp == stamp)
    {
        return found.scan;
    }
    let scan = read(source).unwrap_or(Scan {
        unreadable: true,
        ..Scan::default()
    });
    // A scan that cannot be kept is only read again next time.
    if let Ok(bytes) = serde_json::to_vec(&Kept { stamp, scan }) {
        let _ = fs::create_dir_all(dir).and_then(|()| fs::write(&kept, bytes));
    }
    scan
}

/// What kind of resource file `name` (a path below the source) is.
fn kind(name: &str) -> Option<&'static str> {
    let mut parts = name.split('/');
    if parts.next()? != "assets" {
        return None;
    }
    let (_, folder) = (parts.next()?, parts.next()?);
    let last = name.rsplit('/').next()?;
    match folder {
        "blockstates" if last.ends_with(".json") => Some("blockstate"),
        "models" if last.ends_with(".json") => Some("model"),
        "textures" if last.ends_with(".png") || last.ends_with(".png.mcmeta") => Some("texture"),
        _ => None,
    }
}

/// Whether model JSON `bytes` names a custom loader at its top level.
fn custom(bytes: &[u8]) -> bool {
    bytes.windows(8).any(|window| window == b"\"loader\"")
        && serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|json| json.get("loader").cloned())
            .is_some()
}

fn read(source: &Path) -> Result<Scan, SchematicAssetsError> {
    let mut scan = Scan::default();
    let mut count = |name: &str, bytes: &mut dyn FnMut() -> Option<Vec<u8>>| match kind(name) {
        Some("blockstate") => {
            scan.blocks += 1;
            scan.assets += 1;
        }
        Some("model") => {
            scan.assets += 1;
            if bytes().is_some_and(|bytes| custom(&bytes)) {
                scan.custom_models += 1;
            }
        }
        Some(_) => scan.assets += 1,
        None => {}
    };
    if source.is_dir() {
        let mut stack = vec![source.to_owned()];
        while let Some(dir) = stack.pop() {
            for item in fs::read_dir(&dir)?.filter_map(Result::ok) {
                let path = item.path();
                if item.file_type()?.is_dir() {
                    stack.push(path);
                    continue;
                }
                let Ok(relative) = path.strip_prefix(source) else {
                    continue;
                };
                let name = relative.to_string_lossy().replace('\\', "/");
                count(&name, &mut || fs::read(&path).ok());
            }
        }
        return Ok(scan);
    }
    let file = fs::File::open(source)?;
    let mut archive = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    for index in 0..archive.len() {
        let Ok(mut entry) = archive.by_index(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        count(&name, &mut || {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).ok().map(|_| bytes)
        });
    }
    Ok(scan)
}
