//! The resource pack the 3D preview draws with, built from the player's own
//! `client.jar` (ADR 0027). Mojang's assets are never stored by the launcher:
//! the pack exists only while a preview window is being opened.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, Cursor};
use std::path::Path;
use std::time::UNIX_EPOCH;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

/// The folders of `assets/minecraft` a renderer needs to draw blocks.
const KEPT: [&str; 5] = ["blockstates", "models", "textures", "atlases", "items"];
/// A jar with more entries than this is not a Minecraft client.
const MAX_ENTRIES: usize = 200_000;
/// Total bytes the pack may hold (a 1.21 pack is about 7 MB).
const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePack {
    /// Names the pack for the viewer: the game version and the jar's identity,
    /// so a pack stored for another version is never taken for this one.
    pub id: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum ModelAssetsError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    /// The jar is not a Minecraft client, or is bigger than any real one.
    NotAClient,
}

impl Display for ModelAssetsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Zip(error) => write!(f, "{error}"),
            Self::NotAClient => f.write_str("the game file is not a Minecraft client"),
        }
    }
}

impl Error for ModelAssetsError {}

impl From<io::Error> for ModelAssetsError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for ModelAssetsError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error)
    }
}

fn kept(name: &str) -> bool {
    if name == "pack.mcmeta" {
        return true;
    }
    let Some(rest) = name.strip_prefix("assets/minecraft/") else {
        return false;
    };
    !name.contains("..") && KEPT.contains(&rest.split('/').next().unwrap_or_default())
}

/// A name the viewer can take in a URL: letters, digits and `.-_`.
fn plain(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Copies the block assets of `jar` into a new stored zip. Blocking: run it on
/// a worker.
pub fn build_pack(jar: &Path, game_version: &str) -> Result<ResourcePack, ModelAssetsError> {
    let meta = std::fs::metadata(jar)?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs());
    let id = format!("jar-{}-{:x}-{modified:x}", plain(game_version), meta.len());

    let mut archive = ZipArchive::new(File::open(jar)?)?;
    if archive.len() > MAX_ENTRIES {
        return Err(ModelAssetsError::NotAClient);
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let (mut total, mut found) = (0u64, false);
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        if !entry.is_file() || !kept(entry.name()) {
            continue;
        }
        total = total.saturating_add(entry.size());
        if total > MAX_BYTES {
            return Err(ModelAssetsError::NotAClient);
        }
        found |= entry.name().starts_with("assets/minecraft/textures/");
        writer.start_file(entry.name().to_owned(), options)?;
        io::copy(&mut entry, &mut writer)?;
    }
    if !found {
        return Err(ModelAssetsError::NotAClient);
    }
    Ok(ResourcePack {
        id,
        bytes: writer.finish()?.into_inner(),
    })
}

#[cfg(test)]
mod tests;
