//! Replacement tables kept by name, and the target blocks used last (plan
//! schematic-studio, T24): the studio saves a table as a 方案 and offers it
//! on any other schematic. One file in launcher data, shared by every
//! instance; a missing or unreadable file reads as none.

use std::collections::BTreeMap;
use std::path::Path;
use std::{fs, io};

use serde::{Deserialize, Serialize};

/// How many recent targets are kept.
pub const RECENT: usize = 12;

/// Saved tables by name, and the targets chosen last.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceSchemes {
    #[serde(default)]
    pub schemes: BTreeMap<String, Vec<SchemeRow>>,
    /// Target block ids, the latest first.
    #[serde(default)]
    pub recent: Vec<String>,
}

/// One row of a saved table: a block, the properties it must have (the
/// others match any value), and the block it becomes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemeRow {
    pub source: String,
    #[serde(default)]
    pub properties: Vec<(String, String)>,
    pub target: String,
}

impl ReplaceSchemes {
    /// Puts `targets` first among the recent ones, keeping [`RECENT`].
    pub fn used(&mut self, targets: impl IntoIterator<Item = String>) {
        for target in targets.into_iter().collect::<Vec<_>>().into_iter().rev() {
            self.recent.retain(|recent| *recent != target);
            self.recent.insert(0, target);
        }
        self.recent.truncate(RECENT);
    }
}

/// The saved tables in `path`; none when there is no readable file.
#[must_use]
pub fn read(path: &Path) -> ReplaceSchemes {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Writes `schemes` to `path` through a temporary file.
pub fn write(path: &Path, schemes: &ReplaceSchemes) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(schemes).map_err(io::Error::other)?;
    let partial = path.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(partial, path)
}
