use super::LauncherService;
use super::error::ServiceError;
use super::packs::backup_failure;
use super::support::blocking_value;
use super::types::now;
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskCategory};
use crate::instance::{InstanceRecord, InstanceSettings, Loader, NewInstance};
use crate::staged::Staged;
use crate::transfer::Transport;
use std::path::Path;

impl<T: Transport + Clone> LauncherService<T> {
    /// The games in a folder made by another launcher (ADR 0016). Reads only.
    pub async fn detect_games(
        &self,
        folder: &Path,
    ) -> Result<Vec<crate::import_game::FoundGame>, ServiceError> {
        let folder = folder.to_owned();
        blocking_value(move || crate::import_game::detect(&folder))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// Makes a new game of one found by [`Self::detect_games`]: its player
    /// files are copied (the source is never changed), and its game files are
    /// installed on first launch. Built whole before it appears.
    pub async fn import_game(
        &self,
        game: crate::import_game::FoundGame,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let task = self.begin(
            TaskCategory::Install,
            format!("导入 {}", game.name),
            None,
            Some(&cancel),
        );
        let result = self.import_game_inner(game, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    pub(super) async fn import_game_inner(
        &self,
        game: crate::import_game::FoundGame,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        if game.loader != Loader::Vanilla
            && game.loader_version.as_deref().is_none_or(str::is_empty)
        {
            return Err(ServiceError::Install(
                "the game's loader version could not be told".to_owned(),
            ));
        }
        let id = self.store.lock().await.suggest_id(&game.name)?;
        let staged = Staged::begin(&self.layout, "import", &id)?;
        let (source, into, token) = (game.clone(), staged.game_dir(), cancel.clone());
        let copied =
            blocking_value(move || crate::import_game::copy_game_files(&source, &into, &token))
                .await
                .and_then(|result| {
                    result.map_err(|error| match error {
                        crate::import_game::ImportError::Io(io)
                            if io.kind() == std::io::ErrorKind::Interrupted =>
                        {
                            ServiceError::Cancelled
                        }
                        other => ServiceError::Install(other.to_string()),
                    })
                });
        if let Err(error) = copied {
            staged.discard();
            return Err(error);
        }
        if let Err(error) = staged.mark_ready() {
            staged.discard();
            return Err(error.into());
        }
        let created = self
            .store
            .lock()
            .await
            .create_as(
                &id,
                NewInstance {
                    name: game.name.clone(),
                    game_version: game.game_version.clone(),
                    loader: game.loader,
                    loader_version: game.loader_version.clone(),
                },
                InstanceSettings::default(),
                false,
                now(),
            )
            .cloned();
        let record = match created {
            Ok(record) => record,
            Err(error) => {
                staged.discard();
                return Err(error.into());
            }
        };
        if let Err(error) = staged.publish(&self.layout) {
            let _ = self.store.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        Ok(record)
    }

    /// Saves the whole game (mods, config, worlds, options; not logs) as one
    /// file at `destination` (ADR 0015) and returns its size. Takes the
    /// instance lease so the files hold still. An Activity task; cancelling
    /// it leaves no file.
    pub async fn backup_instance(
        &self,
        id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<u64, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self.instance(id).await?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("备份 {}", record.name),
                Some(id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::BackupInstance {
                instance: id.to_owned(),
                path: destination.display().to_string(),
            });
        let meta = crate::backup::BackupMeta {
            format: 1,
            name: record.name.clone(),
            game_version: record.game_version.clone(),
            loader: record.loader,
            loader_version: record.loader_version.clone(),
            created: now(),
            settings: record.settings.clone(),
        };
        let (game_dir, destination, token) =
            (self.layout.game(id), destination.to_owned(), cancel.clone());
        let result = tokio::task::spawn_blocking(move || {
            crate::backup::write(&game_dir, &meta, &destination, &token)
        })
        .await
        .map_err(std::io::Error::other)
        .map_err(ServiceError::from)
        .and_then(|written| written.map_err(backup_failure));
        self.end(task, &result);
        result
    }

    /// Makes a new game out of a backup (ADR 0015). The game is built whole
    /// before it appears; nothing existing is touched. Its game files are
    /// installed on first launch like any new game.
    pub async fn restore_backup(
        &self,
        archive: &Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let label = archive
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let task = self
            .begin(
                TaskCategory::Install,
                format!("恢复备份 {label}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::RestoreBackup {
                path: archive.display().to_string(),
            });
        let result = self.restore_backup_inner(archive, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    pub(super) async fn restore_backup_inner(
        &self,
        archive: &Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let source = archive.to_owned();
        let meta = blocking_value({
            let source = source.clone();
            move || crate::backup::read_meta(&source)
        })
        .await?
        .map_err(backup_failure)?;
        let (id, name) = {
            let store = self.store.lock().await;
            let taken = store
                .instances()
                .iter()
                .any(|record| record.name.trim() == meta.name.trim());
            let name = if taken {
                format!("{}（恢复）", meta.name.trim())
            } else {
                meta.name.trim().to_owned()
            };
            (store.suggest_id(&name)?, name)
        };
        let staged = Staged::begin(&self.layout, "import", &id)?;
        let (into, token) = (staged.game_dir(), cancel.clone());
        let extracted = blocking_value(move || crate::backup::restore(&source, &into, &token))
            .await
            .and_then(|result| result.map_err(backup_failure));
        if let Err(error) = extracted {
            staged.discard();
            return Err(error);
        }
        if let Err(error) = staged.mark_ready() {
            staged.discard();
            return Err(error.into());
        }
        let mut settings = meta.settings.clone();
        // A Java path belongs to the machine the backup came from.
        settings.java_path = None;
        let created = self
            .store
            .lock()
            .await
            .create_as(
                &id,
                NewInstance {
                    name,
                    game_version: meta.game_version.clone(),
                    loader: meta.loader,
                    loader_version: meta.loader_version.clone(),
                },
                settings,
                false,
                now(),
            )
            .cloned();
        let record = match created {
            Ok(record) => record,
            Err(error) => {
                staged.discard();
                return Err(error.into());
            }
        };
        if let Err(error) = staged.publish(&self.layout) {
            let _ = self.store.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        Ok(record)
    }
}
