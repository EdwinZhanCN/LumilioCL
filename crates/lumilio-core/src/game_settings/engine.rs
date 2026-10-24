use super::document::Document;
use super::{
    GameSettingsError, GameSettingsView, SettingChange, SettingResolution, Snapshot, Values,
    binding, catalog, document, save_binding, storage,
};
use crate::instances::InstanceStore;

pub(crate) fn view(
    store: &InstanceStore,
    instance: &str,
) -> Result<GameSettingsView, GameSettingsError> {
    let mut view = GameSettingsView {
        instance: instance.into(),
        ..Default::default()
    };
    if let Some(binding) = binding(store, instance)? {
        view.preset = Some(storage::preset(store, binding.preset)?);
        view.overrides = binding.overrides;
        if let Some(preset) = &view.preset {
            view.overrides
                .retain(|key, _| !preset.excluded.contains(key));
        }
        view.pending = binding.pending.into_values().collect();
        view.skipped = binding.skipped;
        view.recovery_required = binding.snapshot.as_ref().is_some_and(|s| s.active);
        view.issue = binding.issue;
    }
    Ok(view)
}

/// Compare against what this game actually received, never against the newest preset.
pub(crate) fn capture(
    store: &InstanceStore,
    instance: &str,
    closed: bool,
) -> Result<(), GameSettingsError> {
    let Some(mut binding) = binding(store, instance)? else {
        return Ok(());
    };
    let Some(snapshot) = binding.snapshot.clone() else {
        return Ok(());
    };
    if snapshot.active && !closed {
        return Err(GameSettingsError::RecoveryRequired);
    }
    let path = store.layout().game(instance).join("options.txt");
    let result = (|| {
        let text = document::read(&path)?
            .ok_or_else(|| GameSettingsError::Invalid("options.txt was removed".into()))?;
        if !snapshot.published && text != snapshot.applied {
            if Some(&text) == snapshot.original.as_ref() {
                binding.snapshot = None;
                return Ok(());
            }
            return Err(GameSettingsError::Invalid(
                "interrupted publication needs review; original and current options differ".into(),
            ));
        }
        let values = Document::parse(&text)?.values(&snapshot.game_version);
        for (key, before) in &snapshot.effective {
            let Some(value) = values.get(key) else {
                continue;
            };
            if let Some(pending) = binding.pending.get_mut(key) {
                // Retain the original shared comparison point over repeated launches.
                pending.value = value.clone();
                if pending.value == pending.before {
                    binding.pending.remove(key);
                }
            } else if value != before
                && let Some(shared) = snapshot.shared.get(key)
            {
                binding.pending.insert(
                    key.clone(),
                    SettingChange {
                        setting: key.clone(),
                        before: before.clone(),
                        value: value.clone(),
                        shared_before: shared.clone(),
                    },
                );
            }
        }
        Ok(())
    })();
    if let Some(snapshot) = &mut binding.snapshot {
        snapshot.active = false;
    }
    binding.issue = result.as_ref().err().map(ToString::to_string);
    save_binding(store, instance, Some(&binding))?;
    result
}

pub(crate) fn apply(store: &InstanceStore, instance: &str) -> Result<(), GameSettingsError> {
    if binding(store, instance)?.is_none() {
        return Ok(());
    }
    capture(store, instance, false)?;
    let mut binding = binding(store, instance)?.expect("binding checked under store lock");
    let record = store
        .get(instance)
        .ok_or_else(|| GameSettingsError::Invalid("unknown game".into()))?;
    let preset = storage::preset(store, binding.preset)?;
    let path = store.layout().game(instance).join("options.txt");
    let original = document::read(&path)?;
    binding.skipped.clear();
    binding.issue = None;
    // Let Minecraft initialize a version-marked options document. Never inject modern
    // key tokens into a versionless file that its DataFixer will interpret as legacy.
    let Some(text) = original.as_ref() else {
        binding.skipped = preset.values.keys().cloned().collect();
        save_binding(store, instance, Some(&binding))?;
        return Ok(());
    };
    let mut document = Document::parse(text)?;
    let mut effective = Values::new();
    let mut desired = preset.values.clone();
    desired.retain(|key, _| !preset.excluded.contains(key));
    desired.extend(binding.overrides.clone());
    desired.extend(
        binding
            .pending
            .iter()
            .map(|(key, p)| (key.clone(), p.value.clone())),
    );
    let existing = document.values(&record.game_version);
    for (key, value) in &desired {
        if !preset.values.contains_key(key) || preset.excluded.contains(key) {
            continue;
        }
        // Existing recognized keys prove the target file understands this setting.
        if existing.contains_key(key)
            && let Some(raw) = catalog::encode(key, value, &record.game_version)
        {
            document.set(key, &raw);
            effective.insert(key.clone(), value.clone());
        } else {
            binding.skipped.push(key.clone());
        }
    }
    let output = document.text();
    binding.snapshot = Some(Snapshot {
        game_version: record.game_version.clone(),
        preset_revision: preset.revision,
        shared: preset.values,
        effective,
        original: original.clone(),
        applied: output.clone(),
        active: true,
        published: false,
    });
    // A durable intent plus original bytes is written before touching the game file.
    save_binding(store, instance, Some(&binding))?;
    if output != *text
        && let Err(error) = document::publish(&path, Some(text), &output)
    {
        // Keep the journal but do not infer that an unobserved process has exited.
        binding.issue = Some(error.to_string());
        save_binding(store, instance, Some(&binding))?;
        return Err(error);
    }
    if let Some(snapshot) = &mut binding.snapshot {
        snapshot.published = true;
    }
    save_binding(store, instance, Some(&binding))?;
    Ok(())
}

