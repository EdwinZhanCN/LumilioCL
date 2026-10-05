//! Reads `.litematic` files, written from the published file layout: a gzip
//! NBT document with a `Metadata` compound and per-region block palettes whose
//! indices are packed into a long array.

use std::collections::BTreeMap;

use lumilio_nbt::Tag;

/// Largest volume (blocks) of one region whose materials are counted.
const MAX_VOLUME: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Metadata {
    pub name: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub size: Option<(i64, i64, i64)>,
    pub total_blocks: Option<i64>,
    pub region_count: Option<i64>,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: Option<i64>,
    /// Square ARGB preview as RGBA, if the file carries one.
    pub preview: Option<(u32, Vec<u8>)>,
}

pub struct Litematic {
    pub metadata: Metadata,
    root: Tag,
}

pub fn parse(bytes: &[u8]) -> Result<Litematic, String> {
    let root = lumilio_nbt::parse_maybe_gzip(bytes).map_err(|error| format!("{error:?}"))?;
    let meta = root.get("Metadata").ok_or("missing Metadata")?;
    let text = |key: &str| {
        meta.get(key)
            .and_then(Tag::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let number = |path: &[&str]| meta.at(path).and_then(Tag::as_i64);
    let size = match (
        number(&["EnclosingSize", "x"]),
        number(&["EnclosingSize", "y"]),
        number(&["EnclosingSize", "z"]),
    ) {
        (Some(x), Some(y), Some(z)) => Some((x.abs(), y.abs(), z.abs())),
        _ => None,
    };
    let metadata = Metadata {
        name: text("Name"),
        author: text("Author"),
        description: text("Description"),
        size,
        total_blocks: number(&["TotalBlocks"]),
        region_count: number(&["RegionCount"]),
        modified_ms: number(&["TimeModified"]),
        preview: match meta.get("PreviewImageData") {
            Some(Tag::IntArray(pixels)) => preview(pixels),
            _ => None,
        },
    };
    Ok(Litematic { metadata, root })
}

fn preview(pixels: &[i32]) -> Option<(u32, Vec<u8>)> {
    let side =
        (0..=512u32).find(|side| u64::from(*side) * u64::from(*side) >= pixels.len() as u64)?;
    if side == 0 || u64::from(side) * u64::from(side) != pixels.len() as u64 {
        return None;
    }
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        let [a, r, g, b] = pixel.to_be_bytes();
        rgba.extend([r, g, b, a]);
    }
    Some((side, rgba))
}

impl Litematic {
    /// Block id → count across all regions, air excluded.
    pub fn materials(&self) -> Result<BTreeMap<String, u64>, String> {
        let mut counts = BTreeMap::new();
        let Some(Tag::Compound(regions)) = self.root.get("Regions") else {
            return Ok(counts);
        };
        for region in regions.values() {
            count_region(region, &mut counts)?;
        }
        Ok(counts)
    }
}

fn count_region(region: &Tag, counts: &mut BTreeMap<String, u64>) -> Result<(), String> {
    let dimension = |axis: &str| {
        region
            .at(&["Size", axis])
            .and_then(Tag::as_i64)
            .map(i64::unsigned_abs)
            .ok_or("missing region size")
    };
    let volume = dimension("x")?
        .checked_mul(dimension("y")?)
        .and_then(|v| v.checked_mul(dimension("z").ok()?))
        .ok_or("region is too large")?;
    if volume > MAX_VOLUME {
        return Err("region is too large".into());
    }
    let Some(Tag::List(palette)) = region.get("BlockStatePalette") else {
        return Err("missing palette".into());
    };
    let names: Vec<&str> = palette
        .iter()
        .map(|entry| entry.get("Name").and_then(Tag::as_str).ok_or("bad palette"))
        .collect::<Result<_, _>>()?;
    let Some(Tag::LongArray(words)) = region.get("BlockStates") else {
        return Err("missing block states".into());
    };
    let bits = bit_width(names.len());
    if (volume * u64::from(bits)).div_ceil(64) > words.len() as u64 {
        return Err("block states are too short".into());
    }
    let mut tally = vec![0u64; names.len()];
    for index in 0..volume {
        let value = unpack(words, index, bits);
        *tally
            .get_mut(usize::try_from(value).map_err(|_| "bad index")?)
            .ok_or("block index outside the palette")? += 1;
    }
    for (name, count) in names.into_iter().zip(tally) {
        if count > 0 && !is_air(name) {
            *counts.entry(name.to_owned()).or_default() += count;
        }
    }
    Ok(())
}

/// max(2, ceil(log2(palette length))).
fn bit_width(palette: usize) -> u32 {
    let needed = if palette <= 1 {
        0
    } else {
        usize::BITS - (palette - 1).leading_zeros()
    };
    needed.max(2)
}

/// The `index`-th packed value; a value may straddle two longs.
fn unpack(words: &[i64], index: u64, bits: u32) -> u64 {
    let start = index * u64::from(bits);
    let word = usize::try_from(start / 64).unwrap_or(usize::MAX);
    let offset = u32::try_from(start % 64).unwrap_or(0);
    let mut value = (words[word] as u64) >> offset;
    if offset + bits > 64 {
        value |= (words[word + 1] as u64) << (64 - offset);
    }
    value & ((1u64 << bits) - 1)
}

fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

#[cfg(test)]
pub(crate) fn pack(values: &[u64], bits: u32) -> Vec<i64> {
    let mut words = vec![0u64; (values.len() as u64 * u64::from(bits)).div_ceil(64) as usize];
    for (index, value) in values.iter().enumerate() {
        let start = index as u64 * u64::from(bits);
        let (word, offset) = ((start / 64) as usize, (start % 64) as u32);
        words[word] |= value << offset;
        if offset + bits > 64 {
            words[word + 1] |= value >> (64 - offset);
        }
    }
    words.into_iter().map(|word| word as i64).collect()
}
