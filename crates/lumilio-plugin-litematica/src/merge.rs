//! Hands the 3D viewer a schematic with a single region.
//!
//! Litematica keeps one block palette per region, and the viewer mixes the
//! palettes of a schematic that has several up (a glass block in one region
//! showed as the soul sand that has the same index in another). Merging the
//! regions first, each block keeping its own state, avoids it.

use std::collections::BTreeMap;
use std::io::Write;

use flate2::Compression;
use flate2::write::GzEncoder;
use lumilio_nbt::Tag;

use crate::format::{bit_width, is_air, pack, unpack};

/// The most blocks of merged volume (a region index is a `u16`, two bytes each).
const MAX_VOLUME: u64 = 64 * 1024 * 1024;

struct Part<'a> {
    region: &'a Tag,
    /// The region's lowest corner in the schematic's own coordinates.
    min: [i64; 3],
    size: [u64; 3],
}

fn axis(tag: Option<&Tag>, key: &str) -> Result<i64, String> {
    tag.and_then(|tag| tag.get(key))
        .and_then(Tag::as_i64)
        .ok_or_else(|| format!("a region has no {key}"))
}

fn parts(regions: &BTreeMap<String, Tag>) -> Result<Vec<Part<'_>>, String> {
    regions
        .values()
        .map(|region| {
            let mut min = [0; 3];
            let mut size = [0; 3];
            for (index, key) in ["x", "y", "z"].into_iter().enumerate() {
                let extent = axis(region.get("Size"), key)?;
                let position = axis(region.get("Position"), key)?;
                // A negative size grows the region towards lower coordinates.
                min[index] = position + if extent < 0 { extent + 1 } else { 0 };
                size[index] = extent.unsigned_abs();
            }
            Ok(Part { region, min, size })
        })
        .collect()
}

/// The palette entry's identity: its name and sorted properties.
fn state_key(entry: &Tag) -> Option<String> {
    let name = entry.get("Name")?.as_str()?;
    let mut key = name.to_owned();
    if let Some(Tag::Compound(properties)) = entry.get("Properties") {
        for (property, value) in properties {
            key.push_str(&format!("|{property}={}", value.as_str()?));
        }
    }
    Some(key)
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn vector(x: i64, y: i64, z: i64) -> Tag {
    let int = |v: i64| Tag::Int(i32::try_from(v).unwrap_or_default());
    compound(vec![("x", int(x)), ("y", int(y)), ("z", int(z))])
}

/// The schematic as one region. A schematic that already has one is returned
/// as it is.
pub(crate) fn single_region(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let root = lumilio_nbt::parse_maybe_gzip(bytes).map_err(|error| format!("{error:?}"))?;
    let Some(Tag::Compound(regions)) = root.get("Regions") else {
        return Err("the file has no regions".into());
    };
    if regions.len() <= 1 {
        return Ok(bytes.to_vec());
    }
    let parts = parts(regions)?;
    let mut low = [i64::MAX; 3];
    let mut high = [i64::MIN; 3];
    for part in &parts {
        for index in 0..3 {
            low[index] = low[index].min(part.min[index]);
            high[index] = high[index].max(part.min[index] + part.size[index] as i64);
        }
    }
    let dim = [0, 1, 2].map(|index| (high[index] - low[index]) as u64);
    let volume = dim[0]
        .checked_mul(dim[1])
        .and_then(|v| v.checked_mul(dim[2]))
        .filter(|volume| *volume <= MAX_VOLUME)
        .ok_or("the schematic is too large")?;

    // Index 0 is air, whatever kind of air a region used.
    let mut palette = vec![compound(vec![(
        "Name",
        Tag::String("minecraft:air".into()),
    )])];
    let mut known: BTreeMap<String, u16> = BTreeMap::new();
    let mut merged = vec![0u16; volume as usize];
    let mut tile_entities = Vec::new();

    for part in &parts {
        let Some(Tag::List(entries)) = part.region.get("BlockStatePalette") else {
            return Err("a region has no palette".into());
        };
        let mut remap = Vec::with_capacity(entries.len());
        for entry in entries {
            let name = entry
                .get("Name")
                .and_then(Tag::as_str)
                .ok_or("bad palette")?;
            if is_air(name) {
                remap.push(0u16);
                continue;
            }
            let key = state_key(entry).ok_or("bad palette")?;
            let next = u16::try_from(palette.len()).map_err(|_| "too many block states")?;
            let index = *known.entry(key).or_insert_with(|| {
                palette.push(entry.clone());
                next
            });
            remap.push(index);
        }
        let Some(Tag::LongArray(words)) = part.region.get("BlockStates") else {
            return Err("a region has no block states".into());
        };
        let bits = bit_width(entries.len());
        let count = part.size.iter().product::<u64>();
        if (count * u64::from(bits)).div_ceil(64) > words.len() as u64 {
            return Err("block states are too short".into());
        }
        let offset = [0, 1, 2].map(|index| (part.min[index] - low[index]) as u64);
        for index in 0..count {
            let state = usize::try_from(unpack(words, index, bits)).map_err(|_| "bad index")?;
            let id = *remap.get(state).ok_or("block index outside the palette")?;
            if id == 0 {
                continue;
            }
            // A region stores blocks as (y * sizeZ + z) * sizeX + x.
            let x = index % part.size[0];
            let z = index / part.size[0] % part.size[2];
            let y = index / (part.size[0] * part.size[2]);
            let slot = ((y + offset[1]) * dim[2] + (z + offset[2])) * dim[0] + x + offset[0];
            merged[slot as usize] = id;
        }
        if let Some(Tag::List(items)) = part.region.get("TileEntities") {
            for item in items {
                let coordinate = |key| item.get(key).and_then(Tag::as_i64);
                let (Some(x), Some(y), Some(z), Tag::Compound(fields)) =
                    (coordinate("x"), coordinate("y"), coordinate("z"), item)
                else {
                    continue;
                };
                let mut moved = fields.clone();
                for (key, value, shift) in [("x", x, 0), ("y", y, 1), ("z", z, 2)] {
                    let shifted = value + (part.min[shift] - low[shift]);
                    moved.insert(
                        key.to_owned(),
                        Tag::Int(i32::try_from(shifted).unwrap_or_default()),
                    );
                }
                tile_entities.push(Tag::Compound(moved));
            }
        }
    }

    let bits = bit_width(palette.len());
    let region = compound(vec![
        ("Position", vector(0, 0, 0)),
        ("Size", vector(dim[0] as i64, dim[1] as i64, dim[2] as i64)),
        ("BlockStatePalette", Tag::List(palette)),
        ("BlockStates", Tag::LongArray(pack(&merged, bits))),
        ("TileEntities", Tag::List(tile_entities)),
        ("Entities", Tag::List(Vec::new())),
    ]);
    let Tag::Compound(mut fields) = root else {
        return Err("the file has no root".into());
    };
    if let Some(Tag::Compound(metadata)) = fields.get_mut("Metadata") {
        metadata.insert("RegionCount".into(), Tag::Int(1));
        metadata.insert(
            "EnclosingSize".into(),
            vector(dim[0] as i64, dim[1] as i64, dim[2] as i64),
        );
    }
    fields.insert("Regions".into(), compound(vec![("merged", region)]));
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    gzip.write_all(&lumilio_nbt::to_bytes(&Tag::Compound(fields)))
        .map_err(|error| error.to_string())?;
    gzip.finish().map_err(|error| error.to_string())
}
