use lumilio_plugin_api::{ActionId, PluginState, SettingValue, View};

use super::LauncherService;
use crate::error::ServiceError;
use crate::plugins::{PluginEffect, PluginInfo, PluginTab};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), ServiceError> {
        self.plugin_controls.set_plugin_enabled(id, enabled).await
    }

    pub async fn set_plugin_value(
        &self,
        id: &str,
        key: &str,
        value: SettingValue,
    ) -> Result<(), ServiceError> {
        self.plugin_controls.set_plugin_value(id, key, value).await
    }

    pub async fn reset_plugin(&self, id: &str) -> Result<(), ServiceError> {
        self.plugin_controls.reset_plugin(id).await
    }

    pub async fn plugins(&self) -> Vec<PluginInfo> {
        self.plugin_controls.plugins().await
    }

    pub async fn set_plugin_locale(&self, tag: &str) {
        self.plugin_controls.set_plugin_locale(tag).await
    }

    pub async fn set_plugin(&self, id: &str, state: PluginState) -> Result<(), ServiceError> {
        self.plugin_controls.set_plugin(id, state).await
    }

    pub async fn plugin_tabs(&self, instance: &str) -> Result<Vec<PluginTab>, ServiceError> {
        let (_, runtimes) = self.java.environment().await;
        self.plugin_controls.plugin_tabs(instance, runtimes).await
    }

    pub async fn plugin_view(
        &self,
        instance: &str,
        plugin: &str,
    ) -> Result<Option<View>, ServiceError> {
        self.plugin_controls.plugin_view(instance, plugin).await
    }

    pub async fn plugin_action(
        &self,
        instance: &str,
        plugin: &str,
        action: ActionId,
    ) -> Result<Option<Vec<PluginEffect>>, ServiceError> {
        self.plugin_controls
            .plugin_action(instance, plugin, action)
            .await
    }
}
