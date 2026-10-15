//! The game's own names for blocks, items and entities, per language (plan
//! schematic-studio, T26/D28): what the studio writes instead of an ID.
//!
//! The names come from the game's language files (`en_us` in the client jar,
//! `zh_cn` in the asset index). Only plain `block.<namespace>.<name>`,
//! `item.…` and `entity.…` keys are kept. Several versions merge into one
//! table, newest first: a name a later version changed reads as the later
//! one, and an ID only older versions have keeps its old name.

use std::collections::BTreeMap;
use std::io::Read as _;

use serde::{Deserialize, Serialize};

use crate::ContainerError;

/// `names.dat.z`'s format, bumped when its layout changes.
pub const NAMES_FORMAT: u32 = 1;
/// The game languages the launcher speaks.
pub const NAME_LANGUAGES: [&str; 2] = ["en_us", "zh_cn"];
/// The language a missing name falls back to.
const FALLBACK: &str = "en_us";
const MAGIC: &[u8; 8] = b"LUMINAME";
const LEVEL: i32 = 19;
/// Far above every real file (about 200 KB of JSON).
const MAX_FILE: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockNames {
    /// Language (`en_us`) → translation key (`block.minecraft.stone`) → name.
    languages: BTreeMap<String, BTreeMap<String, String>>,
}

/// The `block.`, `item.` and `entity.` names of one language file, by key.
/// Keys with further parts (`block.minecraft.banner.base.white`) are not
/// names of a thing and are left out.
pub fn names_in_lang_file(json: &[u8]) -> Result<BTreeMap<String, String>, ContainerError> {
    let all: BTreeMap<String, serde_json::Value> = serde_json::from_slice(json)
        .map_err(|error| ContainerError::Corrupt(format!("language file: {error}")))?;
    Ok(all
        .into_iter()
        .filter(|(key, _)| is_name_key(key))
        .filter_map(|(key, value)| Some((key, value.as_str()?.to_owned())))
        .collect())
}

fn is_name_key(key: &str) -> bool {
    let mut parts = key.split('.');
    matches!(parts.next(), Some("block" | "item" | "entity"))
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_some_and(|part| !part.is_empty())
        && parts.next().is_none()
}

/// `minecraft:smooth_stone_slab` → `("minecraft", "smooth_stone_slab")`; a
/// bare name is in the `minecraft` namespace. Block-state properties
/// (`[type=bottom]`) are dropped.
fn split_id(id: &str) -> (&str, &str) {
    let id = id.split('[').next().unwrap_or(id).trim();
    id.split_once(':').unwrap_or(("minecraft", id))
}

impl BlockNames {
    /// Adds a language file's names; a key this table already has keeps its
    /// name. Add the newest version first.
    pub fn add(&mut self, language: &str, names: BTreeMap<String, String>) {
        let table = self.languages.entry(language.to_owned()).or_default();
        for (key, name) in names {
            table.entry(key).or_insert(name);
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.languages.values().all(BTreeMap::is_empty)
    }

    /// How many names `language` has.
    #[must_use]
    pub fn len(&self, language: &str) -> usize {
        self.languages.get(language).map_or(0, BTreeMap::len)
    }

    /// The name of block `id` in `language`, else in English: the block's
    /// own name, else its item's.
    #[must_use]
    pub fn block(&self, id: &str, language: &str) -> Option<&str> {
        self.first(id, &["block", "item"], language)
    }

    /// The name of entity `id`, else of the item that places it (item
    /// frames, paintings, boats).
    #[must_use]
    pub fn entity(&self, id: &str, language: &str) -> Option<&str> {
        self.first(id, &["entity", "item"], language)
    }

    fn first(&self, id: &str, kinds: &[&str], language: &str) -> Option<&str> {
        let (namespace, name) = split_id(id);
        let keys: Vec<String> = kinds
            .iter()
            .map(|kind| format!("{kind}.{namespace}.{name}"))
            .collect();
        [language, FALLBACK].iter().find_map(|language| {
            let table = self.languages.get(*language)?;
            keys.iter()
                .find_map(|key| table.get(key).map(String::as_str))
        })
    }

    /// The file `cargo xtask bake-blocks` publishes as `names.dat.z`.
    pub fn to_bytes(&self) -> Result<Vec<u8>, ContainerError> {
        let json =
            serde_json::to_vec(self).map_err(|error| ContainerError::Io(error.to_string()))?;
        let mut file = Vec::with_capacity(12 + json.len());
        file.extend_from_slice(MAGIC);
        file.extend_from_slice(&NAMES_FORMAT.to_le_bytes());
        file.extend_from_slice(&json);
        zstd::encode_all(file.as_slice(), LEVEL)
            .map_err(|error| ContainerError::Io(error.to_string()))
    }

    pub fn read(bytes: &[u8]) -> Result<Self, ContainerError> {
        let decoder = zstd::stream::read::Decoder::new(bytes)
            .map_err(|error| ContainerError::Io(error.to_string()))?;
        let mut data = Vec::new();
        decoder
            .take(MAX_FILE + 1)
            .read_to_end(&mut data)
            .map_err(|_| ContainerError::NotABundle)?;
        if data.len() as u64 > MAX_FILE {
            return Err(ContainerError::Corrupt(
                "larger than any name file".to_owned(),
            ));
        }
        if data.len() < 12 || &data[..8] != MAGIC {
            return Err(ContainerError::NotABundle);
        }
        let format = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if format != NAMES_FORMAT {
            return Err(ContainerError::UnsupportedFormat(format));
        }
        serde_json::from_slice(&data[12..])
            .map_err(|error| ContainerError::Corrupt(format!("names: {error}")))
    }
}
