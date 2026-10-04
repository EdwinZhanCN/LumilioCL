use super::LauncherService;
use super::error::ServiceError;
use super::types::now;
use crate::activity::CancellationToken;
use crate::catalog::VersionCatalog;
use crate::history::{SessionOutcome, finish_session, record_attempt};
use crate::instance::{InstanceRecord, StoreError};
use crate::launcher::{LaunchRequest, LaunchServiceError, LaunchUpdate, Launcher};
use crate::loader;
use crate::process::GameExit;
use crate::recovery::SessionMarker;
use crate::settings::validate_memory;
use crate::transfer::{SourceChain, Transport};
use crate::tuning::QuickPlay;
use tokio::sync::mpsc;

impl<T: Transport + Clone> LauncherService<T> {
    /// Everything a launch or an install needs about one instance, captured
    /// once: the record, effective settings and where to find the release.
    pub(super) async fn launch_request(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let (record, directories) = {
            let store = self.store.lock().await;
            let record = store
                .get(id)
                .cloned()
                .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
            let directories = store.directories(&record);
            (record, directories)
        };
        self.request_for(record, directories, quick_play, cancel)
            .await
    }

    /// The request to start (or install) `record`, which need not be what the
    /// library stores yet: a version change prepares its new combination
    /// before committing it.
    pub(super) async fn request_for(
        &self,
        record: InstanceRecord,
        directories: crate::launch::LaunchDirectories,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let own_quick_play = record.settings.launch.quick_play.clone();
        let chain = self.chain().await?;
        let (settings, runtimes) = self.environment().await;
        validate_memory(
            record
                .settings
                .min_memory_mb
                .or(settings.default_min_memory_mb),
            record
                .settings
                .max_memory_mb
                .or(settings.default_max_memory_mb),
        )?;

        // Releases already on disk launch without the network.
        let on_disk = |release: &str| {
            directories
                .versions()
                .join(release)
                .join(format!("{release}.json"))
                .is_file()
        };
        let vanilla_ready = on_disk(&record.game_version);
        let all_ready =
            vanilla_ready && record.release_id().is_some_and(|release| on_disk(&release));
        let manifest_url = if all_ready {
            None
        } else {
            let fetched = tokio::select! {
                () = cancel.cancelled() => return Err(ServiceError::Cancelled),
                fetched = VersionCatalog::fetch(&self.transport, &chain) => fetched,
            };
            match fetched {
                Ok(catalog) => catalog
                    .get(&record.game_version)
                    .map(|entry| entry.manifest_url().to_owned()),
                Err(_) if vanilla_ready => None,
                Err(error) => return Err(ServiceError::Remote(error.to_string())),
            }
        };
        let loader_profile_url = record.loader_version.as_deref().and_then(|version| {
            loader::profile_url(record.loader, &record.game_version, version).or_else(|| {
                crate::forge_install::installer_url(record.loader, &record.game_version, version)
            })
        });
        let session = self.session_for(&settings).await?;
        let request = LaunchRequest {
            instance: record,
            directories,
            session,
            manifest_url,
            loader_profile_url,
            runtimes,
            default_max_memory_mb: settings.default_max_memory_mb,
            default_min_memory_mb: settings.default_min_memory_mb,
            tuning: settings.launch.clone(),
            download_concurrency: settings.download_concurrency,
            // `None` asks for the instance's own choice; `Some(None)` for the menu.
            quick_play: quick_play.unwrap_or(own_quick_play),
        };
        Ok((request, chain))
    }

    /// Prepares and starts an instance, recording the session afterwards.
    pub async fn launch(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        self.launch_with(id, None, updates, cancel).await
    }

    /// Starts the game and goes straight into a saved world. The instance's
    /// own direct-start choice is not changed.
    pub async fn launch_world(
        &self,
        id: &str,
        world: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let target = QuickPlay::World(world.to_owned());
        if crate::tuning::quick_play_problem(&target).is_some() {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "the world name is not a valid save folder".to_owned(),
            )));
        }
        self.launch_with(id, Some(Some(target)), updates, cancel)
            .await
    }

    /// Starts the game and goes straight onto a multiplayer server. The
    /// instance's own direct-start choice is not changed.
    pub async fn launch_server(
        &self,
        id: &str,
        address: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let target = QuickPlay::Server(address.trim().to_owned());
        if let Some(problem) = crate::tuning::quick_play_problem(&target) {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                problem.to_owned(),
            )));
        }
        self.launch_with(id, Some(Some(target)), updates, cancel)
            .await
    }

    pub(super) async fn launch_with(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let started = now();
        if self.store.lock().await.get(id).is_none() {
            return Err(ServiceError::NoSuchInstance(id.to_owned()));
        }
        // Dropped on every way out; only the launcher dying leaves it behind.
        let _marker = SessionMarker::place(&self.layout, id, started)?;
        let prepared = self.launch_request(id, quick_play, &cancel).await;
        let (request, chain) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => return Err(self.note_attempt(id, started, error)),
        };
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let exit = match launcher.launch(request, updates, cancel).await {
            Ok(exit) => exit,
            Err(error) => return Err(self.note_attempt(id, started, error.into())),
        };
        let mut store = self.store.lock().await;
        store.mark_installed(id, true)?;
        finish_session(&mut store, id, started, &exit)?;
        Ok(exit)
    }

    /// Writes a launch that never produced a process to the instance history,
    /// then hands the error back. An unknown instance has no history to write.
    pub(super) fn note_attempt(&self, id: &str, started: u64, error: ServiceError) -> ServiceError {
        let outcome = match &error {
            ServiceError::NoSuchInstance(_) => return error,
            ServiceError::Cancelled | ServiceError::Launch(LaunchServiceError::Cancelled) => {
                SessionOutcome::Cancelled
            }
            _ => SessionOutcome::FailedToPrepare,
        };
        // History is a record, not part of the result: a write failure must
        // not replace the error the user needs to see.
        let _ = record_attempt(self.layout.root(), id, started, outcome);
        error
    }
}
