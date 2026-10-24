//! Launcher-side edits of one option while the game is closed. The edit's
//! location is its scope: a game's edit never changes the preset it follows.

use super::document::Document;
use super::{
    Binding, GameSettingValue, GameSettingsError, InstanceOptions, OptionChange, OptionsFile,
    binding, catalog, document, save_binding, storage, view,
};
use crate::instance::InstanceStore;

pub(crate) fn options(
    store: &InstanceStore,
    instance: &str,
) -> Result<InstanceOptions, GameSettingsError> {
    let record = store
        .get(instance)
        .ok_or_else(|| GameSettingsError::Invalid("unknown game".into()))?;
    let mut options = InstanceOptions {
        view: view(store, instance)?,
        game_version: record.game_version.clone(),
        ..Default::default()
    };
    if !catalog::supported(&record.game_version) {
        options.file = OptionsFile::Unsupported;
        return Ok(options);
    }
    let path = store.layout().game(instance).join("options.txt");
    match document::read(&path).and_then(|text| text.map(|t| Document::parse(&t)).transpose()) {
        Ok(Some(document)) => {
            options.values = document.values(&record.game_version);
            options.file = OptionsFile::Ready;
        }
        Ok(None) => options.file = OptionsFile::Missing,
        Err(error) => options.file = OptionsFile::Unreadable(error.to_string()),
    }
    Ok(options)
}

/// Shared options become overrides that the next launch applies; every other
/// option is written to the game's own file now.
pub(crate) fn edit_option(
    store: &InstanceStore,
    instance: &str,
    key: &str,
    change: &OptionChange,
    expected: &super::GameSettingsView,
) -> Result<(), GameSettingsError> {
    if view(store, instance)? != *expected {
        return Err(GameSettingsError::Stale);
    }
    let record = store
        .get(instance)
        .ok_or_else(|| GameSettingsError::Invalid("unknown game".into()))?;
    let mut binding = binding(store, instance)?;
    if binding
        .as_ref()
        .is_some_and(|b| b.snapshot.as_ref().is_some_and(|s| s.active))
    {
        return Err(GameSettingsError::RecoveryRequired);
    }
    let shared = match &binding {
        Some(binding) => {
            let preset = storage::preset(store, binding.preset)?;
            preset.values.contains_key(key) && !preset.excluded.contains(key)
        }
        None => false,
    };
    match (change, binding.as_mut()) {
        (OptionChange::Follow, Some(binding)) if shared => {
            binding.overrides.remove(key);
            settle(binding, key);
            save_binding(store, instance, Some(&*binding))
        }
        (OptionChange::Follow, _) => Err(GameSettingsError::Invalid(
            "this option does not follow a preset".into(),
        )),
        (OptionChange::Set(value), Some(binding)) if shared => {
            if catalog::encode(key, value, &record.game_version).is_none() {
                return Err(invalid_value());
            }
            binding.overrides.insert(key.into(), value.clone());
            settle(binding, key);
            save_binding(store, instance, Some(&*binding))
        }
        (OptionChange::Set(value), _) => write(store, instance, &record.game_version, key, value),
    }
}

/// A decided field must not be rediscovered from the unchanged game file, so
/// the baseline moves to what the file holds (as resolving does).
fn settle(binding: &mut Binding, key: &str) {
    if let Some(change) = binding.pending.remove(key)
        && let Some(snapshot) = &mut binding.snapshot
    {
        snapshot.effective.insert(key.into(), change.value);
    }
}

fn write(
    store: &InstanceStore,
    instance: &str,
    version: &str,
    key: &str,
    value: &GameSettingValue,
) -> Result<(), GameSettingsError> {
    let path = store.layout().game(instance).join("options.txt");
    let text = document::read(&path)?.ok_or_else(|| {
        GameSettingsError::Invalid("launch this game once to create options.txt".into())
    })?;
    let mut document = Document::parse(&text)?;
    // As when applying a preset: only an option the file already holds is
    // known to be understood by this game.
    if !document.values(version).contains_key(key) {
        return Err(GameSettingsError::Invalid(
            "this game's options file does not hold that option".into(),
        ));
    }
    let raw = catalog::encode(key, value, version).ok_or_else(invalid_value)?;
    document.set(key, &raw);
    let output = document.text();
    if output != text {
        document::publish(&path, Some(&text), &output)?;
    }
    Ok(())
}

fn invalid_value() -> GameSettingsError {
    GameSettingsError::Invalid("value is not valid for this game version".into())
}
