//! Schematic edits written to a game's files (plan schematic-studio, D8).
//!
//! Every write keeps the replaced file in the instance's launcher data, writes
//! only when the file still looks as it did when it was read, and appends one
//! line to the instance's operation log. The log and its backups are what the
//! studio's history lists and restores from; the game directory holds neither.
//!
//! These functions do blocking file I/O; the service runs them off the async
//! threads and holds the instance so that a running game keeps them out.

#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use lumilio_plugin_api::PluginError;
#[cfg(test)]
pub(crate) use operations::{BACKUP_DIR, LOG_FILE, RESTORE};
pub use operations::{
    KeptBackup, SchematicLogEntry, SchematicOperation, apply, backups, entry, history, remove,
    restore, restore_backup,
};

#[cfg(test)]
use crate::plugins::write::describe;

pub mod schemes;
#[cfg(test)]
mod tests;

mod operations;
