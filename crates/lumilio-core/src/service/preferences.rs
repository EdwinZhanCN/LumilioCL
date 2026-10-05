use super::LauncherService;
use super::error::ServiceError;
use crate::instance::InstanceSettings;
use crate::java::JavaLocator;
use crate::settings::validate_memory;
use crate::transfer::Transport;
use std::path::{Path, PathBuf};

impl<T: Transport + Clone> LauncherService<T> {
    /// What the launcher window does when this instance's game runs: the
    /// instance's own choice, else the launcher preference.
    pub async fn after_launch_for(&self, id: &str) -> crate::tuning::AfterLaunch {
        let own = self
            .store
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
    pub async fn current_instance(&self) -> Option<String> {
        let saved = self.settings.lock().await.get().current_instance.clone()?;
        let exists = self.store.lock().await.get(&saved).is_some();
        exists.then_some(saved)
    }

    /// Remembers the current instance. It must exist.
    pub async fn set_current_instance(&self, id: &str) -> Result<(), ServiceError> {
        if self.store.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        Ok(self
            .settings
            .lock()
            .await
            .set_current_instance(Some(id.to_owned()))?)
    }

    pub async fn set_preferences(
        &self,
        preferences: crate::tuning::Preferences,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_preferences(preferences)?)
    }

    pub async fn set_launch_defaults(
        &self,
        launch: crate::tuning::LaunchTuning,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_launch_defaults(launch)?)
    }

    pub async fn set_download_concurrency(&self, count: Option<u32>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_download_concurrency(count)?)
    }

    /// Turns a Java installation off (or back on) by its home folder.
    pub async fn set_java_disabled(&self, home: &Path, disabled: bool) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        let mut homes = settings.get().disabled_java.clone();
        homes.retain(|saved| saved != home);
        if disabled {
            homes.push(home.to_owned());
        }
        Ok(settings.set_disabled_java(homes)?)
    }

    /// Adds the Java the person chose: its `java` executable, its home
    /// folder, or a folder of installations. The folder joins the extra
    /// search folders so it is found again later.
    pub async fn add_java(&self, chosen: &Path) -> Result<crate::java::JavaRuntime, ServiceError> {
        let mut candidates = vec![chosen.to_owned()];
        if chosen.is_file()
            && let Some(home) = chosen.parent().and_then(Path::parent)
        {
            candidates.insert(0, home.to_owned());
        }
        for candidate in candidates {
            let probe = candidate.clone();
            let found = tokio::task::spawn_blocking(move || JavaLocator::new([probe]).discover())
                .await
                .unwrap_or_default();
            let Some(runtime) = found.into_iter().next() else {
                continue;
            };
            let mut settings = self.settings.lock().await;
            let mut roots = settings.get().extra_java_roots.clone();
            if !roots.contains(&candidate) {
                roots.push(candidate);
                settings.set_java_roots(roots)?;
            }
            return Ok(runtime);
        }
        Err(ServiceError::NoJavaAt(chosen.to_owned()))
    }

    pub async fn set_java_roots(&self, roots: Vec<PathBuf>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_java_roots(roots)?)
    }

    pub async fn set_mirrors(
        &self,
        mirrors: Vec<crate::settings::MirrorRule>,
        prefer: bool,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        settings.set_mirrors(mirrors, prefer)?;
        self.plugins.set_sources(settings.source_chain()?);
        Ok(())
    }

    pub async fn set_default_memory(
        &self,
        min: Option<u32>,
        max: Option<u32>,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_memory(min, max)?)
    }

    /// Saves overrides after checking their effective combination with current defaults.
    /// `None` clears an override and resumes inheritance.
    pub async fn update_instance_settings(
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
        Ok(self.store.lock().await.update_settings(id, settings)?)
    }
}
