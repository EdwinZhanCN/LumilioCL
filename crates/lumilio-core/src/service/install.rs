use super::error::ServiceError;
use super::{LauncherService, loader_text};
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskCategory};
use crate::history::ChangeKind;
use crate::instance::{InstanceRecord, Loader, StoreError};
use crate::launcher::{LaunchServiceError, LaunchUpdate, Launcher};
use crate::loader::LAUNCHABLE_LOADERS;
use crate::transfer::Transport;
use tokio::sync::mpsc;

impl<T: Transport + Clone> LauncherService<T> {
    /// Installs an instance's game files without starting it. The task shows
    /// in Activity and `cancel_task` can stop it. `installed` becomes true only
    /// when the whole install finished; a failure or cancel leaves it false.
    pub async fn install_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let (request, chain) = self.launch_request(id, Some(None), &cancel).await?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("安装 {}", request.instance.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::InstallInstance {
                instance: id.to_owned(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        self.end(task, &result);
        result
    }

    /// Checks every game file of an instance against its release and fetches
    /// whatever is missing or damaged again, rebuilding the extracted natives
    /// and (for Forge and NeoForge) the patched client. Files that are already
    /// right are not touched, so an intact game does nothing and needs no
    /// network. Worlds, mods and settings are never touched.
    pub async fn repair_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let (request, chain) = self.launch_request(id, Some(None), &cancel).await?;
        let task = self
            .begin(
                TaskCategory::Repair,
                format!("修复 {}", request.instance.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::RepairInstance {
                instance: id.to_owned(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        if result.is_ok() {
            self.record_change(id, ChangeKind::Repaired, "game files");
        }
        self.end(task, &result);
        result
    }

    /// Changes the game version and/or the loader of an instance.
    ///
    /// The new combination is prepared first: its shared files are fetched
    /// into `meta/` under their own release folder (nothing another instance
    /// uses is rewritten) and a Forge or NeoForge installer runs. Only when
    /// that has fully succeeded is the instance's record changed, in one
    /// library transaction. Any failure or cancel before the commit leaves
    /// the old combination exactly as it was and launchable; a failed commit
    /// does too. Worlds, mods and settings are not touched: mods built for
    /// another game version or loader may no longer work, which is why the
    /// caller offers a snapshot first. Refused while the instance launches or
    /// has another write in progress.
    pub async fn change_runtime(
        &self,
        id: &str,
        game_version: &str,
        loader: Loader,
        loader_version: Option<&str>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        if !LAUNCHABLE_LOADERS.contains(&loader) {
            return Err(ServiceError::Launch(LaunchServiceError::LoaderUnsupported(
                loader,
            )));
        }
        let game_version = game_version.trim();
        let loader_version = if loader == Loader::Vanilla {
            None
        } else {
            Some(
                loader_version
                    .map(str::trim)
                    .filter(|version| !version.is_empty())
                    .ok_or(ServiceError::Launch(
                        LaunchServiceError::LoaderVersionMissing,
                    ))?
                    .to_owned(),
            )
        };
        if game_version.is_empty() {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "the game version is empty".to_owned(),
            )));
        }
        let current = self.instance(id).await?;
        if current.game_version == game_version
            && current.loader == loader
            && current.loader_version == loader_version
        {
            return Err(ServiceError::Store(StoreError::InvalidLaunch(
                "that is already this game's version and loader".to_owned(),
            )));
        }
        let mut candidate = current.clone();
        candidate.game_version = game_version.to_owned();
        candidate.loader = loader;
        candidate.loader_version = loader_version.clone();
        let directories = self.layout.launch_directories(&candidate);
        let (request, chain) = self
            .request_for(candidate, directories, Some(None), &cancel)
            .await?;
        let task = self
            .begin(
                TaskCategory::Update,
                format!("更换 {} 的版本", current.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::ChangeRuntime {
                instance: id.to_owned(),
                game_version: game_version.to_owned(),
                loader,
                loader_version: loader_version.clone(),
            });
        let launcher = Launcher::new(self.transport.clone(), chain)
            .with_concurrency(request.download_concurrency);
        let mut result =
            Self::install_tracked(&self.board, task.id, &launcher, request, updates, cancel).await;
        if result.is_ok() {
            result = self
                .store
                .lock()
                .await
                .set_runtime(id, game_version, loader, loader_version.as_deref())
                .map_err(ServiceError::from);
        }
        self.end(task, &result);
        result?;
        self.record_change(
            id,
            ChangeKind::GameVersionChanged,
            &format!(
                "{} {} → {} {}",
                current.game_version,
                loader_text(current.loader, current.loader_version.as_deref()),
                game_version,
                loader_text(loader, loader_version.as_deref()),
            ),
        );
        self.instance(id).await
    }
}
