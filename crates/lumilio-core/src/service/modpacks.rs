use super::LauncherService;
use super::error::{ServiceError, read_unless_cancelled};
use super::packs::{PackKind, UnpackError, sniff_pack, unpack_instance_zip};
use super::support::blocking_value;
use super::types::now;
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskCategory};
use crate::instance::{InstanceRecord, NewInstance};
use crate::modpack;
use crate::staged::Staged;
use crate::transfer::{TransferEngine, TransferRequest, Transport};

impl<T: Transport + Clone> LauncherService<T> {
    /// Copies an instance under a new name. The copy is built and checked in a
    /// staging folder, then published whole; the source is only read. It
    /// shares the source's release (installed game files live in `meta/`),
    /// inherits its settings and installed state, and starts with no favorite,
    /// play time, history or snapshots. `include_worlds` decides whether
    /// `saves/` comes along; `logs/` and `crash-reports/` never do. Refused
    /// while the source launches or runs; cancellable through `cancel_task`.
    pub async fn copy_instance(
        &self,
        source_id: &str,
        new_name: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.reserve_instance(source_id)?;
        let source = self.instance(source_id).await?;
        let id = self.store.lock().await.suggest_id(new_name)?;
        let task = self
            .begin(
                TaskCategory::Install,
                format!("复制 {}", source.name),
                Some(source_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::CopyInstance {
                source: source_id.to_owned(),
                name: new_name.to_owned(),
                include_worlds,
            });
        let result = self
            .copy_staged(&source, new_name, &id, include_worlds, cancel)
            .await;
        self.end(task, &result);
        result
    }

    pub(super) async fn copy_staged(
        &self,
        source: &InstanceRecord,
        new_name: &str,
        id: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let staged = Staged::begin(&self.layout, "copy", id)?;
        let (from, to) = (self.layout.game(&source.id), staged.game_dir());
        let token = cancel.clone();
        let copied = tokio::task::spawn_blocking(move || {
            crate::copy::copy_game(&from, &to, include_worlds, &token)
        })
        .await
        .map_err(std::io::Error::other);
        let copied = match copied {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(crate::copy::CopyError::Cancelled)) => Err(ServiceError::Cancelled),
            Ok(Err(crate::copy::CopyError::Io(error))) | Err(error) => Err(error.into()),
        };
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
                id,
                NewInstance {
                    name: new_name.to_owned(),
                    game_version: source.game_version.clone(),
                    loader: source.loader,
                    loader_version: source.loader_version.clone(),
                },
                source.settings.clone(),
                source.installed,
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

    /// Installs a modpack project as a new instance.
    pub async fn install_modpack(
        &self,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Install,
                format!("安装整合包 {project}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallModpack {
                project: project.to_owned(),
            });
        let result = self.install_modpack_inner(task.id, project, cancel).await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    /// Imports a local `.mrpack` as a new instance. Same rules as installing a
    /// downloaded pack; the file itself is never deleted or modified.
    pub async fn import_modpack_file(
        &self,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let label = pack
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let task = self
            .begin(
                TaskCategory::Install,
                format!("导入整合包 {label}"),
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::ImportPack {
                path: pack.display().to_string(),
            });
        let result = match tokio::fs::metadata(pack).await {
            Ok(metadata) if metadata.is_file() => {
                let kind = {
                    let pack = pack.to_owned();
                    blocking_value(move || sniff_pack(&pack)).await?
                };
                match kind {
                    PackKind::Prism => self.import_prism_zip(pack, cancel).await,
                    PackKind::Backup => self.restore_backup_inner(pack, cancel).await,
                    PackKind::Modrinth => self.import_pack_from(task.id, pack, cancel).await,
                }
            }
            Ok(_) => Err(ServiceError::Install(format!("{label:?} is not a file"))),
            Err(error) => Err(error.into()),
        };
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.end(task, &result);
        result
    }

    /// A MultiMC / Prism instance in a zip (ADR 0016): unpacked into a
    /// scratch folder, read like any other launcher's game, imported, and the
    /// scratch folder removed whatever happened.
    pub(super) async fn import_prism_zip(
        &self,
        archive: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let scratch = self.layout.root().join("cache").join(format!(
            "prism-import-{}-{}",
            std::process::id(),
            now()
        ));
        let outcome = async {
            let (archive, scratch, token) = (archive.to_owned(), scratch.clone(), cancel.clone());
            let root = blocking_value(move || unpack_instance_zip(&archive, &scratch, &token))
                .await?
                .map_err(|error| match error {
                    UnpackError::Cancelled => ServiceError::Cancelled,
                    UnpackError::Other(why) => ServiceError::Install(why),
                })?;
            let found = self.detect_games(&root).await?;
            let game = found
                .into_iter()
                .next()
                .ok_or_else(|| ServiceError::Install("the zip holds no game".to_owned()))?;
            self.import_game_inner(game, cancel.clone()).await
        }
        .await;
        let _ = tokio::fs::remove_dir_all(&scratch).await;
        outcome
    }

    pub(super) async fn import_pack_from(
        &self,
        task: u64,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let chain = self.chain().await?;
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(
            task,
            engine.subscribe(),
            modpack::import(
                &self.store,
                &engine,
                &chain,
                pack,
                now(),
                modpack::is_trusted_source,
                cancel,
            ),
        )
        .await
        .map_err(ServiceError::from)
    }

    pub(super) async fn install_modpack_inner(
        &self,
        task: u64,
        project: &str,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.content_client()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client.versions(project).await.map_err(ServiceError::from)
        })
        .await?;
        let version = versions
            .iter()
            .max_by(|a, b| a.published.cmp(&b.published))
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let file = version
            .install_file()
            .ok_or(ServiceError::NoCompatibleVersion)?;
        if !crate::discover::is_safe_file_name(&file.filename) {
            return Err(ServiceError::Install(format!(
                "unsafe file name {:?}",
                file.filename
            )));
        }
        let pack = self.layout.root().join("downloads").join(&file.filename);
        let mut request = TransferRequest::new(
            format!("pack:{}", version.id),
            chain.candidates(&file.url),
            &pack,
        )
        .map_err(|error| ServiceError::Install(error.to_string()))?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        if let Some(sha1) = &file.sha1 {
            request = request
                .expect_sha1(sha1)
                .map_err(|error| ServiceError::Install(error.to_string()))?;
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(
            task,
            engine.subscribe(),
            engine.transfer(request, cancel.clone()),
        )
        .await
        .map_err(ServiceError::from)?;
        let record = self.import_pack_from(task, &pack, cancel).await;
        let _ = tokio::fs::remove_file(&pack).await;
        let mut record = record?;
        // Remembering where it came from is a convenience, not part of the install.
        let origin = version.project_id.clone();
        if self
            .store
            .lock()
            .await
            .set_source_project(&record.id, &origin)
            .is_ok()
        {
            record.source_project = Some(origin);
        }
        Ok(record)
    }
}
