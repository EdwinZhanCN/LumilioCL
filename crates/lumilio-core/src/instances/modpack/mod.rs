//! Importing modpacks: Modrinth's `.mrpack` here, CurseForge's zip in
//! [`curseforge`]. Both become a [`PackIndex`] that is staged the same way.
//!
//! The Modrinth format is the public Modrinth pack format: a zip with
//! `modrinth.index.json`, plus `overrides/` and `client-overrides/` folders
//! copied into the game directory.

pub mod curseforge;

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::Path;

pub use pack_index::{
    ClientSupport, ModpackError, OVERRIDES_LIMIT, PackFile, PackIndex, PendingDownload,
    extract_overrides, import, import_index, is_trusted_source, parse_index, plan, read_index,
};
#[cfg(test)]
use pack_index::{INDEX, override_path};
pub(crate) use pack_index::{is_safe_relative, zip_entry_path};

#[cfg(test)]
use crate::activity::CancellationToken;
#[cfg(test)]
use crate::instances::{InstanceStore, Loader};
#[cfg(test)]
use crate::transfer::{SourceChain, TransferEngine};

#[cfg(test)]
mod tests;

mod pack_index;
