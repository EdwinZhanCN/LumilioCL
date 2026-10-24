use super::{LauncherService, ServiceError};
use crate::game_settings::Binding;
use crate::transfer::Transport;
use crate::{
    GameSettingGroup, GameSettingValue, GameSettingsError, GameSettingsView, InstanceOptions,
    OptionChange, SettingResolution, SettingsPreset, game_setting_group, game_settings,
};

impl<T: Transport + Clone> LauncherService<T> {
    /// A game that is starting, running or otherwise leased cannot be edited,
    /// and its unfinished session is not an interrupted one.
    fn mark_busy(&self, view: &mut GameSettingsView) {
        view.is_busy = self.instances.is_busy(&view.instance);
        if view.is_busy {
            view.recovery_required = false;
        }
    }
    pub async fn settings_presets(&self) -> Result<Vec<SettingsPreset>, ServiceError> {
        Ok(game_settings::list(&*self.instances.lock().await)?)
    }
    pub async fn game_settings(&self, id: &str) -> Result<GameSettingsView, ServiceError> {
        let store = self.instances.lock().await;
        if store.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.into()));
        }
        let mut view = game_settings::view(&store, id)?;
        self.mark_busy(&mut view);
        Ok(view)
    }
    pub async fn all_game_settings(&self) -> Result<Vec<GameSettingsView>, ServiceError> {
        let store = self.instances.lock().await;
        store
            .instances()
            .iter()
            .map(|r| {
                let mut view = game_settings::view(&store, &r.id)?;
                self.mark_busy(&mut view);
                Ok(view)
            })
            .collect()
    }
    /// The game's own options file, read for editing, with its preset state.
    pub async fn game_options(&self, id: &str) -> Result<InstanceOptions, ServiceError> {
        let store = self.instances.lock().await;
        if store.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.into()));
        }
        let mut options = game_settings::options(&store, id)?;
        self.mark_busy(&mut options.view);
        Ok(options)
    }
    /// Edits one option of a closed game. `expected` is the state the edit
    /// was decided on; a change since (including one made in the game files)
    /// is reported as stale instead of being overwritten.
    pub async fn edit_game_option(
        &self,
        expected: GameSettingsView,
        key: String,
        change: OptionChange,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(&expected.instance)?;
        let store = self.instances.lock().await;
        game_settings::capture(&store, &expected.instance, false)?;
        Ok(game_settings::edit_option(
            &store,
            &expected.instance,
            &key,
            &change,
            &expected,
        )?)
    }
    pub async fn create_settings_preset(
        &self,
        source: &str,
        name: String,
        groups: &[GameSettingGroup],
    ) -> Result<i64, ServiceError> {
        let _lease = self.instances.reserve(source)?;
        let store = self.instances.lock().await;
        let record = store
            .get(source)
            .ok_or_else(|| ServiceError::NoSuchInstance(source.into()))?;
        let values = game_settings::read_values(
            &self.layout.game(source).join("options.txt"),
            &record.game_version,
        )?;
        let excluded = values
            .keys()
            .filter(|key| !groups.contains(&game_setting_group(key)))
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        if values.len() == excluded.len() {
            return Err(GameSettingsError::Invalid(
                "no supported preferences in the selected groups".into(),
            )
            .into());
        }
        let mut preset = game_settings::new_preset(name, values);
        preset.excluded = excluded;
        Ok(game_settings::save_preset(&store, &preset)?)
    }
    pub async fn edit_settings_preset(&self, preset: SettingsPreset) -> Result<(), ServiceError> {
        let store = self.instances.lock().await;
        let original = game_settings::preset(&store, preset.id)?;
        // Scope changes never delete values. An edited value must be a known,
        // normalized value of an option the preset already holds; games
        // receive it at their next launch, and pending game changes compare
        // against it when they are shared.
        if !preset.values.keys().eq(original.values.keys()) {
            return Err(GameSettingsError::Invalid(
                "options cannot be added to or removed from a preset".into(),
            )
            .into());
        }
        if preset
            .values
            .iter()
            .any(|(key, value)| original.values[key] != *value && !game_settings::valid(key, value))
        {
            return Err(
                GameSettingsError::Invalid("value is not valid for this option".into()).into(),
            );
        }
        let mut bindings = Vec::new();
        if preset.excluded != original.excluded {
            for record in store.instances() {
                if let Some(mut binding) = game_settings::binding(&store, &record.id)?
                    && binding.preset == preset.id
                {
                    if binding.snapshot.as_ref().is_some_and(|s| s.active)
                        || binding
                            .pending
                            .keys()
                            .any(|key| preset.excluded.contains(key))
                    {
                        return Err(GameSettingsError::Invalid("close bound games and resolve affected pending changes before changing sharing scope".into()).into());
                    }
                    if let Some(snapshot) = &mut binding.snapshot {
                        snapshot
                            .effective
                            .retain(|key, _| !preset.excluded.contains(key));
                    }
                    bindings.push((record.id.clone(), binding));
                }
            }
        }
        let tx = store
            .settings_database()
            .unchecked_transaction()
            .map_err(GameSettingsError::from)?;
        game_settings::save_preset(&store, &preset)?;
        for (id, binding) in bindings {
            game_settings::save_binding(&store, &id, Some(&binding))?;
        }
        tx.commit().map_err(GameSettingsError::from)?;
        Ok(())
    }
    /// Changes one option of a preset, applied to its latest revision: other
    /// options may have changed since the page read it, this one may not.
    pub async fn set_preset_value(
        &self,
        id: i64,
        key: String,
        before: GameSettingValue,
        value: GameSettingValue,
    ) -> Result<(), ServiceError> {
        let mut preset = game_settings::preset(&*self.instances.lock().await, id)?;
        if preset.values.get(&key) != Some(&before) {
            return Err(GameSettingsError::Stale.into());
        }
        preset.values.insert(key, value);
        self.edit_settings_preset(preset).await
    }
    pub async fn copy_settings_preset(&self, id: i64, name: String) -> Result<i64, ServiceError> {
        let store = self.instances.lock().await;
        let mut preset = game_settings::preset(&store, id)?;
        preset.id = 0;
        preset.name = name;
        Ok(game_settings::save_preset(&store, &preset)?)
    }
    pub async fn delete_settings_preset(&self, id: i64) -> Result<(), ServiceError> {
        Ok(game_settings::delete(&*self.instances.lock().await, id)?)
    }
    pub async fn bind_settings_preset(
        &self,
        id: &str,
        preset: Option<i64>,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        let store = self.instances.lock().await;
        if store.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.into()));
        }
        let previous = game_settings::binding(&store, id)?;
        if previous
            .as_ref()
            .is_some_and(|b| b.snapshot.as_ref().is_some_and(|s| s.active))
        {
            return Err(GameSettingsError::RecoveryRequired.into());
        }
        if previous.as_ref().is_some_and(|b| !b.pending.is_empty()) && preset.is_some() {
            return Err(GameSettingsError::Invalid(
                "resolve pending changes before switching preset".into(),
            )
            .into());
        }
        if let Some(preset) = preset {
            game_settings::preset(&store, preset)?;
            if previous.as_ref().is_some_and(|b| b.preset == preset) {
                return Ok(());
            }
            game_settings::save_binding(
                &store,
                id,
                Some(&Binding {
                    preset,
                    ..Default::default()
                }),
            )?;
        } else {
            game_settings::save_binding(&store, id, None)?;
        }
        Ok(())
    }
    pub async fn resolve_game_settings(
        &self,
        expected: GameSettingsView,
        selected: Vec<String>,
        resolution: SettingResolution,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(&expected.instance)?;
        let store = self.instances.lock().await;
        // Detect edits made since the list opened, then require the UI to review them again.
        game_settings::capture(&store, &expected.instance, false)?;
        Ok(game_settings::resolve(
            &store,
            &expected.instance,
            &selected,
            resolution,
            &expected,
        )?)
    }
    /// Explicit user confirmation is needed after an interrupted launcher session;
    /// we cannot safely infer that an orphan game has stopped.
    pub async fn recover_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        Ok(game_settings::capture(
            &*self.instances.lock().await,
            id,
            true,
        )?)
    }
    pub(super) async fn apply_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        let store = self.instances.lock().await;
        let result = game_settings::apply(&store, id);
        if let Err(error) = &result
            && let Ok(Some(mut binding)) = game_settings::binding(&store, id)
        {
            binding.issue = Some(error.to_string());
            let _ = game_settings::save_binding(&store, id, Some(&binding));
        }
        Ok(result?)
    }
    pub(super) async fn capture_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        Ok(game_settings::capture(
            &*self.instances.lock().await,
            id,
            true,
        )?)
    }
}
