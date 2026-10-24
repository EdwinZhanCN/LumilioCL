//! The file a baked pack is stored in: one zstd frame holding a flat
//! container. The container starts with an index; pixels are stored once
//! however many packs (game versions) use them.
//!
//! ```text
//! "LUMIBAKE"  format: u32 LE  index length: u32 LE  index (JSON)  data …
//! ```
//!
//! The index maps each pack key and each image hash to a byte range of the
//! data. A pack is JSON with its images left out; an image is its width and
//! height (u32 LE) and RGBA8 pixels.

use std::collections::BTreeMap;
use std::io::Read;
use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::ContainerError;
use crate::catalog::BlockCatalog;
use crate::pack::{
    BakedAssets, BakedBlockstate, BakedModel, BakedPack, BakedTexture, ContentHash, Image,
    PackMeta, PaintingVariant, ParentModel,
};

const MAGIC: &[u8; 8] = b"LUMIBAKE";
/// The container format this crate writes and reads; a published index
/// names it so an older launcher can say why it cannot read a newer file.
pub const FORMAT: u32 = 1;
const LEVEL: i32 = 19;
/// Largest container accepted after decompression.
const MAX_CONTAINER: u64 = 1024 * 1024 * 1024;
/// The key of the only pack in a single-pack file.
const SINGLE: &str = "pack";

#[derive(Serialize, Deserialize)]
struct Index {
    packs: BTreeMap<String, (u64, u64)>,
    images: BTreeMap<ContentHash, (u64, u64)>,
}

#[derive(Serialize)]
struct StoredPackRef<'a> {
    meta: &'a PackMeta,
    blockstates: &'a BTreeMap<String, BakedBlockstate>,
    models: &'a BTreeMap<String, BakedModel>,
    textures: &'a BTreeMap<String, BakedTexture>,
    #[serde(skip_serializing_if = "no_samples")]
    tint_samples: &'a BTreeMap<String, [u8; 3]>,
    /// Absent from packs baked before templates were kept (T17); the format
    /// stays 1, since an older reader ignores the field.
    #[serde(skip_serializing_if = "no_parents")]
    parents: &'a BTreeMap<String, ParentModel>,
    /// Absent before T35; the format stays 1 for the same reason.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    paintings: &'a BTreeMap<String, PaintingVariant>,
}

fn no_parents(parents: &&BTreeMap<String, ParentModel>) -> bool {
    parents.is_empty()
}

fn no_samples(samples: &&BTreeMap<String, [u8; 3]>) -> bool {
    samples.is_empty()
}

#[derive(Deserialize)]
struct StoredPack {
    meta: PackMeta,
    blockstates: BTreeMap<String, BakedBlockstate>,
    models: BTreeMap<String, BakedModel>,
    textures: BTreeMap<String, BakedTexture>,
    #[serde(default)]
    tint_samples: BTreeMap<String, [u8; 3]>,
    #[serde(default)]
    parents: BTreeMap<String, ParentModel>,
    #[serde(default)]
    paintings: BTreeMap<String, PaintingVariant>,
}

/// Writes `packs` by key (a game version, say) into one file. The same
/// key and packs always give the same bytes.
pub fn write_bundle<'a>(
    packs: impl IntoIterator<Item = (&'a str, &'a BakedPack)>,
) -> Result<Vec<u8>, ContainerError> {
    let packs: BTreeMap<&str, &BakedPack> = packs.into_iter().collect();
    let mut data = Vec::new();
    let mut index = Index {
        packs: BTreeMap::new(),
        images: BTreeMap::new(),
    };
    for (key, pack) in &packs {
        let start = data.len() as u64;
        let assets = &pack.assets;
        serde_json::to_writer(
            &mut data,
            &StoredPackRef {
                meta: &pack.meta,
                blockstates: &assets.blockstates,
                models: &assets.models,
                textures: &assets.textures,
                tint_samples: &assets.tint_samples,
                parents: &assets.parents,
                paintings: &assets.paintings,
            },
        )
        .map_err(|error| ContainerError::Io(error.to_string()))?;
        index
            .packs
            .insert((*key).to_owned(), (start, data.len() as u64 - start));
    }
    let images: BTreeMap<&ContentHash, &Image> = packs
        .values()
        .flat_map(|pack| &pack.assets.images)
        .collect();
    for (hash, image) in images {
        let start = data.len() as u64;
        data.extend_from_slice(&image.width.to_le_bytes());
        data.extend_from_slice(&image.height.to_le_bytes());
        data.extend_from_slice(&image.rgba);
        index
            .images
            .insert(*hash, (start, data.len() as u64 - start));
    }

    let index =
        serde_json::to_vec(&index).map_err(|error| ContainerError::Io(error.to_string()))?;
    let mut container = Vec::with_capacity(16 + index.len() + data.len());
    container.extend_from_slice(MAGIC);
    container.extend_from_slice(&FORMAT.to_le_bytes());
    container.extend_from_slice(&(index.len() as u32).to_le_bytes());
    container.extend_from_slice(&index);
    container.extend_from_slice(&data);
    zstd::encode_all(container.as_slice(), LEVEL)
        .map_err(|error| ContainerError::Io(error.to_string()))
}

