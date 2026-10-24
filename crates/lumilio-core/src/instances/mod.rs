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
use instance_store::{Index, SCHEMA_VERSION, int, unsigned};
pub(crate) use library::InstanceLibrary;
pub use library::Library;
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
use crate::layout::Layout;
#[cfg(test)]
use crate::tuning::InstanceLaunch;

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

mod instance_store;
