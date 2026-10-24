use super::LauncherService;
use crate::error::ServiceError;
use crate::transfer::Transport;
use crate::{
    GameSettingGroup, GameSettingValue, GameSettingsView, InstanceOptions, OptionChange,
    SettingResolution, SettingsPreset,
};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn settings_presets(&self) -> Result<Vec<SettingsPreset>, ServiceError> {
        self.game_settings.settings_presets().await
    }

    pub async fn game_settings(&self, id: &str) -> Result<GameSettingsView, ServiceError> {
        self.game_settings.game_settings(id).await
    }

    pub async fn all_game_settings(&self) -> Result<Vec<GameSettingsView>, ServiceError> {
        self.game_settings.all_game_settings().await
    }

    pub async fn game_options(&self, id: &str) -> Result<InstanceOptions, ServiceError> {
        self.game_settings.game_options(id).await
    }

    pub async fn edit_game_option(
        &self,
        expected: GameSettingsView,
        key: String,
        change: OptionChange,
    ) -> Result<(), ServiceError> {
        self.game_settings
            .edit_game_option(expected, key, change)
            .await
    }

    pub async fn create_settings_preset(
        &self,
        source: &str,
        name: String,
        groups: &[GameSettingGroup],
    ) -> Result<i64, ServiceError> {
        self.game_settings
            .create_settings_preset(source, name, groups)
            .await
    }

    pub async fn edit_settings_preset(&self, preset: SettingsPreset) -> Result<(), ServiceError> {
        self.game_settings.edit_settings_preset(preset).await
    }

    pub async fn set_preset_value(
        &self,
        id: i64,
        key: String,
        before: GameSettingValue,
        value: GameSettingValue,
    ) -> Result<(), ServiceError> {
        self.game_settings
            .set_preset_value(id, key, before, value)
            .await
    }

    pub async fn copy_settings_preset(&self, id: i64, name: String) -> Result<i64, ServiceError> {
        self.game_settings.copy_settings_preset(id, name).await
    }

    pub async fn delete_settings_preset(&self, id: i64) -> Result<(), ServiceError> {
        self.game_settings.delete_settings_preset(id).await
    }

    pub async fn bind_settings_preset(
        &self,
        id: &str,
        preset: Option<i64>,
    ) -> Result<(), ServiceError> {
        self.game_settings.bind_settings_preset(id, preset).await
    }

    pub async fn resolve_game_settings(
        &self,
        expected: GameSettingsView,
        selected: Vec<String>,
        resolution: SettingResolution,
    ) -> Result<(), ServiceError> {
        self.game_settings
            .resolve_game_settings(expected, selected, resolution)
            .await
    }

    pub async fn recover_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        self.game_settings.recover_game_settings(id).await
    }
}
