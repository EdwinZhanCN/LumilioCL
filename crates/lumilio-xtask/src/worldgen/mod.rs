//! Reproducible development tools for the maintained world generator.
mod btree;
mod decorations;
mod loot;
mod prepare;
mod saves;
#[cfg(test)]
mod tests;

#[cfg(test)]
use std::path::Path;

pub(super) use fixtures::read_tree;
use fixtures::releases;
pub(crate) use fixtures::{btree, diff, golden, import, prepare, saved, tables};
#[cfg(test)]
use serde_json::Value;

mod fixtures;
