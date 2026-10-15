//! Entity model geometry (plan `schematic-studio`, T6/D4).
//!
//! `tools/entity-dump` calls the game's `LayerDefinitions.createRoots()` and
//! writes every model layer as JSON ([`EntityDump`]). The bake collects one
//! dump per game version into `entities.dat.z` ([`write_entities`],
//! [`EntityBundle`]); a layer several versions share is stored once.
//!
//! Numbers are the game's own: 1/16-block units, Y down, rotations in
//! radians applied Z, then Y, then X, each bone relative to its parent.
//!
//! ```text
//! zstd( "LUMIENTS"  format: u32 LE  JSON )
//! ```

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use serde::{Deserialize, Serialize};

use crate::ContainerError;
use crate::pack::Direction;

mod dumper;

pub use dumper::{
    DUMPER_CLASS, DUMPER_CLASS_FILE, DUMPER_JAVA, DUMPER_MAIN, LAYER_DEFINITIONS_CLASS,
    dumper_arguments, dumper_failure, install_dumper, read_dump,
};

const MAGIC: &[u8; 8] = b"LUMIENTS";
/// The entity file format this crate writes and reads.
pub const ENTITY_FORMAT: u32 = 1;
/// The dump format `tools/entity-dump` writes: 2 adds the renderers' looks
/// (plan `schematic-studio`, T34). A format 1 dump, which has only the
/// layers, is still read.
pub const DUMP_FORMAT: u32 = 2;
const LEVEL: i32 = 19;
const MAX_FILE: u64 = 256 * 1024 * 1024;

/// One model layer, such as `minecraft:chest#main`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityLayer {
    /// The texture sheet size the UVs are in, in pixels.
    pub texture_size: [u32; 2],
    pub root: Bone,
}

/// A part of a model, posed relative to its parent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bone {
    /// Translation from the parent.
    pub pivot: [f32; 3],
    /// Rotation in radians about X, Y and Z (applied Z, Y, X).
    pub rotation: [f32; 3],
    #[serde(default = "one3", skip_serializing_if = "is_one3")]
    pub scale: [f32; 3],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cubes: Vec<Cube>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub children: BTreeMap<String, Bone>,
}

/// A box of a bone, in the bone's space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cube {
    /// The corner with the smallest coordinates, before `inflate`.
    pub origin: [f32; 3],
    pub size: [f32; 3],
    /// The texture offset in pixels.
    pub uv: [f32; 2],
    #[serde(default = "one2", skip_serializing_if = "is_one2")]
    pub uv_scale: [f32; 2],
    /// Growth on each side along X, Y and Z.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub inflate: [f32; 3],
    #[serde(default, skip_serializing_if = "is_false")]
    pub mirror: bool,
    /// The faces drawn; `None` draws all six.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<Vec<Direction>>,
}

fn one3() -> [f32; 3] {
    [1.0; 3]
}

fn one2() -> [f32; 2] {
    [1.0; 2]
}

fn is_one3(value: &[f32; 3]) -> bool {
    *value == [1.0; 3]
}

fn is_one2(value: &[f32; 2]) -> bool {
    *value == [1.0; 2]
}

fn is_zero3(value: &[f32; 3]) -> bool {
    *value == [0.0; 3]
}

fn is_false(value: &bool) -> bool {
    !value
}

/// What `tools/entity-dump` writes for one client jar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityDump {
    pub format: u32,
    /// By `ModelLayerLocation`, `namespace:path#layer`.
    pub layers: BTreeMap<String, EntityLayer>,
    /// By entity type (`minecraft:bee`): what the game's own renderer for it
    /// used when the dumper built it (plan `schematic-studio`, T34, D32).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub entities: BTreeMap<String, EntityLook>,
    /// Why no renderer could be built in this version at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entities_error: Option<String>,
}

/// What the game's renderer for one entity type used when the dumper built
/// it from a context holding only the model layers (plan `schematic-studio`,
/// T34, D32): the model layers it asked for, in order (the main model
/// first), and its textures. Simple class names, most specific first.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityLook {
    /// The renderer's classes, up to (without) `EntityRenderer`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub renderer: Vec<String>,
    /// The entity's classes, up to `Entity`: what kind of entity it is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<String>,
    /// A fresh render state's texture (`minecraft:textures/entity/bee/bee.png`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub texture: Option<String>,
    /// The same with the state's `isBaby` set, where it differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baby_texture: Option<String>,
    /// Textures the renderer keeps in fields (a boat's, a minecart's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texture_fields: Vec<String>,
    /// Why its renderer could not be built, or that it has none here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl EntityLook {
    /// Whether its renderer is a living entity's (`LivingEntityRenderer`).
    #[must_use]
    pub fn is_living(&self) -> bool {
        self.renderer
            .iter()
            .any(|class| class == "LivingEntityRenderer")
    }
}

