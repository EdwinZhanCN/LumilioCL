use super::LauncherService;
use super::error::{ServiceError, read_unless_cancelled};
use crate::activity::CancellationToken;
use crate::activity_log::{TaskAction, TaskCategory, TaskLabel};
use crate::history::ChangeKind;
use crate::instance::InstanceRecord;
use crate::transfer::Transport;
use crate::worlds;
use crate::worlds::WorldInfo;
use std::path::Path;

impl<T: Transport + Clone> LauncherService<T> {
    /// The instance's saved worlds. Reading needs no instance lease.
    pub async fn worlds(&self, id: &str) -> Result<Vec<WorldInfo>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || worlds::scan(&game_dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Copies a world under a new folder name (`<folder> copy`, `copy 2`… when
    /// none is given) and returns that name. Never overwrites; the copy appears
    /// all at once. Takes the instance lease.
    pub async fn copy_world(
        &self,
        id: &str,
        folder: &str,
        new_folder: Option<&str>,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let (folder, requested) = (folder.to_owned(), new_folder.map(str::to_owned));
        let created = tokio::task::spawn_blocking(move || {
            // Under the lease nothing else is copying: leftovers are stale.
            worlds::sweep_temporary(&game_dir);
            let name = requested.unwrap_or_else(|| worlds::copy_name(&game_dir, &folder));
            worlds::duplicate(&game_dir, &folder, &name)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldCopied, &created);
        Ok(created)
    }

    /// Deletes one world folder. Takes the instance lease.
    pub async fn delete_world(&self, id: &str, folder: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let name = folder.to_owned();
        tokio::task::spawn_blocking(move || worlds::delete(&game_dir, &name))
            .await
            .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldDeleted, folder);
        Ok(())
    }

    /// Packs one world into a zip at `destination` and returns its size.
    /// Takes the instance lease, so the world is not changing underneath.
    pub async fn export_world(
        &self,
        id: &str,
        folder: &str,
        destination: &Path,
    ) -> Result<u64, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let (folder, destination) = (folder.to_owned(), destination.to_owned());
        tokio::task::spawn_blocking(move || worlds::export_zip(&game_dir, &folder, &destination))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Packs the chosen files of the game as a Modrinth modpack at
    /// `destination`. Files Modrinth knows are listed by address; the rest are
    /// carried inside. If Modrinth cannot be asked, everything is carried and
    /// the report says so. Takes the instance lease so the files hold still.
    pub async fn export_modpack(
        &self,
        id: &str,
        spec: crate::pack_export::ExportSpec,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<crate::pack_export::ExportReport, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        let record = self.instance(id).await?;
        let task = self.begin(
            TaskCategory::Install,
            TaskLabel::Typed {
                action: TaskAction::ExportWorld,
                subject: spec.name.trim().to_owned(),
            },
            Some(id.to_owned()),
            Some(&cancel),
        );
        let result = self
            .export_modpack_inner(&record, spec, destination, cancel)
            .await
            .map_err(|error| match error {
                ServiceError::Export(crate::pack_export::ExportError::Cancelled) => {
                    ServiceError::Cancelled
                }
                other => other,
            });
        self.end(task, &result);
        result
    }

    pub(super) async fn export_modpack_inner(
        &self,
        record: &InstanceRecord,
        spec: crate::pack_export::ExportSpec,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<crate::pack_export::ExportReport, ServiceError> {
        use crate::pack_export as pack;
        let game_dir = self.layout.game(&record.id);

        let (dir, include, stop) = (game_dir.clone(), spec.include.clone(), cancel.clone());
        // Only a Modrinth pack lists files by address, so only it needs hashes.
        let wants_hashes = spec.format == pack::PackFormat::Modrinth;
        let (files, hashed) = tokio::task::spawn_blocking(move || {
            let files = pack::collect_files(&dir, &include)?;
            let mut hashed = Vec::new();
            for path in pack::linkable(&files).into_iter().filter(|_| wants_hashes) {
                if stop.is_cancelled() {
                    return Err(pack::ExportError::Cancelled);
                }
                let file = dir.join(path);
                let (sha1, sha512) = (crate::content::sha1_hex(&file)?, pack::sha512_hex(&file)?);
                let size = std::fs::metadata(&file)?.len();
                hashed.push((path.clone(), sha1, sha512, size));
            }
            Ok::<_, pack::ExportError>((files, hashed))
        })
        .await
        .map_err(std::io::Error::other)??;

        if spec.format == pack::PackFormat::Prism {
            let pack_json = pack::prism_pack_json(
                &record.game_version,
                record.loader,
                record.loader_version.as_deref(),
            )?;
            let (destination, name, stop) =
                (destination.to_owned(), spec.name.clone(), cancel.clone());
            let bundled = files.len();
            let size = tokio::task::spawn_blocking(move || {
                pack::write_prism(&destination, &game_dir, &name, &pack_json, &files, &|| {
                    stop.is_cancelled()
                })
            })
            .await
            .map_err(std::io::Error::other)??;
            return Ok(pack::ExportReport {
                linked: 0,
                bundled,
                lookup_failed: false,
                size,
            });
        }

        let sha1s: Vec<String> = hashed.iter().map(|(_, sha1, _, _)| sha1.clone()).collect();
        let identified = match read_unless_cancelled(&cancel, self.content_client()).await {
            Ok(client) => client.identify(&sha1s).await.ok(),
            Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
            Err(_) => None,
        };
        let lookup_failed = identified.is_none() && !sha1s.is_empty();
        let linked = identified
            .map(|identified| pack::link_identified(&hashed, &identified))
            .unwrap_or_default();
        let carried: Vec<String> = files
            .iter()
            .filter(|path| !linked.iter().any(|file| &file.path == *path))
            .cloned()
            .collect();
        let index = pack::index_json(
            &spec,
            &record.game_version,
            record.loader,
            record.loader_version.as_deref(),
            &linked,
        )?;
        let destination = destination.to_owned();
        let bundled = carried.len();
        let stop = cancel.clone();
        let size = tokio::task::spawn_blocking(move || {
            pack::write_pack(&destination, &game_dir, &index, &carried, &|| {
                stop.is_cancelled()
            })
        })
        .await
        .map_err(std::io::Error::other)??;
        Ok(pack::ExportReport {
            linked: linked.len(),
            bundled,
            lookup_failed,
            size,
        })
    }

    /// Adds the world in a zip as a new world and returns its folder name;
    /// existing worlds are never replaced. Takes the instance lease.
    pub async fn import_world(&self, id: &str, archive: &Path) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (game_dir, archive) = (self.layout.game(id), archive.to_owned());
        let created = tokio::task::spawn_blocking(move || {
            worlds::sweep_temporary(&game_dir);
            worlds::import_zip(&game_dir, &archive)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::WorldImported, &created);
        Ok(created)
    }
}
