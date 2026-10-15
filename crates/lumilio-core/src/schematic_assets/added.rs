//! Resource packs added to an instance's schematic studio from elsewhere
//! (plan schematic-studio, T17): a list of paths in launcher data
//! (`Layout::schematic_packs`). The files stay where they are; they join the
//! instance's overlays, after its mods and enabled packs, while they exist.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The packs added for an instance, in the order they were added. A list
/// that cannot be read is empty.
pub(crate) fn read_added(list: &Path) -> Vec<PathBuf> {
    fs::read(list)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Adds the resource-pack zip `pack` to the list; adding one twice keeps one.
pub(crate) fn add(list: &Path, pack: &Path) -> io::Result<Vec<PathBuf>> {
    let zip = pack
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"));
    if !pack.is_absolute() || !zip || !pack.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a resource-pack zip", pack.display()),
        ));
    }
    let mut packs = read_added(list);
    if !packs.iter().any(|known| known == pack) {
        packs.push(pack.to_owned());
    }
    write(list, &packs)?;
    Ok(packs)
}

/// Takes `pack` off the list; the file itself is left alone.
pub(crate) fn remove(list: &Path, pack: &Path) -> io::Result<Vec<PathBuf>> {
    let mut packs = read_added(list);
    packs.retain(|known| known != pack);
    write(list, &packs)?;
    Ok(packs)
}

fn write(list: &Path, packs: &[PathBuf]) -> io::Result<()> {
    if let Some(parent) = list.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(packs).map_err(io::Error::other)?;
    let partial = list.with_extension("partial");
    fs::write(&partial, bytes)?;
    fs::rename(partial, list)
}
