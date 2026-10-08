use crate::{nbt, worlds};
use lumilio_plugin_api::map::{Dimension, SeedSource, SourceLink, WorldContext, WorldId};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct WorldMapContext {
    pub name: String,
    pub context: WorldContext,
    pub spawn: Option<[i64; 3]>,
}

fn read(path: &Path, base: &Path) -> Option<nbt::Tag> {
    let relative = path.strip_prefix(base).ok()?;
    let mut checked = base.to_owned();
    for part in relative.components() {
        checked.push(part);
        if fs::symlink_metadata(&checked)
            .ok()?
            .file_type()
            .is_symlink()
        {
            return None;
        }
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    nbt::parse_maybe_gzip(&bytes).ok()
}

pub fn contexts(
    instance: &str,
    game_dir: &Path,
) -> Result<Vec<WorldMapContext>, worlds::WorldError> {
    Ok(worlds::scan(game_dir)?
        .into_iter()
        .map(|world| {
            let dir = game_dir.join("saves").join(&world.folder);
            let level = read(&dir.join("level.dat"), game_dir);
            let field = |path: &[&str]| level.as_ref().and_then(|root| root.at(path));
            let mut seed = field(&["Data", "WorldGenSettings", "seed"])
                .or_else(|| field(&["Data", "RandomSeed"]))
                .and_then(nbt::Tag::as_i64);
            let mut sources = vec![SourceLink::Save(world.folder.clone())];
            if seed.is_some() {
                sources.push(SourceLink::Seed(SeedSource::LevelDat));
            } else if let Some(split) =
                read(&dir.join("data/minecraft/world_gen_settings.dat"), game_dir)
            {
                seed = split
                    .get("seed")
                    .or_else(|| split.at(&["data", "seed"]))
                    .and_then(nbt::Tag::as_i64);
                if seed.is_some() {
                    sources.push(SourceLink::Seed(SeedSource::WorldGenSettings));
                }
            }
            let spawn = ["SpawnX", "SpawnY", "SpawnZ"]
                .map(|key| field(&["Data", key]).and_then(nbt::Tag::as_i64));
            WorldMapContext {
                name: world.name,
                spawn: match spawn {
                    [Some(x), Some(y), Some(z)] => Some([x, y, z]),
                    _ => None,
                },
                context: WorldContext {
                    world: WorldId::Save {
                        instance: instance.into(),
                        folder: world.folder,
                    },
                    version: world.game_version,
                    data_version: field(&["Data", "DataVersion"])
                        .and_then(nbt::Tag::as_i64)
                        .and_then(|n| i32::try_from(n).ok()),
                    seed,
                    dimension: Dimension::Overworld,
                    sources,
                },
            }
        })
        .collect())
}

pub(super) fn io_error(error: impl ToString) -> io::Error {
    io::Error::other(error.to_string())
}
