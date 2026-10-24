use tokio::sync::mpsc;

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::instances::{InstanceRecord, StoreError};
use crate::java::JavaRuntime;
use crate::launching::{
    GameExit, JavaRequest, LaunchParts, LaunchRequest, LaunchSteps, LaunchUpdate, Launcher,
};
use crate::loader;
use crate::settings::validate_memory;
use crate::transfer::{SourceChain, Transport};
use crate::tuning::QuickPlay;

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
            let store = self.instances.lock().await;
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
        directories: lumilio_minecraft::LaunchDirectories,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let own_quick_play = record.settings.launch.quick_play.clone();
        let chain = self.network.chain().await?;
        let (settings, runtimes) = self.java.environment().await;
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
            let fetcher = self.network.fetcher();
            let fetched = tokio::select! {
                () = cancel.cancelled() => return Err(ServiceError::Cancelled),
                fetched = crate::minecraft::fetch_catalog(&fetcher, &chain) => fetched,
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
        let session = self
            .accounts
            .session_for(&self.appearance, &settings)
            .await?;
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
            // `None` asks for the instance's own choice; `Some(None)` for the menu.
            quick_play: quick_play.unwrap_or(own_quick_play),
        };
        Ok((request, chain))
    }

    /// Lets the next launch of `id` pick its Java automatically, once,
    /// whatever its settings say. Any launch of it uses this up.
    pub fn launch_next_with_automatic_java(&self, id: &str) {
        self.launching.automatic_java_once(id);
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
                problem.as_str().to_owned(),
            )));
        }
        self.launch_with(id, Some(Some(target)), updates, cancel)
            .await
    }

    async fn launch_with(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, ServiceError> {
        let parts = LaunchParts {
            instances: &self.instances,
            context: &self.context,
            layout: &self.layout,
            plugins: &self.plugins,
        };
        self.launching
            .launch(parts, self, id, quick_play, updates, cancel)
            .await
    }
}

impl<T: Transport + Clone> LaunchSteps for LauncherService<T> {
    type Transport = T;

    async fn request(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        self.launch_request(id, quick_play, cancel).await
    }

    async fn launcher(
        &self,
        chain: SourceChain,
    ) -> (Launcher<T>, mpsc::UnboundedReceiver<JavaRequest>) {
        let launcher = Launcher::new(self.network.transport().clone(), chain)
            .with_budget(self.network.connections().await)
            .keeping_partials(self.network.partials());
        let (sender, receiver) = mpsc::unbounded_channel();
        let launcher = if self.settings.lock().await.get().auto_install_java {
            launcher.with_java_installer(sender)
        } else {
            launcher
        };
        (launcher, receiver)
    }

    async fn install_java(
        &self,
        request: &JavaRequest,
        cancel: &CancellationToken,
    ) -> Result<JavaRuntime, String> {
        LauncherService::install_java(self, request.major, request.build, cancel.clone())
            .await
            .map_err(|error| error.to_string())
    }

    async fn apply_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        self.game_settings.apply_game_settings(id).await
    }

    async fn capture_game_settings(&self, id: &str) -> Result<(), ServiceError> {
        self.game_settings.capture_game_settings(id).await
    }
}
