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

pub use document::{Error, load_litematic, to_litematic_bytes};
pub(crate) use document::{is_air, regions_in_precedence, regions_overlap};
pub use nucleation::{BlockState, Region, UniversalSchematic};

mod document;