impl EntityDump {
    pub fn from_json(bytes: &[u8]) -> Result<Self, ContainerError> {
        let dump: Self = serde_json::from_slice(bytes)
            .map_err(|error| ContainerError::Corrupt(format!("entity dump: {error}")))?;
        if !(1..=DUMP_FORMAT).contains(&dump.format) {
            return Err(ContainerError::UnsupportedFormat(dump.format));
        }
        Ok(dump)
    }

    /// The dump as a renderer is handed it: its layers and its looks.
    #[must_use]
    pub fn into_layers(self) -> EntityLayers {
        let mut layers: EntityLayers = self.layers.into_iter().collect();
        layers.looks = self.entities;
        layers
    }

    /// The canonical JSON of the dump: what the bake and the launcher keep
    /// from a dumper run, so one jar always gives the same bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, ContainerError> {
        serde_json::to_vec(self).map_err(|error| ContainerError::Io(error.to_string()))
    }
}

/// A game version in the entity file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityVersion {
    pub id: String,
    /// Set when the version has no layer definitions of its own (before
    /// 1.17) and borrows another version's geometry (D4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry_from: Option<String>,
    /// Layer id → index into [`EntityBundle::layers`].
    pub layers: BTreeMap<String, u32>,
    /// What each entity type's renderer used (T34); empty for versions
    /// whose renderers were not built.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub looks: BTreeMap<String, EntityLook>,
}

/// The geometry of every baked game version, layers stored once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityBundle {
    pub versions: Vec<EntityVersion>,
    pub layers: Vec<EntityLayer>,
}

