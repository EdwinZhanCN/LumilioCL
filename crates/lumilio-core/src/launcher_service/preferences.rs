use super::LauncherService;
use crate::error::ServiceError;
use crate::instances::InstanceSettings;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn settings(&self) -> crate::settings::LauncherSettings {
        self.preferences.settings().await
    }

    pub async fn after_launch_for(&self, id: &str) -> crate::tuning::AfterLaunch {
        self.preferences.after_launch_for(id).await
    }

    pub async fn current_instance(&self) -> Option<String> {
        self.preferences.current_instance().await
    }

    pub fn context(&self) -> crate::context::Context {
        self.preferences.context()
    }

    pub fn subscribe_context(&self) -> tokio::sync::watch::Receiver<crate::context::Context> {
        self.preferences.subscribe_context()
    }

    pub async fn set_current_instance(&self, id: &str) -> Result<(), ServiceError> {
        self.preferences.set_current_instance(id).await
    }

    pub async fn set_preferences(
        &self,
        preferences: crate::tuning::Preferences,
    ) -> Result<(), ServiceError> {
        self.preferences.set_preferences(preferences).await
    }

    pub async fn set_launch_defaults(
        &self,
        launch: crate::tuning::LaunchTuning,
    ) -> Result<(), ServiceError> {
        self.preferences.set_launch_defaults(launch).await
    }

    pub async fn set_download_concurrency(&self, count: Option<u32>) -> Result<(), ServiceError> {
        self.preferences.set_download_concurrency(count).await
    }

    pub async fn set_auto_update(&self, enabled: bool) -> Result<(), ServiceError> {
        self.preferences.set_auto_update(enabled).await
    }

    pub async fn set_release_notes_seen(&self, version: &str) -> Result<(), ServiceError> {
        self.preferences.set_release_notes_seen(version).await
    }

    pub async fn accept_terms(&self) -> Result<(), ServiceError> {
        self.preferences.accept_terms().await
    }

    pub async fn set_auto_download_block_assets(&self, enabled: bool) -> Result<(), ServiceError> {
        self.preferences
            .set_auto_download_block_assets(enabled)
            .await
    }

    pub async fn set_mirrors(
        &self,
        mirrors: Vec<crate::settings::MirrorRule>,
        prefer: bool,
    ) -> Result<(), ServiceError> {
        self.preferences.set_mirrors(mirrors, prefer).await
    }

    pub async fn set_mirror_rules(
        &self,
        mirrors: Vec<crate::MirrorRule>,
    ) -> Result<(), ServiceError> {
        self.preferences.set_mirror_rules(mirrors).await
    }

    pub async fn set_download_source(
        &self,
        preference: crate::DownloadSourcePreference,
    ) -> Result<(), ServiceError> {
        self.preferences.set_download_source(preference).await
    }

    pub async fn add_mirror_preset(&self, preset: crate::MirrorPreset) -> Result<(), ServiceError> {
        self.preferences.add_mirror_preset(preset).await
    }

    pub async fn set_default_memory(
        &self,
        min: Option<u32>,
        max: Option<u32>,
    ) -> Result<(), ServiceError> {
        self.preferences.set_default_memory(min, max).await
    }

    pub async fn update_instance_settings(
        &self,
        id: &str,
        settings: InstanceSettings,
    ) -> Result<(), ServiceError> {
        self.preferences
            .update_instance_settings(id, settings)
            .await
    }
}
