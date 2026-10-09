//! A chunk's sections and their block states, from either NBT layout.
//!
//! Before 1.18 the chunk keeps everything under a `Level` compound with
//! `Sections`, each section having `Palette` and `BlockStates`. From 1.18 the
//! sections are at the top and hold `block_states.palette` and
//! `block_states.data`. 26.x changed how palette entries are written; see
//! [`palette_name`]. Packed indices are 4 bits or more, and from 1.16
//! (data version 2529) an index never spans two longs; before that it could.
//! Heightmaps use the same packing, 256 entries of 9 bits or so; biomes are
//! in `biome.rs`.
use crate::Error;
use crate::biome::{self, SectionBiomes};
use lumilio_nbt::Tag;

/// The first data version after block names replaced numeric ids (1.13).
const FLATTENED: i32 = 1451;
/// 20w17a, where packed arrays stopped letting an entry span two longs.
const PADDED: i32 = 2529;
const AIR: &str = "minecraft:air";

/// A chunk saved by a version that named blocks by number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsupportedVersion(pub i32);

/// One 16×16×16 cube of a chunk.
#[derive(Clone, Debug)]
pub struct Section {
    /// The section's height in cubes; block `y` is `16 * y + local`.
    pub y: i32,
    /// Block names, `minecraft:stone` and so on, without properties.
    pub palette: Vec<String>,
    /// One palette index per block, `y * 256 + z * 16 + x`; `None` when the
    /// whole section is `palette[0]`.
    indices: Option<Vec<u16>>,
    /// The section's biomes (1.18 and later).
    biomes: Option<SectionBiomes>,
}

impl Section {
    /// The block at local `x`, `y`, `z` (each 0..16).
    pub fn block(&self, x: usize, y: usize, z: usize) -> &str {
        let index = match &self.indices {
            Some(indices) => indices.get(y * 256 + z * 16 + x).copied().unwrap_or(0),
            None => 0,
        };
        self.palette
            .get(usize::from(index))
            .map_or(AIR, String::as_str)
    }

    /// Whether the section is entirely air.
    pub fn is_empty(&self) -> bool {
        self.indices.is_none() && self.palette.first().is_none_or(|name| is_air(name))
    }
}

/// The name in one palette entry. Up to 1.21 it is a compound `{Name,
/// Properties}`. 26.x writes `{id, properties}`, and a palette whose entries
/// have no properties at all as a list of plain strings; in a mixed list a
/// plain entry is wrapped as `{"": name}` (what the game does with a list whose
/// elements are not all the same NBT type).
pub(crate) fn palette_name(entry: &Tag) -> Option<&str> {
    entry.as_str().or_else(|| {
        ["Name", "id", ""]
            .into_iter()
            .find_map(|key| entry.get(key).and_then(Tag::as_str))
    })
}

pub(crate) fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// The heightmaps a map can use. `WorldSurface` is the highest block that is
/// not air; `MotionBlocking` the highest that blocks movement or holds a fluid
/// (so it skips flowers and torches but not water or leaves).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Heightmap {
    WorldSurface,
    MotionBlocking,
}

