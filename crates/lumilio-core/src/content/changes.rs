use super::Content;
use super::{ContentEffect, ContentResult};
use crate::clock::now;
use crate::content::{ContentError, ContentItem, scan as scan_content};
use crate::discover::{ProjectKind, Version};
use crate::error::ServiceError;
use crate::history::{ChangeKind, HistoryEvent, HistoryLog};
use crate::transfer::Transport;

impl<T: Transport + Clone> Content<T> {
    /// Copies local files into an instance (IA P-ADD-FILES): one result per
    /// file, so one refusal does not hide the others. Needs the instance lease.
    pub(crate) async fn import_content(
        &self,
        instance_id: &str,
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    ) -> Result<Vec<(String, Result<String, ContentError>)>, ServiceError> {
        let _lease = self.instances.reserve(instance_id)?;
        self.instances.record(instance_id).await?;
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
                crate::history::record_change(
                    self.layout.root(),
                    instance_id,
                    ChangeKind::ContentAdded,
                    name,
                );
            }
        }
        Ok(results)
    }

    /// Every version of a project, newest first, for choosing one to switch
    /// an installed file to (IA P-VERSION-SWITCH).
    pub(crate) async fn project_versions(
        &self,
        source: &str,
        project: &str,
    ) -> Result<Vec<Version>, ServiceError> {
        let mut versions = self
            .content_client(source)
            .await?
            .versions_with_changelog(project)
            .await?;
        versions.sort_by(|a, b| b.published.cmp(&a.published));
        Ok(versions)
    }

    /// Where an instance's game files live, for showing them in the file
    /// manager. Read-only: nothing is created.
    #[must_use]
    pub(crate) fn game_dir(&self, id: &str) -> std::path::PathBuf {
        self.layout.game(id)
    }

    /// The installed items of one kind. Reading needs no instance lease.
    pub(crate) async fn content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<ContentItem>, ServiceError> {
        self.instances.record(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || scan_content(&game_dir, kind))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Lists one kind of content with where each file came from (IA
    /// P-CONTENT-ITEM). Files are hashed off the async threads, then the
    /// enabled sources are asked in turn (see `recognize`). If a source
    /// cannot be reached the list still comes back, marked
    /// `sources_unavailable`.
    pub(crate) async fn content_details(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<crate::content::ContentList, ServiceError> {
        let record = self.instances.record(id).await?;
        let files = self.hashed_content(id, kind).await?;
        let found = self
            .recognize(&files.iter().collect::<Vec<_>>(), Some(&record))
            .await;
        let note = (!found.failures.is_empty()).then(|| found.failures.join("; "));
        Ok(crate::content::ContentList {
            entries: crate::content::assemble(
                files
                    .into_iter()
                    .map(|file| (file.item, file.sha1))
                    .collect(),
                &found.identified,
                &found.projects,
                &found.latest,
            ),
            sources_unavailable: note.is_some(),
            source_note: note,
        })
    }

    /// Enables or disables several files, one result each. Writes need the
    /// instance lease, so they are refused while it launches or runs.
    pub(crate) async fn set_content_state(
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
    pub(crate) async fn delete_content(
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

    pub(crate) async fn change_content<F>(
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
        let _lease = self.instances.reserve(id)?;
        self.instances.record(id).await?;
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
