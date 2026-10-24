//! The library of instances a user owns, persisted in `launcher.db` under the
//! launcher root.
//!
//! The store is small and synchronous; callers on an interactive thread should
//! open and mutate it from a background task.

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;

pub use instance_store::{
    Collection, InstanceRecord, InstanceSettings, InstanceStore, Loader, NewInstance, StoreError,
};
use instance_store::{Index, SCHEMA_VERSION, int, unsigned};
#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
use crate::layout::Layout;
#[cfg(test)]
use crate::tuning::InstanceLaunch;

#[cfg(test)]
mod tests;

mod database;

mod collections;

mod instance_store;
