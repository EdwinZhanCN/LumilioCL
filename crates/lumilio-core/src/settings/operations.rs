use std::sync::Arc;

use tokio::sync::Mutex;

use super::SettingsStore;
use crate::context::ContextHub;
use crate::error::ServiceError;
use crate::instances::InstanceSettings;
use crate::instances::Instances;
use crate::network::Network;
use crate::plugins::PluginHost;
use crate::settings::validate_memory;
use crate::transfer::Transport;
use crate::translation::SharedTranslation;

/// The launcher's own settings and the current game: the one place they are
/// saved from, with what each change must also reach (download connections,
/// plugin sources, cached translations).
pub(crate) struct Settings<T> {
    settings: Arc<Mutex<SettingsStore>>,
    context: ContextHub,
    instances: Arc<Instances>,
    network: Network<T>,
    plugins: Arc<PluginHost>,
    translation: SharedTranslation,
}

impl<T> Settings<T> {
    pub(crate) fn new(
        settings: Arc<Mutex<SettingsStore>>,
        context: ContextHub,
        instances: Arc<Instances>,
        network: Network<T>,
        plugins: Arc<PluginHost>,
        translation: SharedTranslation,
    ) -> Self {
        Self {
            settings,
            context,
            instances,
            network,
            plugins,
            translation,
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_plugins(&mut self, plugins: Arc<PluginHost>) {
        self.plugins = plugins;
    }
}

impl<T: Transport + Clone> Settings<T> {
    /// The last successfully saved launcher defaults and public account settings.
    pub(crate) async fn settings(&self) -> crate::settings::LauncherSettings {
        self.settings.lock().await.get().clone()
    }

    /// What the launcher window does when this instance's game runs: the
    /// instance's own choice, else the launcher preference.
    pub(crate) async fn after_launch_for(&self, id: &str) -> crate::tuning::AfterLaunch {
        let own = self
            .instances
            .lock()
            .await
            .get(id)
            .and_then(|record| record.settings.launch.after_launch);
        match own {
            Some(own) => own,
            None => self.settings.lock().await.get().preferences.after_launch,
        }
    }

    /// The instance the launcher plays by default. A saved id whose instance
    /// is gone reads as `None`; the caller picks another and saves it.
    pub(crate) async fn current_instance(&self) -> Option<String> {
        let saved = self.settings.lock().await.get().current_instance.clone()?;
        let exists = self.instances.lock().await.get(&saved).is_some();
        exists.then_some(saved)
    }

    /// The current game and account, and how often shared state has changed
    /// (plan global-context).
    pub(crate) fn context(&self) -> crate::context::Context {
        self.context.get()
    }

    /// Every later change of [`Self::context`]; only the newest is kept.
    pub(crate) fn subscribe_context(
        &self,
    ) -> tokio::sync::watch::Receiver<crate::context::Context> {
        self.context.subscribe()
    }

    /// Remembers the current instance. It must exist.
    pub(crate) async fn set_current_instance(&self, id: &str) -> Result<(), ServiceError> {
        if self.instances.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        Ok(self
            .settings
            .lock()
            .await
            .set_current_instance(Some(id.to_owned()))?)
    }

    pub(crate) async fn set_preferences(
        &self,
        preferences: crate::tuning::Preferences,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        let changed = settings.get().preferences.translation != preferences.translation;
        settings.set_preferences(preferences)?;
        if changed {
            self.translation.invalidate();
        }
        Ok(())
    }

    pub(crate) async fn set_launch_defaults(
        &self,
        launch: crate::tuning::LaunchTuning,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_launch_defaults(launch)?)
    }

    pub(crate) async fn set_download_concurrency(
        &self,
        count: Option<u32>,
    ) -> Result<(), ServiceError> {
        self.settings.lock().await.set_download_concurrency(count)?;
        // Running downloads take the new size from their next connection.
        self.network.connections().await;
        Ok(())
    }

    pub(crate) async fn set_auto_update(&self, enabled: bool) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_auto_update(enabled)?)
    }

    /// Remembers that the release notes of `version` were shown.
    pub(crate) async fn set_release_notes_seen(&self, version: &str) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_release_notes_seen(version)?)
    }

    /// Remembers that the person agreed to the current terms.
    pub(crate) async fn accept_terms(&self) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.accept_terms()?)
    }

    /// Turns the automatic block-asset download of schematic previews on or off.
    pub(crate) async fn set_auto_download_block_assets(
        &self,
        enabled: bool,
    ) -> Result<(), ServiceError> {
        Ok(self
            .settings
            .lock()
            .await
            .set_auto_download_block_assets(enabled)?)
    }

    pub(crate) async fn set_mirrors(
        &self,
        mirrors: Vec<crate::settings::MirrorRule>,
        prefer: bool,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        settings.set_mirrors(mirrors, prefer)?;
        self.plugins.set_sources(settings.source_chain()?);
        Ok(())
    }

    pub(crate) async fn set_mirror_rules(
        &self,
        mirrors: Vec<crate::MirrorRule>,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        settings.set_mirror_rules(mirrors)?;
        self.plugins.set_sources(settings.source_chain()?);
        Ok(())
    }

    pub(crate) async fn set_download_source(
        &self,
        preference: crate::DownloadSourcePreference,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        settings.set_download_source(preference)?;
        self.plugins.set_sources(settings.source_chain()?);
        Ok(())
    }

    /// Merge against the latest saved settings, so concurrent preset additions
    /// cannot replace one another with stale UI snapshots.
    pub(crate) async fn add_mirror_preset(
        &self,
        preset: crate::MirrorPreset,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        let mirrors = preset.merge(&settings.get().mirrors);
        settings.set_mirror_rules(mirrors)?;
        self.plugins.set_sources(settings.source_chain()?);
        Ok(())
    }

    pub(crate) async fn set_default_memory(
        &self,
        min: Option<u32>,
        max: Option<u32>,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_memory(min, max)?)
    }

    /// Saves overrides after checking their effective combination with current defaults.
    /// `None` clears an override and resumes inheritance.
    pub(crate) async fn update_instance_settings(
        &self,
        id: &str,
        settings: InstanceSettings,
    ) -> Result<(), ServiceError> {
        let defaults = self.settings.lock().await;
        if let Some(java) = settings.java_path.as_deref()
            && !java.exists()
        {
            return Err(ServiceError::NoJavaAt(java.to_owned()));
        }
        validate_memory(settings.min_memory_mb, settings.max_memory_mb)?;
        validate_memory(
            settings
                .min_memory_mb
                .or(defaults.get().default_min_memory_mb),
            settings
                .max_memory_mb
                .or(defaults.get().default_max_memory_mb),
        )?;
        Ok(self.instances.lock().await.update_settings(id, settings)?)
    }
}
