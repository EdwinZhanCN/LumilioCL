//! The resource pack the 3D preview draws with, built from the player's own
//! `client.jar` (ADR 0027). Mojang's assets are never stored by the launcher:
//! the pack exists only while a preview is being loaded (ADR 0028).

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::File;
use std::io::{self, Cursor};
use std::path::Path;

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

/// Copies the block assets of `jar` into a new stored zip. Blocking: run it on
/// a worker.
pub fn build_pack(jar: &Path) -> Result<ResourcePack, ModelAssetsError> {
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
        bytes: writer.finish()?.into_inner(),
    })
}

#[cfg(test)]
mod tests;