/// One version going into [`write_entities`].
pub enum EntitySource<'a> {
    Dumped(&'a EntityDump),
    /// Uses the geometry of the dumped version with this id, and these
    /// looks (borrowed from it too; see [`borrowed_looks`]).
    SameAs(&'a str, BTreeMap<String, EntityLook>),
}

/// Gives a renderer that drew its model with code the layers its newer
/// version draws the same model from (plan `schematic-studio`, T35): before
/// 1.21.2 `ArrowRenderer` wrote its quads by hand, and from then the same
/// quads are the `arrow` layer, on the same texture. A look built without
/// error and without layers takes the nearest newer look of the same
/// renderer classes that has layers this version lacks, and the version
/// takes their geometry. A look whose renderer could not be built at all
/// (from 1.21.9 a painting's needs the paintings atlas) first takes the
/// renderer and entity classes of the nearest version where it was built,
/// so a row for its kind of renderer still finds it. `dumps` are in release
/// order.
pub fn borrow_layers(dumps: &mut [(String, EntityDump)]) {
    /// An entity's renderer and entity classes.
    type Classes = (Vec<String>, Vec<String>);
    let classes: Vec<BTreeMap<String, Classes>> = dumps
        .iter()
        .map(|(_, dump)| {
            dump.entities
                .iter()
                .filter(|(_, look)| !look.renderer.is_empty())
                .map(|(id, look)| (id.clone(), (look.renderer.clone(), look.entity.clone())))
                .collect()
        })
        .collect();
    for (at, (_, dump)) in dumps.iter_mut().enumerate() {
        for (id, look) in &mut dump.entities {
            if !look.renderer.is_empty() {
                continue;
            }
            let nearest = (1..classes.len())
                .flat_map(|step| [at.checked_sub(step), Some(at + step)])
                .flatten()
                .find_map(|other| classes.get(other)?.get(id));
            if let Some((renderer, entity)) = nearest {
                look.renderer.clone_from(renderer);
                look.entity.clone_from(entity);
            }
        }
    }
    for at in (0..dumps.len()).rev() {
        let (older, newer) = dumps.split_at_mut(at + 1);
        let dump = &mut older[at].1;
        let mut lent = BTreeMap::new();
        for (id, look) in &mut dump.entities {
            if look.error.is_some() || look.renderer.is_empty() || !look.layers.is_empty() {
                continue;
            }
            let donor = newer.iter().find_map(|(_, newer)| {
                let other = newer.entities.get(id)?;
                (other.renderer == look.renderer
                    && !other.layers.is_empty()
                    && other.layers.iter().all(|layer| {
                        !dump.layers.contains_key(layer) && newer.layers.contains_key(layer)
                    }))
                .then(|| (other.layers.clone(), &newer.layers))
            });
            let Some((layers, geometry)) = donor else {
                continue;
            };
            for layer in &layers {
                lent.insert(layer.clone(), geometry[layer].clone());
            }
            look.layers = layers;
        }
        for (layer, geometry) in lent {
            dump.layers.entry(layer).or_insert(geometry);
        }
    }
}

/// Fills in what each dumped version's renderers could not say (plan
/// `schematic-studio`, T34), from the nearest newer version's look of the
/// same entity that has a texture. `dumps` are in release order; `has(version,
/// texture)` says whether that version's own pack has the texture.
///
/// - Before 1.21.2 a renderer takes its texture from the entity, not a render
///   state, so the dumper has none: a look takes the newer one's textures
///   when both draw the same main layer.
/// - A renderer that could not be built here (from 1.19.4 to 1.21.1 a
///   humanoid's armour layer needs the armour trim atlas) takes the newer
///   look whole, keeping only the layers this version has, when this version
///   has its main layer.
pub fn borrow_textures(dumps: &mut [(String, EntityDump)], has: impl Fn(&str, &str) -> bool) {
    for at in (0..dumps.len()).rev() {
        let (older, newer) = dumps.split_at_mut(at + 1);
        let (version, dump) = &mut older[at];
        let layers = &dump.layers;
        for (id, look) in &mut dump.entities {
            // An error says the renderer was not built, whatever classes the look
            // borrowed since ([`borrow_layers`]).
            let unbuilt = look.error.is_some();
            if look.texture.is_some() || (!unbuilt && look.layers.is_empty()) {
                continue;
            }
            let donor = newer.iter().find_map(|(_, newer)| {
                newer.entities.get(id).filter(|other| {
                    let Some(main) = other.layers.first() else {
                        return false;
                    };
                    other.texture.is_some()
                        && if unbuilt {
                            layers.contains_key(main)
                        } else {
                            look.layers.first() == Some(main)
                        }
                })
            });
            let Some(donor) = donor else {
                continue;
            };
            let texture = donor
                .texture
                .clone()
                .filter(|texture| has(version, texture));
            if texture.is_none() {
                continue;
            }
            if unbuilt {
                *look = EntityLook {
                    layers: donor
                        .layers
                        .iter()
                        .filter(|layer| layers.contains_key(*layer))
                        .cloned()
                        .collect(),
                    texture_fields: Vec::new(),
                    ..donor.clone()
                };
            }
            look.texture = texture;
            look.baby_texture = donor
                .baby_texture
                .clone()
                .filter(|texture| has(version, texture));
        }
    }
}

/// The looks of `from` that a version borrowing its geometry can draw: those
/// whose textures `has(version, texture)` says the version's own pack has.
#[must_use]
pub fn borrowed_looks(
    from: &EntityDump,
    version: &str,
    has: impl Fn(&str, &str) -> bool,
) -> BTreeMap<String, EntityLook> {
    from.entities
        .iter()
        .filter(|(_, look)| {
            look.texture
                .as_deref()
                .is_some_and(|texture| has(version, texture))
        })
        .map(|(id, look)| {
            let mut look = look.clone();
            look.baby_texture = look.baby_texture.filter(|texture| has(version, texture));
            (id.clone(), look)
        })
        .collect()
}

/// Writes the versions, in the order given, into `entities.dat.z`. The same
/// input always gives the same bytes.
pub fn write_entities<'a>(
    versions: impl IntoIterator<Item = (&'a str, EntitySource<'a>)>,
) -> Result<Vec<u8>, ContainerError> {
    let versions: Vec<_> = versions.into_iter().collect();
    let mut bundle = EntityBundle {
        versions: Vec::new(),
        layers: Vec::new(),
    };
    let mut stored: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut dumped: HashMap<&str, BTreeMap<String, u32>> = HashMap::new();
    for (id, source) in &versions {
        let EntitySource::Dumped(dump) = source else {
            continue;
        };
        let mut layers = BTreeMap::new();
        for (name, layer) in &dump.layers {
            let key =
                serde_json::to_vec(layer).map_err(|error| ContainerError::Io(error.to_string()))?;
            let next = bundle.layers.len() as u32;
            let at = *stored.entry(key).or_insert(next);
            if at == next {
                bundle.layers.push(layer.clone());
            }
            layers.insert(name.clone(), at);
        }
        dumped.insert(id, layers);
    }
    for (id, source) in &versions {
        let (from, layers, looks) = match source {
            EntitySource::Dumped(dump) => (None, dumped[id].clone(), dump.entities.clone()),
            EntitySource::SameAs(from, looks) => {
                let layers = dumped
                    .get(from)
                    .ok_or_else(|| ContainerError::MissingPack((*from).to_owned()))?;
                (Some((*from).to_owned()), layers.clone(), looks.clone())
            }
        };
        bundle.versions.push(EntityVersion {
            id: (*id).to_owned(),
            geometry_from: from,
            layers,
            looks,
        });
    }
    let json =
        serde_json::to_vec(&bundle).map_err(|error| ContainerError::Io(error.to_string()))?;
    let mut file = Vec::with_capacity(12 + json.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(&ENTITY_FORMAT.to_le_bytes());
    file.extend_from_slice(&json);
    zstd::encode_all(file.as_slice(), LEVEL).map_err(|error| ContainerError::Io(error.to_string()))
}

impl EntityBundle {
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
                "larger than any entity file".to_owned(),
            ));
        }
        if data.len() < 12 || &data[..8] != MAGIC {
            return Err(ContainerError::NotABundle);
        }
        let format = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        if format != ENTITY_FORMAT {
            return Err(ContainerError::UnsupportedFormat(format));
        }
        let bundle: Self = serde_json::from_slice(&data[12..])
            .map_err(|error| ContainerError::Corrupt(format!("entity geometry: {error}")))?;
        let count = bundle.layers.len();
        if bundle
            .versions
            .iter()
            .flat_map(|version| version.layers.values())
            .any(|&at| at as usize >= count)
        {
            return Err(ContainerError::Corrupt(
                "a version points past the layers".to_owned(),
            ));
        }
        Ok(bundle)
    }

    pub fn version(&self, id: &str) -> Option<&EntityVersion> {
        self.versions.iter().find(|version| version.id == id)
    }

    /// The layer `layer` (`minecraft:chest#main`) of game version `version`.
    pub fn layer(&self, version: &str, layer: &str) -> Option<&EntityLayer> {
        let at = *self.version(version)?.layers.get(layer)?;
        self.layers.get(at as usize)
    }

    /// Every layer of game version `version`, the geometry a renderer is
    /// handed beside that version's [`crate::BakedPack`] (plan
    /// `schematic-studio`, T7). Empty for a version the file does not know.
    pub fn layers_of(&self, version: &str) -> EntityLayers {
        let Some(version) = self.version(version) else {
            return EntityLayers::new();
        };
        let mut layers: EntityLayers = version
            .layers
            .iter()
            .filter_map(|(name, &at)| Some((name.clone(), self.layers.get(at as usize)?.clone())))
            .collect();
        layers.looks = version.looks.clone();
        layers
    }
}

