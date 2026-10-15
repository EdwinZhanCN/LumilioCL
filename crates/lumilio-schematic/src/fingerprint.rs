//! Content fingerprints for duplicate detection (schematic-studio T16, D9).
//!
//! The hash covers each region's normalized content: the palette sorted by
//! block-state string and the cell indices renumbered to match, block entities
//! and entities sorted by position. File bytes, compression, metadata, region
//! names and the data-version stamp are ignored, so a copy saved under another
//! name or with another description has the same fingerprint, and one changed
//! block gives a different one.

use crate::{BlockState, Region, UniversalSchematic};
use nucleation::nbt::{NbtMap, NbtValue};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

/// A SHA-256 over normalized content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint(pub [u8; 32]);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Fingerprints a schematic's content.
pub fn fingerprint(schematic: &UniversalSchematic) -> Fingerprint {
    // Region order in a file says nothing about content: sort the per-region
    // hashes before combining them.
    let mut regions: Vec<[u8; 32]> = crate::regions_in_precedence(schematic)
        .into_iter()
        .map(region_hash)
        .collect();
    regions.sort_unstable();
    let mut hasher = Sha256::new();
    hasher.update(b"lumilio-schematic/fingerprint/1");
    hasher.update((regions.len() as u64).to_le_bytes());
    for region in regions {
        hasher.update(region);
    }
    Fingerprint(hasher.finalize().into())
}

fn region_hash(region: &Region) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let bbox = region.get_bounding_box();
    for value in [
        bbox.min.0, bbox.min.1, bbox.min.2, bbox.max.0, bbox.max.1, bbox.max.2,
    ] {
        hasher.update(value.to_le_bytes());
    }

    // Palette: only used states, sorted by canonical string, indices renumbered.
    let palette = region.get_palette();
    let names: Vec<String> = palette.iter().map(canonical_state).collect();
    let mut used: Vec<&str> = region
        .blocks
        .iter()
        .filter_map(|&index| names.get(index).map(String::as_str))
        .collect();
    used.sort_unstable();
    used.dedup();
    let renumber: HashMap<&str, u32> = used
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index as u32))
        .collect();
    hasher.update((used.len() as u64).to_le_bytes());
    for name in &used {
        write_str(&mut hasher, name);
    }
    hasher.update((region.blocks.len() as u64).to_le_bytes());
    for &index in &region.blocks {
        let renumbered = names
            .get(index)
            .and_then(|name| renumber.get(name.as_str()))
            .copied()
            .unwrap_or(u32::MAX);
        hasher.update(renumbered.to_le_bytes());
    }

    let mut block_entities: Vec<_> = region.block_entities.iter().collect();
    block_entities.sort_by_key(|(position, _)| *position);
    hasher.update((block_entities.len() as u64).to_le_bytes());
    for (position, block_entity) in block_entities {
        for value in [position.0, position.1, position.2] {
            hasher.update(value.to_le_bytes());
        }
        write_str(&mut hasher, &block_entity.id);
        write_map(
            &mut hasher,
            block_entity.nbt.inner(),
            TOP_LEVEL_POSITION_AND_ID,
        );
    }

    let mut entities: Vec<_> = region.entities.iter().collect();
    entities.sort_by(|a, b| {
        let key = |e: &nucleation::Entity| {
            (
                e.position.0.to_bits(),
                e.position.1.to_bits(),
                e.position.2.to_bits(),
            )
        };
        key(a).cmp(&key(b)).then_with(|| a.id.cmp(&b.id))
    });
    hasher.update((entities.len() as u64).to_le_bytes());
    for entity in entities {
        for value in [entity.position.0, entity.position.1, entity.position.2] {
            hasher.update(value.to_bits().to_le_bytes());
        }
        write_str(&mut hasher, &entity.id);
        write_entity_map(&mut hasher, &entity.nbt, TOP_LEVEL_POSITION_AND_ID);
    }

    hasher.finalize().into()
}

/// `id[a=1,b=2]` with properties sorted by name.
fn canonical_state(state: &BlockState) -> String {
    let mut properties: Vec<_> = state.properties.iter().collect();
    properties.sort();
    let mut text = state.name.to_string();
    if !properties.is_empty() {
        let joined: Vec<String> = properties.iter().map(|(k, v)| format!("{k}={v}")).collect();
        text.push('[');
        text.push_str(&joined.join(","));
        text.push(']');
    }
    text
}

fn write_str(hasher: &mut Sha256, text: &str) {
    hasher.update((text.len() as u64).to_le_bytes());
    hasher.update(text.as_bytes());
}

