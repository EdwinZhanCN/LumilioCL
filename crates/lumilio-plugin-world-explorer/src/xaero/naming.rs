//! Folder and file naming of Xaero's Minimap saves, restricted to what the
//! waypoint overlay needs.
//!
//! Adapted from XaeroTools crates/xaero-core/src/naming.rs (MIT, Copyright (c) 2026 Dek), ADR 0022.
//! Reworked onto `lumilio_plugin_api::map::Dimension`.

use lumilio_plugin_api::map::Dimension;

/// Directory, below the game directory, holding the Minimap's per-world data.
pub(crate) const MINIMAP_ROOT: &str = "xaero/minimap";

/// Resource id -> folder segment (`:` -> `$`, `/` -> `%`).
pub(crate) fn escape_folder_id(id: &str) -> String {
    id.replace(':', "$").replace('/', "%")
}

/// Folder segment -> resource id. Only the first `$` is the namespace
/// separator; every `%` was a `/`.
pub(crate) fn unescape_folder_id(folder: &str) -> String {
    let with_slashes = folder.replace('%', "/");
    match with_slashes.split_once('$') {
        Some((namespace, path)) => format!("{namespace}:{path}"),
        None => with_slashes,
    }
}

/// A minimap dimension folder (`dim%0`, `dim%-1`, `dim%1`, `dim%<escaped id>`),
/// including the older names the mod still maps on load.
pub(crate) fn dimension_of(folder: &str) -> Option<Dimension> {
    match folder {
        "Overworld" => return Some(Dimension::Overworld),
        "Nether" => return Some(Dimension::Nether),
        "The End" => return Some(Dimension::End),
        _ => {}
    }
    Some(match folder.strip_prefix("dim%")? {
        "0" => Dimension::Overworld,
        "-1" => Dimension::Nether,
        "1" => Dimension::End,
        other => Dimension::Custom(unescape_folder_id(other)),
    })
}

/// The folder name the mod writes for a dimension.
pub(crate) fn folder_of(dimension: &Dimension) -> String {
    match dimension {
        Dimension::Overworld => "dim%0".into(),
        Dimension::Nether => "dim%-1".into(),
        Dimension::End => "dim%1".into(),
        Dimension::Custom(id) => format!("dim%{}", escape_folder_id(id)),
    }
}

/// The minimap's own waypoint backups: `backup`, `backup-`, `backup--`, ... The
/// mod skips them when loading, and so must we: they hold waypoints the player
/// may since have deleted.
pub(crate) fn is_backup_dir(name: &str) -> bool {
    name.strip_prefix("backup")
        .is_some_and(|rest| rest.bytes().all(|byte| byte == b'-'))
}

/// Files that are mid-write, superseded or written by something other than
/// the mod, and are never live waypoint data.
pub(crate) fn is_transient(name: &str) -> bool {
    name.ends_with(".temp")
        || name.ends_with(".outdated")
        || name == ".lock"
        || name.contains(".sync-conflict-")
        || name
            .rsplit_once(".backup")
            .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()))
}

/// A waypoint file name, `<multiworldId>_<displayName>.txt` with `_` in the
/// display name stored as `%us%`, plus the legacy `waypoints.txt` that carries
/// no multiworld id. Returns the multiworld id and the display name.
pub(crate) fn parse_file_name(name: &str) -> Option<(Option<&str>, String)> {
    let (stem, extension) = name.rsplit_once('.')?;
    if extension != "txt" {
        return None;
    }
    if stem == "waypoints" {
        return Some((None, stem.to_owned()));
    }
    let mut parts = stem.split('_');
    let multiworld = parts.next()?;
    let display = parts.next()?;
    Some((Some(multiworld), display.replace("%us%", "_")))
}

/// Whether a path below the game directory is a live waypoint file of the
/// world directory `world`, and which dimension folder it sits in.
pub(crate) fn waypoint_file<'a>(path: &'a str, world: &str) -> Option<(Dimension, &'a str)> {
    let rest = path
        .strip_prefix(MINIMAP_ROOT)?
        .strip_prefix('/')?
        .strip_prefix(world)?
        .strip_prefix('/')?;
    let mut parts: Vec<&str> = rest.split('/').collect();
    let file = parts.pop()?;
    let [folder] = parts.as_slice() else {
        return None;
    };
    if is_backup_dir(folder) || is_transient(file) || !file.starts_with("mw$") {
        return None;
    }
    parse_file_name(file)?;
    Some((dimension_of(folder)?, file))
}
