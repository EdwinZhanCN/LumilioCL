//! What an instance has on disk to draw with: its mods, enabled resource
//! packs and added packs, and its installed clients (only their language
//! files are read, for names). Which overlays are switched on depends on the
//! studio folder; the service fills that in.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::order::{arrange, read_order};
use super::{InstalledClient, InstanceAssets, read_added};
use crate::instance::InstanceRecord;
use crate::layout::Layout;
use crate::release::ReleaseManifest;

/// Reads what [`super::resolve`] needs about `record`, every overlay off.
/// Blocking.
pub(crate) fn instance_assets(layout: &Layout, record: &InstanceRecord) -> InstanceAssets {
    let directories = layout.launch_directories(record);
    let client = client_jar(directories.versions(), record);
    let mut other_clients = installed_clients(directories.versions());
    other_clients.retain(|other| Some(&other.jar) != client.as_ref().map(|own| &own.jar));
    let added = read_added(&layout.schematic_packs(&record.id));
    let mut overlays = overlays(directories.game());
    for pack in &added {
        if pack.exists() && !overlays.contains(pack) {
            overlays.push(pack.clone());
        }
    }
    let overlays = arrange(overlays, &read_order(&layout.schematic_order(&record.id)));
    InstanceAssets {
        game_version: record.game_version.clone(),
        client,
        other_clients,
        overlays,
        added,
        switched_on: Vec::new(),
    }
}

fn manifest(versions: &Path, id: &str) -> Option<ReleaseManifest> {
    let text = fs::read_to_string(versions.join(id).join(format!("{id}.json"))).ok()?;
    ReleaseManifest::decode_json(&text).ok()
}

/// The vanilla client jar the installed instance plays with.
fn client_jar(versions: &Path, record: &InstanceRecord) -> Option<InstalledClient> {
    let jar_id = manifest(versions, &record.release_id()?)?
        .jar_id()
        .to_owned();
    let jar = versions.join(&jar_id).join(format!("{jar_id}.jar"));
    jar.is_file().then(|| InstalledClient {
        version: record.game_version.clone(),
        jar,
    })
}

/// Every installed client jar under `versions`, most recently changed first.
fn installed_clients(versions: &Path) -> Vec<InstalledClient> {
    let Ok(listing) = fs::read_dir(versions) else {
        return Vec::new();
    };
    let mut found: Vec<(SystemTime, InstalledClient)> = listing
        .filter_map(Result::ok)
        .filter_map(|item| {
            let id = item.file_name().to_str()?.to_owned();
            let jar = item.path().join(format!("{id}.jar"));
            let modified = fs::metadata(&jar).ok()?.modified().ok()?;
            (manifest(versions, &id)?.jar_id() == id)
                .then_some((modified, InstalledClient { version: id, jar }))
        })
        .collect();
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.version.cmp(&b.1.version)));
    found.into_iter().map(|(_, client)| client).collect()
}

/// `mods/*.jar` by name, then the resource packs `options.txt` enables, in
/// its order (lowest priority first, as the game layers them).
pub(super) fn overlays(game: &Path) -> Vec<PathBuf> {
    let mut mods: Vec<PathBuf> = fs::read_dir(game.join("mods"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|item| item.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        })
        .collect();
    mods.sort();
    let options = fs::read_to_string(game.join("options.txt")).unwrap_or_default();
    let packs = enabled_resource_packs(&options)
        .into_iter()
        .map(|name| game.join("resourcepacks").join(name))
        .filter(|path| path.exists());
    mods.into_iter().chain(packs).collect()
}

/// The `file/…` entries of `resourcePacks:[…]` in `options.txt`, without
/// the prefix. Built-in packs (`vanilla`, `fabric`, `programmer_art`) are
/// not files and are left out; names that would leave the folder too.
pub(super) fn enabled_resource_packs(options: &str) -> Vec<String> {
    options
        .lines()
        .find_map(|line| line.strip_prefix("resourcePacks:"))
        .and_then(|list| serde_json::from_str::<Vec<String>>(list.trim()).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| entry.strip_prefix("file/").map(str::to_owned))
        .filter(|name| {
            !name.is_empty() && !name.contains(['/', '\\']) && name != ".." && name != "."
        })
        .collect()
}
