use std::collections::BTreeSet;

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::{Binding, GameSettingsError, SettingsPreset, Values};
use crate::instances::InstanceStore;

#[derive(Deserialize, Serialize)]
struct PresetFields {
    values: Values,
    excluded: BTreeSet<String>,
}

pub(crate) fn list(store: &InstanceStore) -> Result<Vec<SettingsPreset>, GameSettingsError> {
    let mut statement = store
        .settings_database()
        .prepare("SELECT id,name,revision,settings FROM game_settings_presets ORDER BY name,id")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, u32>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (id, name, revision, text) = row?;
        let fields: PresetFields =
            serde_json::from_str(&text).map_err(|e| GameSettingsError::Invalid(e.to_string()))?;
        Ok(SettingsPreset {
            id,
            name,
            revision,
            values: fields.values,
            excluded: fields.excluded,
        })
    })
    .collect()
}
pub(crate) fn save_preset(
    store: &InstanceStore,
    preset: &SettingsPreset,
) -> Result<i64, GameSettingsError> {
    let name = preset.name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(GameSettingsError::Invalid(
            "preset name must contain 1 to 80 printable characters".into(),
        ));
    }
    if preset
        .excluded
        .iter()
        .any(|key| !preset.values.contains_key(key))
    {
        return Err(GameSettingsError::Invalid(
            "unknown excluded setting".into(),
        ));
    }
    if preset.values.len() == preset.excluded.len() {
        return Err(GameSettingsError::Invalid(
            "select at least one shared setting".into(),
        ));
    }
    let text = serde_json::to_string(&PresetFields {
        values: preset.values.clone(),
        excluded: preset.excluded.clone(),
    })
    .map_err(|e| GameSettingsError::Invalid(e.to_string()))?;
    let db = store.settings_database();
    let duplicate = list(store)?
        .iter()
        .any(|p| p.id != preset.id && p.name.to_lowercase() == name.to_lowercase());
    if duplicate {
        return Err(GameSettingsError::Invalid(
            "preset name is already used".into(),
        ));
    }
    if preset.id == 0 {
        db.execute(
            "INSERT INTO game_settings_presets(name,settings) VALUES(?1,?2)",
            params![name, text],
        )?;
        Ok(db.last_insert_rowid())
    } else {
        let changed = db.execute("UPDATE game_settings_presets SET name=?1,settings=?2,revision=revision+1 WHERE id=?3 AND revision=?4",params![name,text,preset.id,preset.revision])?;
        if changed == 0 {
            return Err(GameSettingsError::Stale);
        }
        Ok(preset.id)
    }
}
pub(crate) fn binding(
    store: &InstanceStore,
    instance: &str,
) -> Result<Option<Binding>, GameSettingsError> {
    let text: Option<String> = store
        .settings_database()
        .query_row(
            "SELECT state FROM game_settings_bindings WHERE instance=?1",
            [instance],
            |row| row.get(0),
        )
        .optional()?;
    text.map(|text| {
        serde_json::from_str(&text).map_err(|e| GameSettingsError::Invalid(e.to_string()))
    })
    .transpose()
}
pub(crate) fn save_binding(
    store: &InstanceStore,
    instance: &str,
    binding: Option<&Binding>,
) -> Result<(), GameSettingsError> {
    if let Some(binding) = binding {
        let text = serde_json::to_string(binding)
            .map_err(|e| GameSettingsError::Invalid(e.to_string()))?;
        store.settings_database().execute("INSERT INTO game_settings_bindings(instance,preset,state) VALUES(?1,?2,?3) ON CONFLICT(instance) DO UPDATE SET preset=excluded.preset,state=excluded.state",params![instance,binding.preset,text])?;
    } else {
        store.settings_database().execute(
            "DELETE FROM game_settings_bindings WHERE instance=?1",
            [instance],
        )?;
    }
    Ok(())
}
pub(crate) fn delete(store: &InstanceStore, id: i64) -> Result<(), GameSettingsError> {
    let db = store.settings_database();
    let used: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM game_settings_bindings WHERE preset=?1)",
        [id],
        |r| r.get(0),
    )?;
    if used {
        return Err(GameSettingsError::Invalid(
            "preset is bound to a game".into(),
        ));
    }
    if db.execute("DELETE FROM game_settings_presets WHERE id=?1", [id])? == 0 {
        return Err(GameSettingsError::Invalid("unknown preset".into()));
    }
    Ok(())
}
pub(crate) fn preset(store: &InstanceStore, id: i64) -> Result<SettingsPreset, GameSettingsError> {
    list(store)?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| GameSettingsError::Invalid("unknown preset".into()))
}
pub(crate) fn new_preset(name: String, values: Values) -> SettingsPreset {
    SettingsPreset {
        id: 0,
        name,
        revision: 1,
        values,
        excluded: BTreeSet::new(),
    }
}
