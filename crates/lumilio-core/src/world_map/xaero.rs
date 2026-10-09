//! Which Xaero Minimap directories belong to which world (plan W1). Only a
//! person confirms a link: a single-player directory with the same name as a
//! save is suggested, never attached, because a renamed save or a copied one
//! would otherwise show another world's waypoints.
use super::WorldMapContext;
use lumilio_plugin_api::map::{Dimension, SourceLink, WorldContext, WorldId};
use std::collections::BTreeMap;
use std::path::Path;

/// Where the Minimap keeps its per-world directories, relative to the game.
const ROOT: &str = "xaero/minimap";
/// Multiplayer directories are named after the server address.
const SERVER_PREFIX: &str = "Multiplayer_";

/// The Minimap world directories on disk, sorted. Links and files that are not
/// directories, hidden entries and the mod's own `backup` directories are left
/// out.
pub fn minimap_dirs(game_dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut checked = game_dir.to_path_buf();
    for part in ROOT.split('/') {
        checked.push(part);
        if std::fs::symlink_metadata(&checked).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return found;
        }
    }
    let Ok(entries) = std::fs::read_dir(&checked) else {
        return found;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let backup = name
            .strip_prefix("backup")
            .is_some_and(|rest| rest.bytes().all(|byte| byte == b'-'));
        if kind.is_dir() && !name.starts_with('.') && !backup {
            found.push(name);
        }
    }
    found.sort();
    found
}

/// Adds what the Minimap directories offer to the worlds: confirmed links
/// become sources, a same-named directory becomes a suggestion, and a
/// multiplayer directory no save is linked to becomes a world of its own.
pub fn attach(
    contexts: &mut Vec<WorldMapContext>,
    instance: &str,
    dirs: &[String],
    links: &BTreeMap<String, String>,
) {
    let mut used: Vec<&str> = Vec::new();
    for world in contexts.iter_mut() {
        let WorldId::Save { folder, .. } = &world.context.world else {
            continue;
        };
        match links.get(folder).filter(|dir| dirs.contains(dir)) {
            Some(dir) => {
                world
                    .context
                    .sources
                    .push(SourceLink::XaeroMinimap(dir.clone()));
                used.push(dir);
            }
            None => {
                world.suggested_xaero = dirs
                    .iter()
                    .find(|dir| *dir == folder && !dir.starts_with(SERVER_PREFIX))
                    .cloned();
            }
        }
    }
    let taken: Vec<String> = used.iter().map(|dir| (*dir).to_owned()).collect();
    contexts.extend(
        dirs.iter()
            .filter(|dir| dir.starts_with(SERVER_PREFIX) && !taken.contains(dir))
            .map(|dir| WorldMapContext {
                name: dir[SERVER_PREFIX.len()..].to_owned(),
                spawn: None,
                suggested_xaero: None,
                context: WorldContext {
                    world: WorldId::Server {
                        instance: instance.into(),
                        address: dir[SERVER_PREFIX.len()..].to_owned(),
                    },
                    version: None,
                    data_version: None,
                    seed: None,
                    dimension: Dimension::Overworld,
                    sources: vec![SourceLink::XaeroMinimap(dir.clone())],
                },
            }),
    );
}
