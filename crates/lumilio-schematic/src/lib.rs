//! Schematic domain logic for the studio (plan schematic-studio, D6/D10).
//!
//! Every operation reads or edits a [`UniversalSchematic`] in memory and
//! returns values or bytes. Reading files and writing them back (with backups)
//! is the host's job.
//!
//! Where regions overlap, a cell belongs to the first region that holds a
//! non-air block there, in Nucleation's precedence order: the default region,
//! then the named regions by name. The renderer draws the same single block.

pub mod convert;
pub mod fingerprint;
pub mod inspect;
pub mod metadata;
pub mod replace;
pub mod stats;
pub mod suggest;

#[cfg(test)]
mod tests;

pub use nucleation::{BlockState, Region, UniversalSchematic};

use std::fmt;

/// A schematic could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Reads a `.litematic` file's bytes.
pub fn load_litematic(bytes: &[u8]) -> Result<UniversalSchematic, Error> {
    nucleation::litematic::from_litematic(bytes).map_err(|error| Error(error.to_string()))
}

/// Writes a schematic as `.litematic` bytes at its current data version.
pub fn to_litematic_bytes(schematic: &UniversalSchematic) -> Result<Vec<u8>, Error> {
    nucleation::litematic::to_litematic(schematic).map_err(|error| Error(error.to_string()))
}

/// Regions in precedence order: the default region, then named regions by name.
///
/// A schematic built in memory starts with an empty 1×1×1 default region that
/// only waits for its first block; that placeholder is left out.
pub(crate) fn regions_in_precedence(schematic: &UniversalSchematic) -> Vec<&Region> {
    let default = &schematic.default_region;
    let placeholder = default.size == (1, 1, 1)
        && default.is_empty()
        && default.entities.is_empty()
        && default.block_entities.is_empty();
    let mut named: Vec<&Region> = schematic.other_regions.values().collect();
    named.sort_by(|a, b| a.name.cmp(&b.name));
    (!placeholder)
        .then_some(default)
        .into_iter()
        .chain(named)
        .collect()
}

/// Whether any two regions' boxes share a cell.
pub(crate) fn regions_overlap(regions: &[&Region]) -> bool {
    let boxes: Vec<_> = regions.iter().map(|r| r.get_bounding_box()).collect();
    boxes
        .iter()
        .enumerate()
        .any(|(i, a)| boxes[i + 1..].iter().any(|b| a.intersects(b)))
}

/// The three air blocks Nucleation treats as empty space.
pub(crate) fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}