impl Heightmap {
    fn key(self) -> &'static str {
        match self {
            Self::WorldSurface => "WORLD_SURFACE",
            Self::MotionBlocking => "MOTION_BLOCKING",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub data_version: i32,
    /// The lowest block height the chunk can hold: 0 before 1.18, the
    /// chunk's `yPos` sections down from 1.18 (-64 in a default overworld).
    pub min_y: i32,
    /// Sections from the top of the world down.
    pub sections: Vec<Section>,
    status: Option<String>,
    heightmaps: Vec<(Heightmap, Vec<i64>)>,
    /// Numeric biome ids before 1.18.
    legacy_biomes: Option<Vec<i32>>,
}

impl Chunk {
    /// Reads a chunk's root compound.
    pub fn from_nbt(root: &Tag) -> Result<Self, Error> {
        let data_version = root
            .get("DataVersion")
            .and_then(Tag::as_i64)
            .and_then(|version| i32::try_from(version).ok())
            .unwrap_or(0);
        let level = root.get("Level");
        let (holder, modern) = match level {
            Some(level) => (level, false),
            None => (root, true),
        };
        let status = holder
            .get("Status")
            .and_then(Tag::as_str)
            .map(str::to_owned);
        let list = holder
            .get(if modern { "sections" } else { "Sections" })
            .and_then(|tag| match tag {
                Tag::List(items) => Some(items.as_slice()),
                _ => None,
            })
            .unwrap_or(&[]);
        let mut sections = Vec::new();
        let mut lowest = i32::MAX;
        for item in list {
            let y = item.get("Y").and_then(Tag::as_i64).unwrap_or(0) as i32;
            lowest = lowest.min(y);
            let (palette, data) = if modern {
                (
                    item.at(&["block_states", "palette"]),
                    item.at(&["block_states", "data"]),
                )
            } else {
                (item.get("Palette"), item.get("BlockStates"))
            };
            let Some(Tag::List(palette)) = palette else {
                // Numeric block ids: the layout before the flattening.
                if item.get("Blocks").is_some() || (data_version != 0 && data_version < FLATTENED) {
                    return Err(Error::Unsupported(UnsupportedVersion(data_version)));
                }
                continue;
            };
            let biomes = match item.get("biomes") {
                Some(biomes) if modern => SectionBiomes::from_nbt(biomes)?,
                _ => None,
            };
            let names: Vec<String> = palette
                .iter()
                .map(|entry| palette_name(entry).unwrap_or(AIR).to_owned())
                .collect();
            if names.is_empty() {
                continue;
            }
            let indices = match data {
                Some(Tag::LongArray(longs)) if names.len() > 1 => {
                    Some(unpack(longs, names.len(), 4096, 4, data_version < PADDED)?)
                }
                _ => None,
            };
            sections.push(Section {
                y,
                palette: names,
                indices,
                biomes,
            });
        }
        sections.sort_by_key(|section| std::cmp::Reverse(section.y));
        let min_y = if modern {
            holder
                .get("yPos")
                .and_then(Tag::as_i64)
                .map(|y| y as i32)
                .unwrap_or(if lowest == i32::MAX { 0 } else { lowest })
                .saturating_mul(16)
        } else {
            0
        };
        let heightmaps = [Heightmap::WorldSurface, Heightmap::MotionBlocking]
            .into_iter()
            .filter_map(|kind| match holder.at(&["Heightmaps", kind.key()]) {
                Some(Tag::LongArray(longs)) => Some((kind, longs.clone())),
                _ => None,
            })
            .collect();
        let legacy_biomes = match holder.get("Biomes") {
            Some(Tag::IntArray(ids)) if !modern => Some(ids.clone()),
            Some(Tag::ByteArray(ids)) if !modern => {
                Some(ids.iter().map(|id| i32::from(*id as u8)).collect())
            }
            _ => None,
        };
        Ok(Self {
            data_version,
            min_y,
            sections,
            status,
            heightmaps,
            legacy_biomes,
        })
    }

    /// The height of the top block of every column, `z * 16 + x`, by the
    /// game's own heightmap; `None` for an empty column. `None` overall when
    /// the chunk has no such heightmap or it does not decode.
    pub fn heightmap(&self, kind: Heightmap) -> Option<Vec<Option<i32>>> {
        let (_, longs) = self.heightmaps.iter().find(|(have, _)| *have == kind)?;
        let spanning = self.data_version < PADDED;
        // The entry width is not stored: 9 bits for worlds up to 512 blocks
        // tall, more for taller ones. Find the width the array length fits.
        let bits = (1..=16_usize).find(|bits| {
            let need = if spanning {
                (256 * bits).div_ceil(64)
            } else {
                256_usize.div_ceil(64 / bits)
            };
            need == longs.len()
        })?;
        let values = unpack_bits(longs, bits, 256, spanning).ok()?;
        Some(
            values
                .into_iter()
                .map(|value| (value > 0).then(|| self.min_y + value as i32 - 1))
                .collect(),
        )
    }

