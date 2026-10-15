//! Which overlays a studio folder draws with (plan schematic-studio, T21,
//! D23): the instance's mods, enabled resource packs and added packs are all
//! listed, but each is off until switched on. The switches are kept in
//! launcher data (`Layout::schematic_sources`), per instance and folder, as
//! the paths switched on.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Switches {
    /// Studio folder (`schematics`) → the sources switched on.
    #[serde(default)]
    folders: BTreeMap<String, Vec<PathBuf>>,
}

fn read(list: &Path) -> Switches {
    fs::read(list)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn write(list: &Path, switches: &Switches) -> io::Result<()> {
    if let Some(parent) = list.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(switches).map_err(io::Error::other)?;
    let partial = list.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(partial, list)
}

/// The sources switched on for the studio folder that holds `file` (a path
/// below the game folder, `schematics/farms/a.litematic`); the deepest such
/// folder, should two nest. None when nothing was switched on there, or the
/// list cannot be read.
pub(crate) fn switched_on(list: &Path, file: &str) -> Vec<PathBuf> {
    read(list)
        .folders
        .into_iter()
        .filter(|(folder, _)| holds(folder, file))
        .max_by_key(|(folder, _)| folder.len())
        .map(|(_, on)| on)
        .unwrap_or_default()
}

/// Whether `file` lies below the studio folder `folder`.
fn holds(folder: &str, file: &str) -> bool {
    let folder = folder.trim_end_matches('/');
    !folder.is_empty()
        && file
            .strip_prefix(folder)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Switches `source` on or off for `folder`.
pub(crate) fn switch(list: &Path, folder: &str, source: &Path, on: bool) -> io::Result<()> {
    let mut switches = read(list);
    let entry = switches.folders.entry(folder.to_owned()).or_default();
    if !on {
        entry.retain(|known| known != source);
    } else if !entry.iter().any(|known| known == source) {
        entry.push(source.to_owned());
    }
    if entry.is_empty() {
        switches.folders.remove(folder);
    }
    write(list, &switches)
}

/// Switches `source` off in every folder: it was taken off the sources.
pub(crate) fn forget(list: &Path, source: &Path) -> io::Result<()> {
    let mut switches = read(list);
    for on in switches.folders.values_mut() {
        on.retain(|known| known != source);
    }
    switches.folders.retain(|_, on| !on.is_empty());
    write(list, &switches)
}
