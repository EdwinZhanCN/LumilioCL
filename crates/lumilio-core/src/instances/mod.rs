//! The library of instances a user owns, persisted in `launcher.db` under the
//! launcher root.
//!
//! The store is small and synchronous; callers on an interactive thread should
//! open and mutate it from a background task.

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;

pub(crate) use access::{InstanceBusy, Instances};
pub use instance_store::{
    Collection, InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
use instance_store::{Index, int, unsigned};
pub(crate) use library::InstanceLibrary;
pub use library::Library;
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
use crate::settings::tuning::InstanceLaunch;
#[cfg(test)]
use crate::storage::Layout;

#[cfg(test)]
mod tests;

mod access;

mod database;

mod library;
mod lifecycle;
mod modpacks;
mod packs;
mod portability;
mod snapshots;

mod collections;

pub(crate) mod backup;
pub(crate) mod copy;
pub(crate) mod deletion;
pub(crate) mod history;
pub(crate) mod import_game;
mod instance_store;
pub(crate) mod modpack;
pub(crate) mod pack_export;
pub(crate) mod snapshot_files;
pub(crate) mod staged;
