//! Tiles of base maps drawn from a world's files (plan W6), kept per instance
//! in `profiles/<id>/map-cache/` beside the length and modification time of
//! every file each was drawn from. A tile is reused only while all of its
//! files still look the same, so once the game has written a region, that
//! region's tiles and the coarser tiles above them are drawn again and every
//! other tile is not.
use super::cache::{read_entry, write_entry};
use super::context::io_error;
use crate::plugins::SourceStamp;
use lumilio_plugin_api::map::{TileKey, TileReply};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;
use std::path::PathBuf;

/// A coarse tile names many files, so an entry may be larger than a seed
/// tile's.
const MAX_ENTRY: u64 = 8 * 1024 * 1024;
/// What every entry's name is derived from besides its key: a launcher update
/// may draw tiles differently (new colour tables), so it starts afresh.
const FORMAT: &str = concat!("sourced-1-", env!("CARGO_PKG_VERSION"));

#[derive(Deserialize, Serialize)]
struct Entry {
    sources: Vec<SourceStamp>,
    reply: TileReply,
}

#[derive(Clone, Debug)]
pub struct SourcedCache {
    root: PathBuf,
    limit: u64,
}

impl SourcedCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            limit: 512 * 1024 * 1024,
        }
    }

    fn path(&self, key: &TileKey) -> io::Result<PathBuf> {
        let bytes = serde_json::to_vec(&(key, FORMAT)).map_err(io_error)?;
        let digest: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(self.root.join(format!("{digest}.tile")))
    }

    /// The tile kept for `key`, if its files are exactly `sources` now.
    pub fn get(&self, key: &TileKey, sources: &[SourceStamp]) -> io::Result<Option<TileReply>> {
        let Some(bytes) = read_entry(&self.path(key)?, MAX_ENTRY)? else {
            return Ok(None);
        };
        Ok(serde_json::from_slice::<Entry>(&bytes)
            .ok()
            .filter(|entry| entry.sources == sources && entry.reply.is_valid())
            .map(|entry| entry.reply))
    }

    pub fn put(
        &self,
        key: &TileKey,
        sources: Vec<SourceStamp>,
        reply: TileReply,
    ) -> io::Result<()> {
        if !reply.is_valid() {
            return Err(io_error("invalid cache tile"));
        }
        let bytes = serde_json::to_vec(&Entry { sources, reply }).map_err(io_error)?;
        write_entry(&self.root, self.limit, &self.path(key)?, &bytes, MAX_ENTRY)
    }
}
