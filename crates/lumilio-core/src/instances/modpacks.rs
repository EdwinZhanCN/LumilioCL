use super::InstanceLibrary;
use super::packs::{PackKind, UnpackError, sniff_pack, unpack_instance_zip};
use crate::activity::CancellationToken;
use crate::activity::{RetryAction, TaskAction, TaskCategory, TaskLabel};
use crate::blocking::blocking_value;
use crate::clock::now;
use crate::content::Content;
use crate::discover::{Catalog, ProjectOrigin};
use crate::error::{ServiceError, read_unless_cancelled};
use crate::instances::{InstanceRecord, NewInstance};
use crate::modpack;
use crate::modpack::PackIndex;
use crate::staged::Staged;
use crate::transfer::{TransferRequest, Transport};

impl<T: Transport + Clone> InstanceLibrary<T> {
    /// Copies an instance under a new name. The copy is built and checked in a
    /// staging folder, then published whole; the source is only read. It
    /// shares the source's release (installed game files live in `meta/`),
    /// inherits its settings and installed state, and starts with no favorite,
    /// play time, history or snapshots. `include_worlds` decides whether
    /// `saves/` comes along; `logs/` and `crash-reports/` never do. Refused
    /// while the source launches or runs; cancellable through `cancel_task`.
    pub(crate) async fn copy_instance(
        &self,
        source_id: &str,
        new_name: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let _lease = self.instances.reserve(source_id)?;
        let source = self.instances.record(source_id).await?;
        let staged = Staged::claim(&*self.instances.lock().await, "copy", new_name)?;
        let task = self
            .tasks
            .begin(
                TaskCategory::Install,
                TaskLabel::Typed {
                    action: TaskAction::CopyGame,
                    subject: source.name.clone(),
                },
                Some(source_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::CopyInstance {
                source: source_id.to_owned(),
                name: new_name.to_owned(),
                include_worlds,
            });
        let result = self
            .copy_staged(&source, new_name, staged, include_worlds, cancel)
            .await;
        self.tasks.end(task, &result);
        result
    }

    pub(crate) async fn copy_staged(
        &self,
        source: &InstanceRecord,
        new_name: &str,
        staged: Staged,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let id = staged.id().to_owned();
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
            .instances
            .lock()
            .await
            .create_as(
                &id,
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
            let _ = self.instances.lock().await.remove(&record.id);
            staged.discard();
            return Err(error.into());
        }
        crate::settings::settle_current(&self.instances, &self.context, &self.settings).await;
        Ok(record)
    }

    /// Installs a modpack project of a content source as a new instance:
    /// `version` (a version id of the project), or its newest version when
    /// `None`.
    pub(crate) async fn install_modpack(
        &self,
        content: &Content<T>,
        source: &str,
        project: &str,
        version: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let task = self
            .tasks
            .begin(
                TaskCategory::Install,
                TaskLabel::Typed {
                    action: TaskAction::InstallModpack,
                    subject: project.to_owned(),
                },
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallModpack {
                source: source.to_owned(),
                project: project.to_owned(),
                version: version.map(str::to_owned),
            });
        let result = self
            .install_modpack_inner(content, task.id, source, project, version, cancel)
            .await;
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.tasks.end(task, &result);
        result
    }

    /// Imports a local `.mrpack` or CurseForge pack zip (or an instance or
    /// backup zip) as a new instance. Same rules as installing a downloaded
    /// pack; the file itself is never deleted or modified.
    pub(crate) async fn import_modpack_file(
        &self,
        content: &Content<T>,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let label = pack
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let task = self
            .tasks
            .begin(
                TaskCategory::Install,
                TaskLabel::Typed {
                    action: TaskAction::ImportModpack,
                    subject: label.clone(),
                },
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
                    PackKind::Modrinth | PackKind::CurseForge => {
                        self.import_pack_from(content, task.id, pack, cancel).await
                    }
                }
            }
            Ok(_) => Err(ServiceError::Install(format!("{label:?} is not a file"))),
            Err(error) => Err(error.into()),
        };
        if let Ok(record) = &result {
            task.about(&record.id);
        }
        self.tasks.end(task, &result);
        result
    }

    /// A MultiMC / Prism instance in a zip (ADR 0016): unpacked into a
    /// scratch folder, read like any other launcher's game, imported, and the
    /// scratch folder removed whatever happened.
    pub(crate) async fn import_prism_zip(
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

    /// Imports a downloaded or chosen pack of either format.
    pub(crate) async fn import_pack_from(
        &self,
        content: &Content<T>,
        task: u64,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let kind = {
            let pack = pack.to_owned();
            blocking_value(move || sniff_pack(&pack)).await?
        };
        let (index, allow) = if kind == PackKind::CurseForge {
            let index =
                read_unless_cancelled(&cancel, self.curseforge_index(content, pack)).await?;
            (
                index,
                modpack::curseforge::is_trusted_source as fn(&str) -> bool,
            )
        } else {
            let index = {
                let pack = pack.to_owned();
                blocking_value(move || modpack::read_index(&pack)).await??
            };
            (index, modpack::is_trusted_source as fn(&str) -> bool)
        };
        let chain = self.network.chain().await?;
        let engine = self.network.transfer_engine().await;
        let record = self
            .tasks
            .metered(
                task,
                engine.subscribe(),
                modpack::import_index(
                    self.instances.store(),
                    &engine,
                    &chain,
                    pack,
                    &index,
                    now(),
                    allow,
                    cancel,
                ),
            )
            .await
            .map_err(ServiceError::from)?;
        if !index.manual.is_empty() {
            // The game is installed either way; the list is how the person
            // learns what to add.
            let _ = content
                .save_pending_downloads(&record.id, &index.manual)
                .await;
        }
        crate::settings::settle_current(&self.instances, &self.context, &self.settings).await;
        Ok(record)
    }

    /// A CurseForge pack as an index: its files resolved by a CurseForge
    /// source, and those the author lets people download only on
    /// CurseForge's site looked for on Modrinth (plan curseforge-source D3).
    async fn curseforge_index(
        &self,
        content: &Content<T>,
        pack: &std::path::Path,
    ) -> Result<PackIndex, ServiceError> {
        let manifest = {
            let pack = pack.to_owned();
            blocking_value(move || modpack::curseforge::read_manifest(&pack)).await??
        };
        let client = self
            .plugins
            .catalog_client(Catalog::CurseForge)
            .await
            .ok_or(ServiceError::NoCatalogSource(Catalog::CurseForge))?;
        let references: Vec<(String, String)> = manifest
            .files
            .iter()
            .map(|file| (file.project.clone(), file.file.clone()))
            .collect();
        let resolved = client.resolve_files(&references).await?;
        let blocked: Vec<String> = resolved
            .iter()
            .filter(|file| !file.file.downloadable())
            .filter_map(|file| file.file.sha1.clone())
            .collect();
        let elsewhere = if blocked.is_empty() {
            std::collections::BTreeMap::new()
        } else {
            content.elsewhere(&blocked).await
        };
        Ok(modpack::curseforge::index(
            &manifest, &resolved, &elsewhere,
        )?)
    }

    pub(crate) async fn install_modpack_inner(
        &self,
        content: &Content<T>,
        task: u64,
        source: &str,
        project: &str,
        wanted: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.network.chain()).await?;
        let client = read_unless_cancelled(&cancel, content.content_client(source)).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client.versions(project).await.map_err(ServiceError::from)
        })
        .await?;
        let version = match wanted {
            Some(id) => versions.iter().find(|version| version.id == id),
            None => versions.iter().max_by(|a, b| a.published.cmp(&b.published)),
        }
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
        let sources = read_unless_cancelled(&cancel, content.file_sources(&chain, file)).await?;
        let mut request = TransferRequest::new(format!("pack:{}", version.id), sources, &pack)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        if file.size > 0 {
            request = request.expect_size(file.size);
        }
        if let Some(sha1) = &file.sha1 {
            request = request
                .expect_sha1(sha1)
                .map_err(|error| ServiceError::Install(error.to_string()))?;
        }
        let engine = self.network.transfer_engine().await;
        self.tasks
            .metered(
                task,
                engine.subscribe(),
                engine.transfer(request, cancel.clone()),
            )
            .await
            .map_err(ServiceError::from)?;
        let record = self.import_pack_from(content, task, &pack, cancel).await;
        let _ = tokio::fs::remove_file(&pack).await;
        let mut record = record?;
        // Remembering where it came from is a convenience, not part of the install.
        let origin = ProjectOrigin {
            source: client.plugin_id().to_owned(),
            project: version.project_id.clone(),
        }
        .stored();
        if self
            .instances
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