    /// The biome at block `x`, `z` of the chunk (each 0..16) and height `y`.
    pub fn biome(&self, x: usize, y: i32, z: usize) -> Option<&str> {
        if x >= 16 || z >= 16 {
            return None;
        }
        if let Some(ids) = &self.legacy_biomes {
            return biome::legacy(ids, x, y, z);
        }
        let section_y = y.div_euclid(16);
        let section = self
            .sections
            .iter()
            .find(|section| section.y == section_y)?;
        section
            .biomes
            .as_ref()
            .map(|biomes| biomes.at(x, y.rem_euclid(16) as usize, z))
    }

    /// Whether world generation got far enough to place this chunk's surface
    /// (features are placed before lighting starts). A chunk caught partway
    /// (only noise, or only structure starts) has no surface worth drawing.
    pub fn is_complete(&self) -> bool {
        match self.status.as_deref() {
            None => true,
            Some(status) => matches!(
                status.strip_prefix("minecraft:").unwrap_or(status),
                "full"
                    | "fullchunk"
                    | "postprocessed"
                    | "finalized"
                    | "mobs_spawned"
                    | "spawn"
                    | "light"
                    | "initialize_light"
            ),
        }
    }
}

/// Unpacks `count` palette indices from a packed long array, `min_bits` or
/// more bits each. `spanning` is the layout before 1.16, in which an entry may
/// straddle two longs.
pub(crate) fn unpack(
    longs: &[i64],
    palette_len: usize,
    count: usize,
    min_bits: usize,
    spanning: bool,
) -> Result<Vec<u16>, Error> {
    let bits = (usize::BITS - (palette_len - 1).leading_zeros()).max(min_bits as u32) as usize;
    Ok(unpack_bits(longs, bits, count, spanning)?
        .into_iter()
        .map(|value| (value as usize).min(palette_len - 1) as u16)
        .collect())
}

/// `count` entries of `bits` bits each (1..=16) from a packed long array.
fn unpack_bits(
    longs: &[i64],
    bits: usize,
    count: usize,
    spanning: bool,
) -> Result<Vec<u64>, Error> {
    let mask = (1u64 << bits) - 1;
    let mut out = Vec::with_capacity(count);
    if spanning {
        if longs.len() * 64 < count * bits {
            return Err(Error::Format("block states are too short"));
        }
        for index in 0..count {
            let start = index * bits;
            let (word, shift) = (start / 64, start % 64);
            let mut value = (longs[word] as u64) >> shift;
            if shift + bits > 64 {
                value |= (longs[word + 1] as u64) << (64 - shift);
            }
            out.push(value & mask);
        }
    } else {
        let per_long = 64 / bits;
        if longs.len() * per_long < count {
            return Err(Error::Format("block states are too short"));
        }
        for index in 0..count {
            let value = (longs[index / per_long] as u64) >> ((index % per_long) * bits);
            out.push(value & mask);
        }
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) fn pack(
    indices: &[u16],
    palette_len: usize,
    min_bits: usize,
    spanning: bool,
) -> Vec<i64> {
    let bits = (usize::BITS - (palette_len - 1).leading_zeros()).max(min_bits as u32) as usize;
    if spanning {
        let mut longs = vec![0u64; (indices.len() * bits).div_ceil(64)];
        for (index, value) in indices.iter().enumerate() {
            let start = index * bits;
            let (word, shift) = (start / 64, start % 64);
            longs[word] |= u64::from(*value) << shift;
            if shift + bits > 64 {
                longs[word + 1] |= u64::from(*value) >> (64 - shift);
            }
        }
        longs.into_iter().map(|long| long as i64).collect()
    } else {
        let per_long = 64 / bits;
        let mut longs = vec![0u64; indices.len().div_ceil(per_long)];
        for (index, value) in indices.iter().enumerate() {
            longs[index / per_long] |= u64::from(*value) << ((index % per_long) * bits);
        }
        longs.into_iter().map(|long| long as i64).collect()
    }
}
