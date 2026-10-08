use super::LauncherService;
use super::error::{ServiceError, read_unless_cancelled};
use super::support::blocking;
use super::types::{DependencyNeed, DependencyReport, InstalledProject, now};
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskAction, TaskCategory, TaskLabel};
use crate::content::ContentError;
use crate::content::scan as scan_content;
use crate::discover::{ProjectKind, Version, fits, install_request, pick_version};
use crate::history::{ChangeKind, HistoryEvent, HistoryLog};
use crate::instance::InstanceRecord;
use crate::transfer::{TransferEngine, TransferRequest, Transport};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

impl<T: Transport + Clone> LauncherService<T> {
    /// Downloads one version's file to a place the person chose, instead of
    /// into a game. Returns the file's name. A file already there is replaced
    /// (the person was asked by the save dialog).
    pub async fn save_version_as(
        &self,
        project: &str,
        version_id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Download,
                TaskLabel::Typed {
                    action: TaskAction::DownloadContent,
                    subject: project.to_owned(),
                },
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::SaveVersion {
                project: project.to_owned(),
                version_id: version_id.to_owned(),
                path: destination.display().to_string(),
            });
        let result = self
            .save_version_inner(task.id, project, version_id, destination, cancel)
            .await;
        self.end(task, &result);
        result
    }

    pub(super) async fn save_version_inner(
        &self,
        task: u64,
        project: &str,
        version_id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.content_client()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client.versions(project).await.map_err(ServiceError::from)
        })
        .await?;
        let version = versions
            .iter()
            .find(|version| version.id == version_id)
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let file = version
            .install_file()
            .ok_or_else(|| ServiceError::Install("this version has no file".to_owned()))?;
        let mut request = TransferRequest::new(
            format!("save:{}:{}", version.project_id, version.id),
            chain.candidates(&file.url),
            destination,
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
        self.metered(task, engine.subscribe(), engine.transfer(request, cancel))
            .await
            .map_err(ServiceError::from)?;
        Ok(file.filename.clone())
    }

    /// What of this kind the game already has, by Modrinth project: the file
    /// and, if a newer compatible version exists, its version id. Files
    /// Modrinth does not know are not listed. Needs the network.
    pub async fn installed_projects(
        &self,
        instance_id: &str,
        kind: ProjectKind,
    ) -> Result<BTreeMap<String, InstalledProject>, ServiceError> {
        let list = self.content_details(instance_id, kind).await?;
        if list.sources_unavailable {
            return Err(ServiceError::Remote(
                "the content source could not be asked which files are known".to_owned(),
            ));
        }
        Ok(list
            .entries
            .into_iter()
            .filter_map(|entry| {
                let source = entry.source?;
                Some((
                    source.project_id,
                    InstalledProject {
                        file_name: entry.item.file_name,
                        version_id: source.version_id,
                        update: entry.update.map(|version| version.id),
                    },
                ))
            })
            .collect())
    }

    /// The required dependencies the game lacks (see [`Self::dependency_report`]).
    pub async fn missing_dependencies(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<Vec<DependencyNeed>, ServiceError> {
        Ok(self
            .dependency_report(instance_id, kind, project, version_id)
            .await?
            .needs)
    }

    /// What installing the version `install_content` would pick (or
    /// `version_id`) means for the game's other mods: the required
    /// dependencies it lacks (nearest first, up to four levels), the optional
    /// ones it lacks (the version's own only), and installed mods it declares
    /// itself incompatible with. Only mods have any of these. A dependency
    /// with no version for this game is listed with `version: None` so the
    /// person is told instead of left to crash.
    pub async fn dependency_report(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
    ) -> Result<DependencyReport, ServiceError> {
        const DEPTH: usize = 4;
        if kind != ProjectKind::Mod {
            return Ok(DependencyReport::default());
        }
        let record = self.instance(instance_id).await?;
        let client = self.content_client().await?;
        let versions = client.versions(project).await?;
        let root = match version_id {
            Some(id) => versions
                .iter()
                .find(|version| version.id == id)
                .filter(|version| fits(version, kind, &record.game_version, record.loader)),
            None => pick_version(&versions, kind, &record.game_version, record.loader),
        }
        .ok_or(ServiceError::NoCompatibleVersion)?;

        // What the game already has, by project (files Modrinth does not know
        // cannot be matched, so a dependency they satisfy is offered anyway).
        let game_dir = self.layout.game(instance_id);
        let hashes = blocking(move || {
            let mut hashes = Vec::new();
            for item in scan_content(&game_dir, ProjectKind::Mod).unwrap_or_default() {
                if item.enabled
                    && !item.is_directory
                    && let Ok(hash) =
                        crate::content::sha1_hex(&game_dir.join("mods").join(&item.file_name))
                {
                    hashes.push(hash);
                }
            }
            Ok(hashes)
        })
        .await?;
        let installed: BTreeSet<String> = client
            .identify(&hashes)
            .await
            .map(|found| found.values().map(|v| v.project_id.clone()).collect())
            .unwrap_or_default();
        let mut have = installed.clone();
        have.insert(root.project_id.clone());

        let resolve = |dependency: &crate::discover::Dependency, candidates: &[Version]| {
            dependency
                .version_id
                .as_deref()
                .and_then(|wanted| candidates.iter().find(|v| v.id == wanted))
                .filter(|v| fits(v, kind, &record.game_version, record.loader))
                .or_else(|| pick_version(candidates, kind, &record.game_version, record.loader))
                .cloned()
        };

        let mut found: Vec<(String, Option<Version>)> = Vec::new();
        let mut level: Vec<Version> = vec![root.clone()];
        for _ in 0..DEPTH {
            let mut next = Vec::new();
            for version in &level {
                for dependency in version.required_dependencies() {
                    let Some(dependency_project) = dependency.project_id.clone() else {
                        continue;
                    };
                    if !have.insert(dependency_project.clone()) {
                        continue;
                    }
                    let candidates = client.versions(&dependency_project).await?;
                    let chosen = resolve(dependency, &candidates);
                    if let Some(chosen) = &chosen {
                        next.push(chosen.clone());
                    }
                    found.push((dependency_project, chosen));
                }
            }
            if next.is_empty() {
                break;
            }
            level = next;
        }

        // Optional ones of the version itself, and what it cannot live with.
        let mut optional: Vec<(String, Option<Version>)> = Vec::new();
        let mut conflicts: Vec<String> = Vec::new();
        for dependency in &root.dependencies {
            let Some(other) = dependency.project_id.clone() else {
                continue;
            };
            match dependency.kind {
                crate::discover::DependencyKind::Optional if have.insert(other.clone()) => {
                    if let Ok(candidates) = client.versions(&other).await {
                        optional.push((other, resolve(dependency, &candidates)));
                    }
                }
                crate::discover::DependencyKind::Incompatible if installed.contains(&other) => {
                    conflicts.push(other);
                }
                _ => {}
            }
        }

        let ids: Vec<String> = found
            .iter()
            .chain(optional.iter())
            .map(|(id, _)| id.clone())
            .chain(conflicts.iter().cloned())
            .collect();
        let titles: BTreeMap<String, String> = client
            .project_summaries(&ids)
            .await
            .map(|summaries| summaries.into_iter().map(|s| (s.id, s.title)).collect())
            .unwrap_or_default();
        let title_of = |id: &str| titles.get(id).cloned().unwrap_or_else(|| id.to_owned());
        let need = |(project_id, version): (String, Option<Version>)| DependencyNeed {
            title: title_of(&project_id),
            project_id,
            version,
        };
        Ok(DependencyReport {
            needs: found.into_iter().map(need).collect(),
            optional: optional.into_iter().map(need).collect(),
            conflicts: conflicts.iter().map(|id| title_of(id)).collect(),
        })
    }

    pub(super) async fn install_from(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        let record = self
            .store
            .lock()
            .await
            .get(instance_id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(instance_id.to_owned()))?;
        let task = self
            .begin(
                TaskCategory::Download,
                TaskLabel::Typed {
                    action: TaskAction::InstallContent,
                    subject: project.to_owned(),
                },
                Some(instance_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::InstallContent {
                instance: instance_id.to_owned(),
                kind,
                project: project.to_owned(),
                version: version_id.map(str::to_owned),
            });
        let result = self
            .install_content_inner(task.id, &record, kind, project, version_id, cancel)
            .await;
        self.end(task, &result);
        result
    }

    pub(super) async fn install_content_inner(
        &self,
        task: u64,
        record: &InstanceRecord,
        kind: ProjectKind,
        project: &str,
        version_id: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.content_client()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client.versions(project).await.map_err(ServiceError::from)
        })
        .await?;
        let version = match version_id {
            Some(id) => versions
                .iter()
                .find(|version| version.id == id)
                .filter(|version| fits(version, kind, &record.game_version, record.loader)),
            None => pick_version(&versions, kind, &record.game_version, record.loader),
        }
        .ok_or(ServiceError::NoCompatibleVersion)?;
        let game_dir = self.layout.game(&record.id);
        let file = version.install_file().map(|file| file.url.clone());
        let sources = file.map(|url| chain.candidates(&url)).unwrap_or_default();
        let request: TransferRequest = install_request(kind, version, &game_dir, sources)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        let name = request
            .destination()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        // A same-named file with other content is the user's: never replace it
        // silently. Identical content is reused by the transfer.
        crate::content::folder(&game_dir, kind)?;
        let target = request.destination().to_path_buf();
        let expected = request.expected_sha1().map(str::to_owned);
        let in_the_way = blocking(move || {
            if target.symlink_metadata().is_err() {
                return Ok(false);
            }
            let same = target.is_file()
                && expected.is_some_and(|expected| {
                    crate::content::sha1_hex(&target).is_ok_and(|actual| actual == expected)
                });
            Ok(!same)
        })
        .await?;
        if in_the_way {
            return Err(ContentError::Conflict(name).into());
        }
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        self.metered(task, engine.subscribe(), engine.transfer(request, cancel))
            .await
            .map_err(ServiceError::from)?;
        HistoryLog::for_instance(self.layout.root(), &record.id).append(&HistoryEvent::Change {
            at: now(),
            kind: ChangeKind::ContentAdded,
            subject: name.clone(),
        })?;
        Ok(name)
    }

    /// Replaces an installed file with another version of the same project
    /// (IA P-VERSION-SWITCH: "update" and "switch version" are this one
    /// operation). The new file is downloaded and verified first; only then is
    /// the old one removed. A disabled file stays disabled. A different file
    /// already using the new name is never overwritten (`Conflict`).
    pub async fn switch_content_version(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        file_name: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        self.instance(instance_id).await?;
        let task = self
            .begin(
                TaskCategory::Update,
                TaskLabel::Typed {
                    action: TaskAction::SwitchContentVersion,
                    subject: file_name.to_owned(),
                },
                Some(instance_id.to_owned()),
                Some(&cancel),
            )
            .retrying(RetryAction::SwitchContent {
                instance: instance_id.to_owned(),
                kind,
                file_name: file_name.to_owned(),
                project: project.to_owned(),
                version_id: version_id.to_owned(),
            });
        let result = self
            .switch_inner(
                task.id,
                instance_id,
                kind,
                file_name,
                project,
                version_id,
                cancel,
            )
            .await;
        self.end(task, &result);
        result
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn switch_inner(
        &self,
        task: u64,
        instance_id: &str,
        kind: ProjectKind,
        file_name: &str,
        project: &str,
        version_id: &str,
        cancel: CancellationToken,
    ) -> Result<String, ServiceError> {
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let client = read_unless_cancelled(&cancel, self.content_client()).await?;
        let versions = read_unless_cancelled(&cancel, async {
            client.versions(project).await.map_err(ServiceError::from)
        })
        .await?;
        let version = versions
            .into_iter()
            .find(|version| version.id == version_id)
            .ok_or(ServiceError::NoCompatibleVersion)?;
        let game_dir = self.layout.game(instance_id);
        let folder = crate::content::folder(&game_dir, kind)?;
        let disabled = file_name.ends_with(crate::content::DISABLED_SUFFIX);
        let new_name = version
            .install_file()
            .map(|file| file.filename.clone())
            .ok_or(ServiceError::NoCompatibleVersion)?;
        // Another file already using the new name is the user's.
        let current = file_name
            .strip_suffix(crate::content::DISABLED_SUFFIX)
            .unwrap_or(file_name);
        if new_name != current {
            let taken = [
                new_name.clone(),
                format!("{new_name}{}", crate::content::DISABLED_SUFFIX),
            ]
            .into_iter()
            .any(|name| folder.join(name).symlink_metadata().is_ok());
            if taken {
                return Err(ContentError::Conflict(new_name).into());
            }
        }
        let current_sha1 = {
            let path = folder.join(file_name);
            blocking(move || crate::content::sha1_hex(&path)).await?
        };
        let sources = version
            .install_file()
            .map(|file| chain.candidates(&file.url))
            .unwrap_or_default();
        let update = crate::updates::ContentUpdate {
            kind,
            file_name: file_name.to_owned(),
            current_sha1,
            latest: version,
        };
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        let written = self
            .metered(
                task,
                engine.subscribe(),
                crate::updates::apply(&engine, &update, &game_dir, sources, cancel),
            )
            .await
            .map_err(|error| ServiceError::Install(error.to_string()))?;
        if disabled && written != file_name {
            // A rename: quick enough to do here.
            crate::content::set_enabled(&game_dir, kind, &written, false)?;
        }
        self.record_change(instance_id, ChangeKind::ContentUpdated, &written);
        Ok(written)
    }
}