/// One game version's entities as a renderer is handed them: the model
/// layers by id (`minecraft:chest#main`), which it reads as a map, and the
/// looks its renderers gave each entity type, by id (`minecraft:bee`; T34).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntityLayers {
    layers: BTreeMap<String, EntityLayer>,
    pub looks: BTreeMap<String, EntityLook>,
}

impl EntityLayers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl std::ops::Deref for EntityLayers {
    type Target = BTreeMap<String, EntityLayer>;

    fn deref(&self) -> &Self::Target {
        &self.layers
    }
}

impl std::ops::DerefMut for EntityLayers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.layers
    }
}

impl FromIterator<(String, EntityLayer)> for EntityLayers {
    fn from_iter<I: IntoIterator<Item = (String, EntityLayer)>>(iter: I) -> Self {
        Self {
            layers: iter.into_iter().collect(),
            looks: BTreeMap::new(),
        }
    }
}

impl<const N: usize> From<[(String, EntityLayer); N]> for EntityLayers {
    fn from(layers: [(String, EntityLayer); N]) -> Self {
        layers.into_iter().collect()
    }
}

impl IntoIterator for EntityLayers {
    type Item = (String, EntityLayer);
    type IntoIter = std::collections::btree_map::IntoIter<String, EntityLayer>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.into_iter()
    }
}

impl<'a> IntoIterator for &'a EntityLayers {
    type Item = (&'a String, &'a EntityLayer);
    type IntoIter = std::collections::btree_map::Iter<'a, String, EntityLayer>;

    fn into_iter(self) -> Self::IntoIter {
        self.layers.iter()
    }
}

#[cfg(test)]
mod tests;
