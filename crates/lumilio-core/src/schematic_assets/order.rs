//! The order an instance's studio layers its mods and resource packs in
//! (plan schematic-studio, T36): a list of paths in launcher data
//! (`Layout::schematic_order`), lowest priority first, written whole each
//! time a source is moved. Without one the default order holds: mods by
//! name, the game's enabled packs as `options.txt` lists them, then the
//! packs added in the studio.
//!
//! The order is part of an overlay bake's key ([`super::LocalCache`]) and of
//! a thumbnail's look, so a reorder never reuses a bake or picture made in
//! another order.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The kept order; empty when none was kept or it cannot be read.
pub(crate) fn read_order(list: &Path) -> Vec<PathBuf> {
    fs::read(list)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub(crate) fn write_order(list: &Path, order: &[PathBuf]) -> io::Result<()> {
    if let Some(parent) = list.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(order).map_err(io::Error::other)?;
    let partial = list.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(partial, list)
}

/// `overlays` (in the default order) in `order`: those the order names in
/// its order, after those it does not name, which keep their default order
/// at the bottom. A mod installed or a pack enabled since the order was kept
/// starts there, below everything placed by hand.
#[must_use]
pub(crate) fn arrange(overlays: Vec<PathBuf>, order: &[PathBuf]) -> Vec<PathBuf> {
    let (mut placed, unplaced): (Vec<PathBuf>, Vec<PathBuf>) =
        overlays.into_iter().partition(|path| order.contains(path));
    placed.sort_by_key(|path| order.iter().position(|known| known == path));
    unplaced.into_iter().chain(placed).collect()
}

/// `overlays` with `source` and `other` trading places: the studio moves a
/// row past its neighbour in the list it shows, which leaves out mods
/// without assets. `None` unless both are there and differ.
#[must_use]
pub(crate) fn swapped(overlays: &[PathBuf], source: &Path, other: &Path) -> Option<Vec<PathBuf>> {
    let at = overlays.iter().position(|path| path == source)?;
    let to = overlays.iter().position(|path| path == other)?;
    if at == to {
        return None;
    }
    let mut order = overlays.to_vec();
    order.swap(at, to);
    Some(order)
}
