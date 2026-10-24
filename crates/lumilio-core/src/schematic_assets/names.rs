//! The game's names for blocks, items and entities when the bundle's
//! `names.dat.z` cannot be had (plan schematic-studio, T26/D28): the
//! installed games' own language files. English is in each client jar;
//! the other languages are asset objects the launcher downloaded with the
//! game, found through the version's asset index.

use std::fs;
use std::io::Read as _;
use std::path::Path;

use lumilio_block_assets::{BlockNames, NAME_LANGUAGES, names_in_lang_file};

use super::InstalledClient;
use lumilio_minecraft::ReleaseManifest;

/// The names in `clients`' language files, the first client's first:
/// pass the instance's own client before the others. `assets` is the
/// launcher's asset folder (`indexes/`, `objects/`). Blocking.
#[must_use]
pub fn installed_names(clients: &[&InstalledClient], assets: &Path) -> BlockNames {
    let mut names = BlockNames::default();
    for client in clients {
        for language in NAME_LANGUAGES {
            let file = if language == "en_us" {
                jar_entry(&client.jar, "assets/minecraft/lang/en_us.json")
            } else {
                asset_object(
                    &client.jar,
                    assets,
                    &format!("minecraft/lang/{language}.json"),
                )
            };
            if let Some(found) = file.and_then(|bytes| names_in_lang_file(&bytes).ok()) {
                names.add(language, found);
            }
        }
    }
    names
}

fn jar_entry(jar: &Path, name: &str) -> Option<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(fs::File::open(jar).ok()?).ok()?;
    let mut entry = archive.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// The asset object `name` of the version whose client is `jar`: its
/// manifest sits beside the jar and names the asset index.
fn asset_object(jar: &Path, assets: &Path, name: &str) -> Option<Vec<u8>> {
    let manifest =
        ReleaseManifest::decode_json(&fs::read_to_string(jar.with_extension("json")).ok()?).ok()?;
    let id = manifest.asset_catalog()?.id().to_owned();
    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(assets.join("indexes").join(format!("{id}.json"))).ok()?)
            .ok()?;
    let hash = index["objects"][name]["hash"].as_str()?;
    if hash.len() < 3 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    fs::read(assets.join("objects").join(&hash[..2]).join(hash)).ok()
}
