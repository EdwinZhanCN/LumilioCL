use lumilio_plugin_api::{ActionId, PluginState, SettingValue, View};

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::ServiceError;
use crate::instances::Instances;
use crate::java::JavaRuntime;
use crate::plugins::{PluginEffect, PluginHost, PluginInfo, PluginTab};
use crate::settings::SettingsStore;
use crate::storage::Layout;

/// Which plugins run, their saved settings, and the tabs they add to a game.
pub(crate) struct PluginControls {
    plugins: Arc<PluginHost>,
    settings: Arc<Mutex<SettingsStore>>,
    instances: Arc<Instances>,
    layout: Layout,
}

impl PluginControls {
    #[cfg(test)]
    pub(crate) fn replace_plugins(&mut self, plugins: Arc<PluginHost>) {
        self.plugins = plugins;
    }

    pub(crate) fn new(
        plugins: Arc<PluginHost>,
        settings: Arc<Mutex<SettingsStore>>,
        instances: Arc<Instances>,
        layout: Layout,
    ) -> Self {
        Self {
            plugins,
            settings,
            instances,
            layout,
        }
    }

    pub(crate) async fn set_plugin_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<(), ServiceError> {
        self.change_plugin(id, move |state| state.enabled = Some(enabled))
            .await
    }

    /// A field save merges with the latest preferences, preserving other fields.
    pub(crate) async fn set_plugin_value(
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

    pub(crate) async fn reset_plugin(&self, id: &str) -> Result<(), ServiceError> {
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
    pub(crate) async fn plugins(&self) -> Vec<PluginInfo> {
        self.plugins.list().await
    }

    /// Tells every plugin context which language the launcher speaks from
    /// now on, so plugin text follows a language switch.
    pub(crate) async fn set_plugin_locale(&self, tag: &str) {
        self.plugins.set_locale(tag);
    }

    /// Saves one plugin's preference group atomically, then publishes it to
    /// subsequent calls. An empty state restores the manifest's defaults.
    pub(crate) async fn set_plugin(
        &self,
        id: &str,
        state: PluginState,
    ) -> Result<(), ServiceError> {
        self.plugins
            .validate_state(id, &state)
            .await
            .map_err(ServiceError::Plugin)?;
        let mut settings = self.settings.lock().await;
        settings.set_plugin(id.to_owned(), state.clone())?;
        self.plugins.set_state(id.to_owned(), state);
        Ok(())
    }

    /// The plugin tabs that show for this instance, which can run on
    /// `runtimes`.
    pub(crate) async fn plugin_tabs(
        &self,
        instance: &str,
        runtimes: Vec<JavaRuntime>,
    ) -> Result<Vec<PluginTab>, ServiceError> {
        let record = self.instances.record(instance).await?;
        let layout = self.layout.clone();
        let game_dir = self.layout.game(instance);
        let game = tokio::task::spawn_blocking(move || {
            crate::diagnostics::game_facts(&layout, &record, &runtimes)
        })
        .await
        .map_err(std::io::Error::other)?;
        Ok(self.plugins.tabs(game_dir, game).await)
    }

    /// What one plugin's tab shows; `None` if the plugin is off or has failed.
    pub(crate) async fn plugin_view(
        &self,
        instance: &str,
        plugin: &str,
    ) -> Result<Option<View>, ServiceError> {
        self.instances.record(instance).await?;
        Ok(self
            .plugins
            .tab_view(instance, self.layout.game(instance), plugin)
            .await)
    }

    /// Runs one action of a plugin's tab; the interface performs the returned
    /// effects. `None` if the plugin is off or has failed.
    pub(crate) async fn plugin_action(
        &self,
        instance: &str,
        plugin: &str,
        action: ActionId,
    ) -> Result<Option<Vec<PluginEffect>>, ServiceError> {
        self.instances.record(instance).await?;
        Ok(self
            .plugins
            .tab_action(instance, self.layout.game(instance), plugin, action)
            .await)
    }
}