/// A read bundle: the index is checked, packs are decoded on request.
pub struct Bundle {
    data: Vec<u8>,
    /// Where the data section starts in `data`.
    offset: usize,
    index: Index,
}

impl Bundle {
    pub fn read(bytes: &[u8]) -> Result<Self, ContainerError> {
        let decoder = zstd::stream::read::Decoder::new(bytes)
            .map_err(|error| ContainerError::Io(error.to_string()))?;
        let mut data = Vec::new();
        decoder
            .take(MAX_CONTAINER + 1)
            .read_to_end(&mut data)
            .map_err(|_| ContainerError::NotABundle)?;
        if data.len() as u64 > MAX_CONTAINER {
            return Err(ContainerError::Corrupt("larger than any bundle".to_owned()));
        }
        if data.len() < 16 || &data[..8] != MAGIC {
            return Err(ContainerError::NotABundle);
        }
        let word =
            |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
        let format = word(8);
        if format != FORMAT {
            return Err(ContainerError::UnsupportedFormat(format));
        }
        let offset = 16usize
            .checked_add(word(12) as usize)
            .filter(|end| *end <= data.len())
            .ok_or_else(|| ContainerError::Corrupt("the index is cut short".to_owned()))?;
        let index: Index = serde_json::from_slice(&data[16..offset])
            .map_err(|error| ContainerError::Corrupt(format!("the index: {error}")))?;
        let size = (data.len() - offset) as u64;
        let fits =
            |&(start, len): &(u64, u64)| start.checked_add(len).is_some_and(|end| end <= size);
        if !index.packs.values().chain(index.images.values()).all(fits) {
            return Err(ContainerError::Corrupt(
                "the index points past the data".to_owned(),
            ));
        }
        Ok(Self {
            data,
            offset,
            index,
        })
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.index.packs.keys().map(String::as_str)
    }

    /// Distinct images across all packs.
    pub fn image_count(&self) -> usize {
        self.index.images.len()
    }

    /// Decodes the pack `key` with only the images it uses, each checked
    /// against its hash.
    pub fn pack(&self, key: &str) -> Result<BakedPack, ContainerError> {
        let range = self
            .index
            .packs
            .get(key)
            .ok_or_else(|| ContainerError::MissingPack(key.to_owned()))?;
        let stored: StoredPack = serde_json::from_slice(&self.data[self.range(*range)])
            .map_err(|error| ContainerError::Corrupt(format!("pack {key}: {error}")))?;
        let mut images = BTreeMap::new();
        for texture in stored.textures.values() {
            if images.contains_key(&texture.image) {
                continue;
            }
            images.insert(texture.image, self.image(texture.image)?);
        }
        Ok(BakedPack {
            meta: stored.meta,
            assets: BakedAssets {
                blockstates: stored.blockstates,
                models: stored.models,
                textures: stored.textures,
                images,
                tint_samples: stored.tint_samples,
                parents: stored.parents,
                paintings: stored.paintings,
            },
        })
    }

    /// The block states pack `key` knows, without decoding its images.
    pub fn catalog(&self, key: &str) -> Result<BlockCatalog, ContainerError> {
        let range = self
            .index
            .packs
            .get(key)
            .ok_or_else(|| ContainerError::MissingPack(key.to_owned()))?;
        let stored: StoredPack = serde_json::from_slice(&self.data[self.range(*range)])
            .map_err(|error| ContainerError::Corrupt(format!("pack {key}: {error}")))?;
        Ok(BlockCatalog::from_blockstates(&stored.blockstates))
    }

    fn range(&self, (start, len): (u64, u64)) -> Range<usize> {
        // Checked against the data size in `read`.
        let start = self.offset + start as usize;
        start..start + len as usize
    }

    fn image(&self, hash: ContentHash) -> Result<Image, ContainerError> {
        let range = self
            .index
            .images
            .get(&hash)
            .ok_or_else(|| ContainerError::Corrupt(format!("image {hash} is missing")))?;
        let bytes = &self.data[self.range(*range)];
        let corrupt = || ContainerError::Corrupt(format!("image {hash} is damaged"));
        if bytes.len() < 8 {
            return Err(corrupt());
        }
        let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let expected = (width as u64) * (height as u64) * 4;
        if expected != (bytes.len() - 8) as u64 {
            return Err(corrupt());
        }
        let image = Image {
            width,
            height,
            rgba: bytes[8..].to_vec(),
        };
        if image.hash() != hash {
            return Err(corrupt());
        }
        Ok(image)
    }
}

impl BakedPack {
    /// This pack alone in a file (a local bake's cache entry).
    pub fn to_bytes(&self) -> Result<Vec<u8>, ContainerError> {
        write_bundle([(SINGLE, self)])
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ContainerError> {
        Bundle::read(bytes)?.pack(SINGLE)
    }
}