/// All selected changes commit together. A conflict leaves every selection untouched.
pub(crate) fn resolve(
    store: &InstanceStore,
    instance: &str,
    selected: &[String],
    resolution: SettingResolution,
    expected: &GameSettingsView,
) -> Result<(), GameSettingsError> {
    let mut binding = binding(store, instance)?
        .ok_or_else(|| GameSettingsError::Invalid("game is independently managed".into()))?;
    if binding.snapshot.as_ref().is_some_and(|s| s.active) {
        return Err(GameSettingsError::RecoveryRequired);
    }
    if view(store, instance)? != *expected {
        return Err(GameSettingsError::Stale);
    }
    let mut preset = storage::preset(store, binding.preset)?;
    if selected.iter().any(|key| preset.excluded.contains(key)) {
        return Err(GameSettingsError::Invalid(
            "selected field is no longer shared".into(),
        ));
    }
    let changes: Vec<_> = selected
        .iter()
        .map(|key| {
            binding
                .pending
                .get(key)
                .cloned()
                .or_else(|| {
                    let value = binding.overrides.get(key)?.clone();
                    let shared = preset.values.get(key)?.clone();
                    Some(SettingChange {
                        setting: key.clone(),
                        before: shared.clone(),
                        value,
                        shared_before: shared,
                    })
                })
                .ok_or_else(|| {
                    GameSettingsError::Invalid(
                        "selected setting is no longer pending or overridden".into(),
                    )
                })
        })
        .collect::<Result<_, _>>()?;
    if changes.is_empty() {
        return Err(GameSettingsError::Invalid("no settings selected".into()));
    }
    if resolution == SettingResolution::Shared {
        let conflicts: Vec<_> = changes
            .iter()
            .filter(|c| {
                preset
                    .values
                    .get(&c.setting)
                    .is_none_or(|p| *p != c.shared_before && *p != c.value)
            })
            .map(|c| c.setting.clone())
            .collect();
        if !conflicts.is_empty() {
            return Err(GameSettingsError::Conflict(conflicts));
        }
    }
    for change in &changes {
        match resolution {
            SettingResolution::Instance => {
                binding
                    .overrides
                    .insert(change.setting.clone(), change.value.clone());
            }
            SettingResolution::Shared | SettingResolution::ReplaceShared => {
                preset
                    .values
                    .insert(change.setting.clone(), change.value.clone());
                binding.overrides.remove(&change.setting);
            }
            SettingResolution::Restore => {
                binding.overrides.remove(&change.setting);
            }
        }
        binding.pending.remove(&change.setting);
    }
    // Rebase resolved fields so the unchanged game file cannot be rediscovered as a new
    // change before the next launch. The requested value is materialized at next launch.
    if let Some(snapshot) = &mut binding.snapshot {
        for change in &changes {
            snapshot
                .effective
                .insert(change.setting.clone(), change.value.clone());
        }
    }
    let tx = store.settings_database().unchecked_transaction()?;
    if matches!(
        resolution,
        SettingResolution::Shared | SettingResolution::ReplaceShared
    ) {
        storage::save_preset(store, &preset)?;
    }
    save_binding(store, instance, Some(&binding))?;
    tx.commit()?;
    Ok(())
}
