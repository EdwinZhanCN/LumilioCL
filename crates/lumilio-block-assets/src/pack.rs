//! The baked format: what a renderer needs to draw one game version's blocks,
//! with every version difference already settled by the baker.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

/// One game version's blocks (or a resource-pack overlay), ready to mesh.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BakedPack {
    /// Where the assets came from. Two bakes of the same assets from
    /// different files differ only here.
    pub meta: PackMeta,
    pub assets: BakedAssets,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackMeta {
    /// `world_version` from the client's `version.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_version: Option<u32>,
    /// `id` from the client's `version.json`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_version: Option<String>,
    /// The sources in the order they were layered, lowest first.
    pub sources: Vec<SourceStamp>,
    /// Entries that could not be read and were left out, as `name: reason`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// A source as recorded in a pack. Paths are not stored: a pack may be shared.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceStamp {
    pub kind: SourceKind,
    /// SHA-1 of the archive file; directories have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    /// Blockstate files this source holds: the blocks it adds or redraws.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub blocks: u32,
    /// Asset files of any kind this source holds; none means it is not a
    /// resource source at all (a library mod, say).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub assets: u32,
    /// The source could not be read as a resource archive and was left out
    /// (only an overlay is left out; an unreadable base fails the bake).
    #[serde(default, skip_serializing_if = "is_false")]
    pub unreadable: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    #[default]
    Archive,
    Directory,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BakedAssets {
    /// Block id (`minecraft:stone`) → its states.
    pub blockstates: BTreeMap<String, BakedBlockstate>,
    /// Model id (`minecraft:block/stone`) → the model with its parents applied.
    /// Kept are the models some blockstate uses and every `item/` model (the
    /// items block entities and entities hold are drawn with them).
    pub models: BTreeMap<String, BakedModel>,
    /// Texture id (`minecraft:block/stone`) → its pixels.
    pub textures: BTreeMap<String, BakedTexture>,
    /// Pixels by content, shared by every texture id that has them.
    pub images: BTreeMap<ContentHash, Image>,
    /// Each biome colormap by name (`grass`, `foliage`, `dry_foliage`),
    /// sampled where the game samples it for plains: the color that tints
    /// grass, leaves and leaf litter there (plan schematic-studio, T5).
    pub tint_samples: BTreeMap<String, [u8; 3]>,
    /// Model id → the model as written, for every model another model names
    /// as its parent (`minecraft:block/cube_all` …). Only the baker reads
    /// them: [`crate::bake_over`] resolves mod and resource-pack models that
    /// inherit from this pack's models against them (plan schematic-studio,
    /// T17).
    pub parents: BTreeMap<String, ParentModel>,
    /// Painting variant id (`minecraft:kebab`) → its size and picture (plan
    /// schematic-studio, T35): from the game's `painting_variant` data, or
    /// before 1.21 from each picture's size at 16 pixels a block.
    pub paintings: BTreeMap<String, PaintingVariant>,
}

/// One painting: its size in blocks and its picture's texture id
/// (`minecraft:painting/kebab`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaintingVariant {
    pub width: u32,
    pub height: u32,
    pub texture: String,
}

/// A model before inheritance, in one spelling for every game version:
/// texture ids namespaced and renamed as everywhere in a pack, `#name`
/// references kept, elements and display transforms parsed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParentModel {
    /// Namespaced, or `builtin/…` as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(
        rename = "ambientocclusion",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub ambient_occlusion: Option<bool>,
    /// Variable → a texture id or a `#name` reference.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub textures: BTreeMap<String, String>,
    /// A face's `texture` is a texture id or a `#name` reference.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<BakedElement>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub display: BTreeMap<String, DisplayTransform>,
}