/// Keys the litematic reader copies into an object's NBT next to the typed id
/// and position, which are hashed on their own: an object built in memory
/// lacks them, the same object read from a file has them.
const TOP_LEVEL_POSITION_AND_ID: &[&str] = &["Id", "id", "Pos", "x", "y", "z"];

fn write_map(hasher: &mut Sha256, map: &HashMap<String, NbtValue>, skip: &[&str]) {
    let mut keys: Vec<&String> = map
        .keys()
        .filter(|key| !skip.contains(&key.as_str()))
        .collect();
    keys.sort();
    hasher.update((keys.len() as u64).to_le_bytes());
    for key in keys {
        write_str(hasher, key);
        write_value(hasher, &map[key]);
    }
}

fn write_value(hasher: &mut Sha256, value: &NbtValue) {
    match value {
        NbtValue::Byte(v) => tagged(hasher, 1, &v.to_le_bytes()),
        NbtValue::Short(v) => tagged(hasher, 2, &v.to_le_bytes()),
        NbtValue::Int(v) => tagged(hasher, 3, &v.to_le_bytes()),
        NbtValue::Long(v) => tagged(hasher, 4, &v.to_le_bytes()),
        NbtValue::Float(v) => tagged(hasher, 5, &v.to_bits().to_le_bytes()),
        NbtValue::Double(v) => tagged(hasher, 6, &v.to_bits().to_le_bytes()),
        NbtValue::ByteArray(v) => {
            tagged(hasher, 7, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|b| hasher.update(b.to_le_bytes()));
        }
        NbtValue::String(v) => {
            hasher.update([8]);
            write_str(hasher, v);
        }
        NbtValue::List(items) => {
            tagged(hasher, 9, &(items.len() as u64).to_le_bytes());
            items.iter().for_each(|item| write_value(hasher, item));
        }
        NbtValue::Compound(map) => {
            hasher.update([10]);
            write_nbt_map(hasher, map);
        }
        NbtValue::IntArray(v) => {
            tagged(hasher, 11, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|i| hasher.update(i.to_le_bytes()));
        }
        NbtValue::LongArray(v) => {
            tagged(hasher, 12, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|i| hasher.update(i.to_le_bytes()));
        }
    }
}

fn write_nbt_map(hasher: &mut Sha256, map: &NbtMap) {
    write_map(hasher, map.inner(), &[]);
}

fn write_entity_map(
    hasher: &mut Sha256,
    map: &HashMap<String, nucleation::NbtValue>,
    skip: &[&str],
) {
    let mut keys: Vec<&String> = map
        .keys()
        .filter(|key| !skip.contains(&key.as_str()))
        .collect();
    keys.sort();
    hasher.update((keys.len() as u64).to_le_bytes());
    for key in keys {
        write_str(hasher, key);
        write_entity_value(hasher, &map[key]);
    }
}

fn write_entity_value(hasher: &mut Sha256, value: &nucleation::NbtValue) {
    use nucleation::NbtValue as V;
    match value {
        V::Byte(v) => tagged(hasher, 1, &v.to_le_bytes()),
        V::Boolean(v) => tagged(hasher, 1, &[u8::from(*v)]),
        V::Short(v) => tagged(hasher, 2, &v.to_le_bytes()),
        V::Int(v) => tagged(hasher, 3, &v.to_le_bytes()),
        V::Long(v) => tagged(hasher, 4, &v.to_le_bytes()),
        V::Float(v) => tagged(hasher, 5, &v.to_bits().to_le_bytes()),
        V::Double(v) => tagged(hasher, 6, &v.to_bits().to_le_bytes()),
        V::ByteArray(v) => {
            tagged(hasher, 7, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|b| hasher.update(b.to_le_bytes()));
        }
        V::String(v) => {
            hasher.update([8]);
            write_str(hasher, v);
        }
        V::List(items) => {
            tagged(hasher, 9, &(items.len() as u64).to_le_bytes());
            items
                .iter()
                .for_each(|item| write_entity_value(hasher, item));
        }
        V::Compound(map) => {
            hasher.update([10]);
            write_entity_map(hasher, map, &[]);
        }
        V::IntArray(v) => {
            tagged(hasher, 11, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|i| hasher.update(i.to_le_bytes()));
        }
        V::LongArray(v) => {
            tagged(hasher, 12, &(v.len() as u64).to_le_bytes());
            v.iter().for_each(|i| hasher.update(i.to_le_bytes()));
        }
    }
}

fn tagged(hasher: &mut Sha256, tag: u8, payload: &[u8]) {
    hasher.update([tag]);
    hasher.update(payload);
}
