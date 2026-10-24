use std::fs;
use std::path::PathBuf;

use rusqlite::params;

use super::{
    Collection, Index, InstanceRecord, InstanceSettings, InstanceStore, StoreError, int, unsigned,
};
use crate::storage::SCHEMA_VERSION;
use crate::storage::persist;

impl InstanceStore {
    /// Reads the tables into memory; a fresh database first imports the
    /// legacy JSON library when one exists.
    pub(super) fn load(&mut self) -> Result<(), StoreError> {
        let version: u32 = self
            .db
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 0 {
            let legacy = self.layout.legacy_library();
            if legacy.exists() {
                let loaded = persist::load::<Index>(&legacy, 1)?;
                self.recovered |= loaded.recovered;
                if self.preserved.is_none() {
                    self.preserved = loaded.preserved;
                }
                self.index = loaded.value;
            }
            self.save(self.index.clone())?;
            if legacy.exists() && !self.recovered {
                let mut done = legacy.clone().into_os_string();
                done.push(".imported");
                fs::rename(&legacy, PathBuf::from(done))?;
            }
            return Ok(());
        }
        self.index = self.read()?;
        Ok(())
    }

    pub(super) fn read(&self) -> Result<Index, StoreError> {
        let mut index = Index::default();
        let mut statement = self.db.prepare(
            "SELECT id, name, game_version, loader, loader_version, favorite, created_at,
                    last_played, play_seconds, installed, java_path, max_memory_mb,
                    min_memory_mb, jvm_arguments, tuning, source_project
             FROM instances ORDER BY position",
        )?;
        let rows = statement.query_map([], |row| {
            let loader: String = row.get(3)?;
            let java_path: Option<String> = row.get(10)?;
            let jvm: String = row.get(13)?;
            let tuning: Option<String> = row.get(14)?;
            Ok(InstanceRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                game_version: row.get(2)?,
                loader: serde_json::from_value(serde_json::Value::String(loader))
                    .unwrap_or_default(),
                loader_version: row.get(4)?,
                favorite: row.get(5)?,
                created_at: unsigned(row.get(6)?),
                last_played: row.get::<_, Option<i64>>(7)?.map(unsigned),
                play_seconds: unsigned(row.get(8)?),
                installed: row.get(9)?,
                settings: InstanceSettings {
                    java_path: java_path.map(PathBuf::from),
                    max_memory_mb: row.get(11)?,
                    min_memory_mb: row.get(12)?,
                    jvm_arguments: serde_json::from_str(&jvm).unwrap_or_default(),
                    launch: tuning
                        .and_then(|text| serde_json::from_str(&text).ok())
                        .unwrap_or_default(),
                },
                source_project: row.get(15)?,
            })
        })?;
        for row in rows {
            index.instances.push(row?);
        }
        let mut statement = self
            .db
            .prepare("SELECT name FROM collections ORDER BY position")?;
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for name in names {
            let mut statement = self.db.prepare(
                "SELECT instance FROM collection_members WHERE collection = ?1 ORDER BY position",
            )?;
            let members = statement
                .query_map([&name], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            index.collections.push(Collection { name, members });
        }
        Ok(index)
    }

    /// Replaces the tables in one transaction, so a crash leaves either the
    /// old or the new library, never a torn one.
    pub(super) fn save(&mut self, next: Index) -> Result<(), StoreError> {
        let transaction = self.db.transaction()?;
        transaction.execute_batch(
            "DELETE FROM instances; DELETE FROM collections; DELETE FROM collection_members;",
        )?;
        for (position, record) in next.instances.iter().enumerate() {
            let loader = serde_json::to_value(record.loader)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            let jvm = serde_json::to_string(&record.settings.jvm_arguments)
                .map_err(|error| StoreError::Encode(error.to_string()))?;
            let tuning = if record.settings.launch.is_default() {
                None
            } else {
                Some(
                    serde_json::to_string(&record.settings.launch)
                        .map_err(|error| StoreError::Encode(error.to_string()))?,
                )
            };
            transaction.execute(
                "INSERT INTO instances VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                params![
                    int(position),
                    record.id,
                    record.name,
                    record.game_version,
                    loader,
                    record.loader_version,
                    record.favorite,
                    int(record.created_at),
                    record.last_played.map(int),
                    int(record.play_seconds),
                    record.installed,
                    record
                        .settings
                        .java_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    record.settings.max_memory_mb,
                    record.settings.min_memory_mb,
                    jvm,
                    tuning,
                    record.source_project,
                ],
            )?;
        }
        for (position, collection) in next.collections.iter().enumerate() {
            transaction.execute(
                "INSERT INTO collections VALUES (?1, ?2)",
                params![int(position), collection.name],
            )?;
            for (at, member) in collection.members.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO collection_members VALUES (?1, ?2, ?3)",
                    params![collection.name, int(at), member],
                )?;
            }
        }
        transaction.execute(
            "DELETE FROM world_map_seeds WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM world_map_links WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM world_map_annotations WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.execute(
            "DELETE FROM game_settings_bindings WHERE instance NOT IN (SELECT id FROM instances)",
            [],
        )?;
        transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        transaction.commit()?;
        self.index = next;
        if let Some(hub) = &self.hub {
            hub.bump(crate::context::Change::Library);
        }
        Ok(())
    }
}
