//! Reading games made by other launchers (user path: `docs/ia/paths/library.md`).
//!
//! Two layouts are understood: a MultiMC/Prism instance (`instance.cfg` and
//! `mmc-pack.json`, game files in `.minecraft` or `minecraft`) and a plain
//! `.minecraft` folder (the game and loader read from `versions/*/*.json`,
//! see `profile.rs`). Reading never modifies the source.

mod profile;

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;

pub(crate) use detection::copy_game_files;
pub use detection::{FoundGame, GameOrigin, ImportError, detect};

#[cfg(test)]
use crate::instance::Loader;

#[cfg(test)]
mod tests;

mod detection;
