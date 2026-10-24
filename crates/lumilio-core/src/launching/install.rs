use tokio::sync::mpsc;

use super::LaunchParts;
use crate::activity::CancellationToken;
use crate::activity::{RetryAction, TaskAction, TaskCategory, TaskLabel};
use crate::error::ServiceError;
use crate::history::ChangeKind;
use crate::instances::{InstanceRecord, Loader, StoreError};
use crate::launching::{LaunchServiceError, LaunchUpdate};
use crate::loader::LAUNCHABLE_LOADERS;
use crate::transfer::Transport;

/// `Fabric 0.16.0` / `Vanilla`, for a history line.
fn loader_text(loader: Loader, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{loader:?} {version}"),
        None => format!("{loader:?}"),
    }
}

impl<T: Transport + Clone> LaunchParts<'_, T> {
    /// Installs an instance's game files without starting it. The task shows
    /// in Activity and `cancel_task` can stop it. `installed` becomes true only
    /// when the whole install finished; a failure or cancel leaves it false.
    pub(crate) async fn install_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        let (request, chain) = self.request(id, Some(None), &cancel).await?;
        let task = self
            .tasks
            .begin(
                TaskCategory::Install,
                TaskLabel::Typed {
                    action: TaskAction::InstallGame,
                    subject: request.instance.name.clone(),
                },
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::InstallInstance {
                instance: id.to_owned(),
            });
        let mut result = self
            .install_serving_java(task.id, chain, request, updates, cancel)
            .await;
        if result.is_ok() {
            result = self
                .instances
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        self.tasks.end(task, &result);
        result
    }

    /// Checks every game file of an instance against its release and fetches
    /// whatever is missing or damaged again, rebuilding the extracted natives
    /// and (for Forge and NeoForge) the patched client. Files that are already
    /// right are not touched, so an intact game does nothing and needs no
    /// network. Worlds, mods and settings are never touched.
    pub(crate) async fn repair_instance(
        &self,
        id: &str,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        let (request, chain) = self.request(id, Some(None), &cancel).await?;
        let task = self
            .tasks
            .begin(
                TaskCategory::Repair,
                TaskLabel::Typed {
                    action: TaskAction::RepairGame,
                    subject: request.instance.name.clone(),
                },
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::RepairInstance {
                instance: id.to_owned(),
            });
        let mut result = self
            .install_serving_java(task.id, chain, request, updates, cancel)
            .await;
        if result.is_ok() {
            result = self
                .instances
                .lock()
                .await
                .mark_installed(id, true)
                .map_err(ServiceError::from);
        }
        if result.is_ok() {
            crate::history::record_change(
                self.layout.root(),
                id,
                ChangeKind::Repaired,
                "game files",
            );
        }
        self.tasks.end(task, &result);
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
    pub(crate) async fn change_runtime(
        &self,
        id: &str,
        game_version: &str,
        loader: Loader,
        loader_version: Option<&str>,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.instances.reserve(id)?;
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
        let current = self.instances.record(id).await?;
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
            .tasks
            .begin(
                TaskCategory::Update,
                TaskLabel::Typed {
                    action: TaskAction::ChangeVersion,
                    subject: current.name.clone(),
                },
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::ChangeRuntime {
                instance: id.to_owned(),
                game_version: game_version.to_owned(),
                loader,
                loader_version: loader_version.clone(),
            });
        let mut result = self
            .install_serving_java(task.id, chain, request, updates, cancel)
            .await;
        if result.is_ok() {
            result = self
                .instances
                .lock()
                .await
                .set_runtime(id, game_version, loader, loader_version.as_deref())
                .map_err(ServiceError::from);
        }
        self.tasks.end(task, &result);
        result?;
        crate::history::record_change(
            self.layout.root(),
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
        self.instances.record(id).await
    }
}
