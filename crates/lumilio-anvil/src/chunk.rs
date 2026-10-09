//! A chunk's sections and their block states, from either NBT layout.
//!
//! Before 1.18 the chunk keeps everything under a `Level` compound with
//! `Sections`, each section having `Palette` and `BlockStates`. From 1.18 the
//! sections are at the top and hold `block_states.palette` and
//! `block_states.data`. Packed indices are 4 bits or more, and from 1.16
//! (data version 2529) an index never spans two longs; before that it could.
use crate::Error;
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

pub(crate) fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

#[derive(Clone, Debug)]
pub struct Chunk {
    pub data_version: i32,
    /// Sections from the top of the world down.
    pub sections: Vec<Section>,
    status: Option<String>,
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
        for item in list {
            let y = item.get("Y").and_then(Tag::as_i64).unwrap_or(0) as i32;
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
            let names: Vec<String> = palette
                .iter()
                .map(|entry| {
                    entry
                        .get("Name")
                        .and_then(Tag::as_str)
                        .unwrap_or(AIR)
                        .to_owned()
                })
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
            });
        }
        sections.sort_by_key(|section| std::cmp::Reverse(section.y));
        Ok(Self {
            data_version,
            sections,
            status,
        })
    }

    /// Whether world generation finished this chunk. A chunk caught partway
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
            ),
        }
    }
}

/// Unpacks `count` palette indices from a packed long array, `min_bits` or
/// more bits each. `spanning` is the layout before 1.16, in which an entry may
/// straddle two longs.
fn unpack(
    longs: &[i64],
    palette_len: usize,
    count: usize,
    min_bits: usize,
    spanning: bool,
) -> Result<Vec<u16>, Error> {
    let bits = (usize::BITS - (palette_len - 1).leading_zeros()).max(min_bits as u32) as usize;
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
            out.push(((value & mask) as usize).min(palette_len - 1) as u16);
        }
    } else {
        let per_long = 64 / bits;
        if longs.len() * per_long < count {
            return Err(Error::Format("block states are too short"));
        }
        for index in 0..count {
            let value = (longs[index / per_long] as u64) >> ((index % per_long) * bits);
            out.push(((value & mask) as usize).min(palette_len - 1) as u16);
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