impl BakedAssets {
    pub fn texture_image(&self, id: &str) -> Option<&Image> {
        self.images.get(&self.textures.get(id)?.image)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BakedBlockstate {
    /// Canonical property key (`facing=north,half=bottom`, names sorted; ``
    /// for a block without properties) → the weighted models for it.
    Variants(BTreeMap<String, Vec<BakedVariant>>),
    Multipart(Vec<BakedCase>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedCase {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Condition>,
    pub apply: Vec<BakedVariant>,
}

/// A multipart condition. Values are strings as in the state key: a game
/// version that writes `"power": 0` or `"lit": true` bakes the same as one
/// that writes `"0"` and `"true"`. A value may still list alternatives
/// (`side|up`) or start with `!`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Any(Vec<Condition>),
    All(Vec<Condition>),
    Properties(BTreeMap<String, String>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedVariant {
    /// Model id, namespaced.
    pub model: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub x: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub y: i32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub uvlock: bool,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedModel {
    /// Ambient occlusion after inheritance (the game's default is on).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub ambient_occlusion: bool,
    /// Texture variables with every `#reference` followed to a texture id;
    /// variables that lead nowhere are left out.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub textures: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<BakedElement>,
    /// Display transforms by context (`fixed`, `ground`, `gui` …) after
    /// inheritance; item frames, armor stands and dropped items use them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub display: BTreeMap<String, DisplayTransform>,
}

/// How an item model is placed in one display context.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayTransform {
    /// Degrees about X, Y and Z.
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedElement {
    pub from: [f32; 3],
    pub to: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<Rotation>,
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub shade: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shade_direction_override: Option<Direction>,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub light_emission: u8,
    pub faces: BTreeMap<Direction, BakedFace>,
}

/// An element rotation. The single-axis form (`axis`, `angle`) and the Euler
/// form (`x`, `y`, `z`) both bake to degrees about each axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rotation {
    pub origin: [f32; 3],
    /// Degrees about X, Y and Z.
    pub angles: [f32; 3],
    #[serde(default, skip_serializing_if = "is_false")]
    pub rescale: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BakedFace {
    /// Texture id; `None` when the model's variable leads nowhere (the game
    /// draws its missing-texture tile).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// As written; the game derives it from the element when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uv: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cullface: Option<Direction>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tintindex: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    /// The name in any game version: newer models also write `bottom`/`top`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "down" | "bottom" => Self::Down,
            "up" | "top" => Self::Up,
            "north" => Self::North,
            "south" => Self::South,
            "west" => Self::West,
            "east" => Self::East,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BakedTexture {
    pub image: ContentHash,
    /// The `animation` section of the texture's `.mcmeta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    pub frametime: u32,
    #[serde(default, skip_serializing_if = "is_false")]
    pub interpolate: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// `(index, ticks)`; `None` plays the frames in order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<(u32, Option<u32>)>>,
}

/// Straight RGBA8 pixels, row by row.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn hash(&self) -> ContentHash {
        let mut hasher = Sha256::new();
        hasher.update(self.width.to_le_bytes());
        hasher.update(self.height.to_le_bytes());
        hasher.update(&self.rgba);
        ContentHash(hasher.finalize().into())
    }
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Image({}×{})", self.width, self.height)
    }
}

/// SHA-256 of an image's size and pixels, written as hex.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(pub [u8; 32]);

impl Display for ContentHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ContentHash({self})")
    }
}

impl FromStr for ContentHash {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let bytes = text.as_bytes();
        if bytes.len() != 64 {
            return Err(format!("`{text}` is not a content hash"));
        }
        let mut hash = [0u8; 32];
        for (index, pair) in bytes.chunks(2).enumerate() {
            let pair = std::str::from_utf8(pair).map_err(|e| e.to_string())?;
            hash[index] = u8::from_str_radix(pair, 16)
                .map_err(|_| format!("`{text}` is not a content hash"))?;
        }
        Ok(Self(hash))
    }
}

impl Serialize for ContentHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ContentHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// `minecraft:` is the namespace of an id written without one.
pub fn namespaced(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_true(value: &bool) -> bool {
    *value
}

fn yes() -> bool {
    true
}

fn one() -> u32 {
    1
}

fn is_one(value: &u32) -> bool {
    *value == 1
}
