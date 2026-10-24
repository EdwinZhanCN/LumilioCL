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

pub use dumper::{
    DUMPER_CLASS, DUMPER_CLASS_FILE, DUMPER_JAVA, DUMPER_MAIN, LAYER_DEFINITIONS_CLASS,
    dumper_arguments, dumper_failure, install_dumper, read_dump,
};
pub use entity_bundle::{
    Bone, Cube, DUMP_FORMAT, ENTITY_FORMAT, EntityBundle, EntityDump, EntityLayer, EntityLayers,
    EntityLook, EntitySource, EntityVersion, borrow_layers, borrow_textures, borrowed_looks,
    write_entities,
};

#[cfg(test)]
use crate::ContainerError;
#[cfg(test)]
use crate::pack::Direction;

mod dumper;

#[cfg(test)]
mod tests;

mod entity_bundle;
