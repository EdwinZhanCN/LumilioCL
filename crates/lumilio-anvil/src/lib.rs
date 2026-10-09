//! Reading Minecraft's Anvil world files: a region's chunk table, compressed
//! chunks, the block states and biomes inside them, their heightmaps, and the
//! top block of each column.
//!
//! Nothing here touches the file system. A region is read through a
//! [`Source`], which hands out byte ranges, so the caller decides where the
//! bytes come from (the launcher reads them through a host that allows only
//! ranges of one granted directory). A damaged chunk is an error for that
//! chunk alone.
//!
//! The region layout and the block-state unpacking are adapted from fastanvil
//! fastanvil/src/region.rs and fastanvil/src/java/section_data.rs (MIT OR Apache-2.0, Copyright (c) 2020 Owen Gage), ADR 0022.
//! Differences: reads go through byte ranges rather than a seekable stream, the
//! external `.mcc` chunks are supported, and NBT comes from `lumilio-nbt`, which
//! caps the size and depth of what it parses.

mod biome;
mod chunk;
mod lz4;
mod region;
mod surface;

pub use biome::legacy_name as legacy_biome_name;
pub use chunk::{Chunk, Heightmap, Section, UnsupportedVersion};
pub use region::{ChunkLocation, Region, Source, region_coords};
pub use surface::{Column, columns};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    /// The source could not give the bytes asked for.
    Source(String),
    /// The region header or a chunk header is inconsistent.
    Format(&'static str),
    /// A chunk's compression is not one the game writes.
    Compression(u8),
    /// Decompressing or parsing the chunk failed.
    Data(String),
    /// A chunk format older than the 1.13 flattening, which names blocks by
    /// number.
    Unsupported(UnsupportedVersion),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(message) | Self::Data(message) => f.write_str(message),
            Self::Format(message) => f.write_str(message),
            Self::Compression(kind) => write!(f, "unknown chunk compression {kind}"),
            Self::Unsupported(UnsupportedVersion(version)) => {
                write!(f, "chunk format {version} is not supported")
            }
        }
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests;
