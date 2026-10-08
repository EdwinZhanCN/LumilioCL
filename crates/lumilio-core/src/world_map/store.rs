//! Manual seeds share launcher.db but survive the library's table replacement.
use super::WorldMapContext;
use lumilio_plugin_api::map::{Dimension, SeedSource, SourceLink, WorldContext, WorldId};
use rusqlite::{Connection, params};
use std::path::Path;

pub fn parse_seed(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(seed) = text.parse() {
        return Some(seed);
    }
    // Java String.hashCode operates on UTF-16 code units, including surrogates.
    Some(i64::from(text.encode_utf16().fold(0_i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })))
}
pub fn save(path: &Path, instance: &str, seed: i64, version: &str) -> rusqlite::Result<()> {
    let db = Connection::open(path)?;
    db.execute("INSERT OR IGNORE INTO world_map_seeds (instance,seed,version) SELECT ?1,?2,?3 WHERE EXISTS (SELECT 1 FROM instances WHERE id = ?1)",params![instance,seed,version])?;
    Ok(())
}
pub fn load(path: &Path, instance: &str) -> rusqlite::Result<Vec<WorldMapContext>> {
    let db = Connection::open(path)?;
    let mut query =
        db.prepare("SELECT seed,version FROM world_map_seeds WHERE instance=?1 ORDER BY rowid")?;
    query
        .query_map([instance], |row| {
            let seed: i64 = row.get(0)?;
            let version: String = row.get(1)?;
            Ok(WorldMapContext {
                name: format!("{seed} ({version})"),
                spawn: None,
                context: WorldContext {
                    world: WorldId::Seed {
                        seed,
                        version: version.clone(),
                    },
                    seed: Some(seed),
                    version: Some(version),
                    data_version: None,
                    dimension: Dimension::Overworld,
                    sources: vec![SourceLink::Seed(SeedSource::Manual)],
                },
            })
        })?
        .collect()
}

#[cfg(test)]
mod tests;
