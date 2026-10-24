//! Safe boundary around the pinned cubiomes source snapshot.
mod decorations;
mod ffi;
mod finders;
mod loot;
mod ravines;
mod veins;

pub use decorations::{Decoration, MAX_DECORATION_AREA, Placed, decorations};
pub use finders::{
    Bastion, MAX_AREA, Position, Spawn, Structure, Variant, slime_chunks, spawn, strongholds,
    structures,
};
pub use generator::{
    Dimension, Error, Range, SUPPORTED_VERSIONS, Version, biome_colors, biomes, biomes_until, probe,
};
pub use loot::{Chest, desert_pyramid_chests, desert_pyramid_loot_available};
pub use ravines::{MAX_RAVINE_AREA, Ravine, ravines, ravines_available};
pub use veins::{MAX_VEIN_CELLS, Vein, vein_zones};

#[cfg(test)]
mod tests;

mod generator;
