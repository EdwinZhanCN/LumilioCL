use super::{LauncherService, ServiceError};
use crate::plugins::PluginInfo;
use crate::transfer::Transport;
use lumilio_plugin_api::{PluginState, SettingValue};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<(), ServiceError> {
        self.change_plugin(id, move |state| state.enabled = Some(enabled))
            .await
    }

    /// A field save merges with the latest preferences, preserving other fields.
    pub async fn set_plugin_value(
        &self,
        id: &str,
        key: &str,
        value: SettingValue,
    ) -> Result<(), ServiceError> {
        self.change_plugin(id, |state| {
            state.values.insert(key.to_owned(), value);
        })
        .await
    }

    pub async fn reset_plugin(&self, id: &str) -> Result<(), ServiceError> {
        self.set_plugin(id, PluginState::default()).await
    }

    async fn change_plugin(
        &self,
        id: &str,
        change: impl FnOnce(&mut PluginState),
    ) -> Result<(), ServiceError> {
        // Registration can call plugin code; finish it before taking the store lock.
        self.plugins.list().await;
        let mut settings = self.settings.lock().await;
        let mut state = settings.get().plugins.get(id).cloned().unwrap_or_default();
        change(&mut state);
        self.plugins
            .validate_state(id, &state)
            .await
            .map_err(ServiceError::Plugin)?;
        settings.set_plugin(id.to_owned(), state.clone())?;
        self.plugins.set_state(id.to_owned(), state);
        Ok(())
    }
    pub async fn plugins(&self) -> Vec<PluginInfo> {
        self.plugins.list().await
    }

    /// Saves one plugin's preference group atomically, then publishes it to
    /// subsequent calls. An empty state restores the manifest's defaults.
    pub async fn set_plugin(&self, id: &str, state: PluginState) -> Result<(), ServiceError> {
        self.plugins
            .validate_state(id, &state)
            .await
            .map_err(ServiceError::Plugin)?;
        let mut settings = self.settings.lock().await;
        settings.set_plugin(id.to_owned(), state.clone())?;
        self.plugins.set_state(id.to_owned(), state);
        Ok(())
    }
}
