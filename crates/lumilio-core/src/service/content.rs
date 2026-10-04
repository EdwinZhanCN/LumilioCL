use super::LauncherService;
use super::error::ServiceError;
use super::types::{ContentEffect, ContentResult, now};
use crate::content::scan as scan_content;
use crate::content::{ContentError, ContentItem};
use crate::discover::{ProjectKind, Version};
use crate::history::{ChangeKind, HistoryEvent, HistoryLog};
use crate::transfer::Transport;
use std::collections::BTreeMap;

impl<T: Transport + Clone> LauncherService<T> {
    /// Copies local files into an instance (IA P-ADD-FILES): one result per
    /// file, so one refusal does not hide the others. Needs the instance lease.
    pub async fn import_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    ) -> Result<Vec<(String, Result<String, ContentError>)>, ServiceError> {
        let _lease = self.reserve_instance(instance_id)?;
        self.instance(instance_id).await?;
        let game_dir = self.layout.game(instance_id);
        let results = tokio::task::spawn_blocking(move || {
            files
                .into_iter()
                .map(|file| {
                    let shown = file
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| file.display().to_string());
                    (shown, crate::content::import(&game_dir, kind, &file))
                })
                .collect::<Vec<_>>()
        })
        .await
        .map_err(std::io::Error::other)?;
        for (_, result) in &results {
            if let Ok(name) = result {
                self.record_change(instance_id, ChangeKind::ContentAdded, name);
            }
        }
        Ok(results)
    }

    /// Every version of a project, newest first, for choosing one to switch
    /// an installed file to (IA P-VERSION-SWITCH).
    pub async fn project_versions(&self, project: &str) -> Result<Vec<Version>, ServiceError> {
        let mut versions = self
            .modrinth()
            .await?
            .versions_with_changelog(project)
            .await
            .map_err(|error| ServiceError::Remote(error.to_string()))?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }

    /// Where an instance's game files live, for showing them in the file
    /// manager. Read-only: nothing is created.
    #[must_use]
    pub fn game_dir(&self, id: &str) -> std::path::PathBuf {
        self.layout.game(id)
    }

    /// The installed items of one kind. Reading needs no instance lease.
    pub async fn content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<ContentItem>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || scan_content(&game_dir, kind))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Lists one kind of content with where each file came from (IA
    /// P-CONTENT-ITEM). Files are hashed off the async threads; Modrinth is
    /// asked in three batches (versions by hash, projects, teams) plus one
    /// for the newest compatible versions. If it cannot be reached the list
    /// still comes back, marked `sources_unavailable`.
    pub async fn content_details(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<crate::content_sources::ContentList, ServiceError> {
        let record = self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let files = tokio::task::spawn_blocking(move || {
            let items = scan_content(&game_dir, kind)?;
            let folder = kind.install_folder().unwrap_or_default();
            Ok::<_, crate::content::ContentError>(
                items
                    .into_iter()
                    .map(|item| {
                        let hash = (!item.is_directory)
                            .then(|| {
                                crate::content::sha1_hex(
                                    &game_dir.join(folder).join(&item.file_name),
                                )
                                .ok()
                            })
                            .flatten();
                        (item, hash)
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)?;

        let hashes: Vec<String> = files.iter().filter_map(|(_, hash)| hash.clone()).collect();
        let enabled: Vec<String> = files
            .iter()
            .filter(|(item, _)| item.enabled)
            .filter_map(|(_, hash)| hash.clone())
            .collect();
        let unknown = |files| crate::content_sources::ContentList {
            entries: crate::content_sources::assemble(
                files,
                &BTreeMap::new(),
                &[],
                &BTreeMap::new(),
                &BTreeMap::new(),
            ),
            sources_unavailable: true,
        };
        let Ok(client) = self.modrinth().await else {
            return Ok(unknown(files));
        };
        let (identified, latest) = tokio::join!(
            client.identify(&hashes),
            client.latest_for(&enabled, record.loader, &record.game_version)
        );
        let Ok(identified) = identified else {
            return Ok(unknown(files));
        };
        let mut project_ids: Vec<String> =
            identified.values().map(|v| v.project_id.clone()).collect();
        project_ids.sort();
        project_ids.dedup();
        // Labels are decoration: a failure here leaves titles to the files.
        let projects = client
            .project_summaries(&project_ids)
            .await
            .unwrap_or_default();
        let mut teams: Vec<String> = projects.iter().filter_map(|p| p.team.clone()).collect();
        teams.sort();
        teams.dedup();
        let authors = client.team_authors(&teams).await.unwrap_or_default();
        Ok(crate::content_sources::ContentList {
            entries: crate::content_sources::assemble(
                files,
                &identified,
                &projects,
                &authors,
                &latest.unwrap_or_default(),
            ),
            sources_unavailable: false,
        })
    }

    /// Enables or disables several files, one result each. Writes need the
    /// instance lease, so they are refused while it launches or runs.
    pub async fn set_content_state(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
        enabled: bool,
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.change_content(id, kind, file_names, move |game_dir, kind, name| {
            let renamed = crate::content::set_enabled(game_dir, kind, name, enabled)?;
            if renamed == name {
                return Ok((ContentEffect::Unchanged, None));
            }
            let change = if enabled {
                ChangeKind::ContentEnabled
            } else {
                ChangeKind::ContentDisabled
            };
            let subject = renamed
                .strip_suffix(".disabled")
                .unwrap_or(&renamed)
                .to_owned();
            Ok((ContentEffect::Renamed(renamed), Some((change, subject))))
        })
        .await
    }

    /// Deletes several files (folder packs recursively), one result each.
    pub async fn delete_content(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
    ) -> Result<Vec<ContentResult>, ServiceError> {
        self.change_content(id, kind, file_names, |game_dir, kind, name| {
            crate::content::remove(game_dir, kind, name)?;
            Ok((
                ContentEffect::Removed,
                Some((ChangeKind::ContentRemoved, name.to_owned())),
            ))
        })
        .await
    }

    pub(super) async fn change_content<F>(
        &self,
        id: &str,
        kind: ProjectKind,
        file_names: &[String],
        apply: F,
    ) -> Result<Vec<ContentResult>, ServiceError>
    where
        F: Fn(
                &std::path::Path,
                ProjectKind,
                &str,
            ) -> Result<(ContentEffect, Option<(ChangeKind, String)>), ContentError>
            + Send
            + Clone
            + 'static,
    {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let mut results = Vec::with_capacity(file_names.len());
        for name in file_names {
            let (path, file) = (game_dir.clone(), name.clone());
            let apply = apply.clone();
            let applied = tokio::task::spawn_blocking(move || apply(&path, kind, &file))
                .await
                .map_err(std::io::Error::other)?;
            results.push(match applied {
                Ok((effect, change)) => {
                    let recorded = match change {
                        Some((change, subject)) => HistoryLog::for_instance(self.layout.root(), id)
                            .append(&HistoryEvent::Change {
                                at: now(),
                                kind: change,
                                subject,
                            })
                            .is_ok(),
                        None => true,
                    };
                    ContentResult {
                        file_name: name.clone(),
                        outcome: Ok(effect),
                        recorded,
                    }
                }
                Err(error) => ContentResult {
                    file_name: name.clone(),
                    outcome: Err(error),
                    recorded: true,
                },
            });
        }
        Ok(results)
    }
}
