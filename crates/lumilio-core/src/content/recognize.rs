//! Which project each installed file is, asking the enabled content sources
//! in turn (plan curseforge-source D4): a file belongs to the first source
//! that knows it, and only files still unknown go on to the next one. What a
//! source said is kept, and asked again only when it expires (plan
//! content-recognition D2).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::Content;
use super::recognition_cache::{
    ContentLookup, DETAILS_FOR, KNOWN_FOR, Stored, UNKNOWN_FOR, identify_key, latest_key,
    project_key,
};
use super::{Recognized, Summaries};
use crate::clock::now;
use crate::content::discover::{ContentClient, FileHashes, ProjectKind, ProjectSummary, Version};
use crate::content::{ContentItem, scan as scan_content};
use crate::error::ServiceError;
use crate::instances::InstanceRecord;
use crate::transfer::Transport;

/// One installed file: what the list shows, where it is, and its SHA-1
/// (`None` for folder packs and unreadable files).
pub(crate) struct HashedFile {
    pub(super) item: ContentItem,
    pub(super) path: PathBuf,
    pub(super) sha1: Option<String>,
}

/// What the sources said about a game's files.
#[derive(Default)]
pub(crate) struct Recognition {
    /// By SHA-1.
    pub(super) identified: BTreeMap<String, Recognized>,
    /// The newest compatible version, by SHA-1, from the source that knew
    /// the file; only for files asked about.
    pub(super) latest: BTreeMap<String, Version>,
    pub(super) projects: Summaries,
    /// Sources that could not be asked, with why.
    pub(super) failures: Vec<String>,
}

/// Which of `keys` a lookup takes from what is kept, and which it asks.
/// `Kept` asks nothing; `Again` asks everything.
fn split<T: Clone>(
    keys: &[String],
    kept: &HashMap<String, Stored<T>>,
    lookup: ContentLookup,
) -> (Vec<(usize, T)>, Vec<usize>) {
    let mut known = Vec::new();
    let mut ask = Vec::new();
    for (index, key) in keys.iter().enumerate() {
        match kept.get(key) {
            Some(stored)
                if lookup == ContentLookup::Kept
                    || (lookup == ContentLookup::Fresh && stored.fresh) =>
            {
                known.push((index, stored.value.clone()));
            }
            _ if lookup == ContentLookup::Kept => {}
            _ => ask.push(index),
        }
    }
    (known, ask)
}

impl<T: Transport + Clone> Content<T> {
    /// The installed files of one kind, hashed off the async threads; a file
    /// unchanged since it was last hashed keeps its kept SHA-1.
    pub(crate) async fn hashed_content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<HashedFile>, ServiceError> {
        let game_dir = self.layout.game(id);
        let cache = self.recognition.clone();
        tokio::task::spawn_blocking(move || {
            let items = scan_content(&game_dir, kind)?;
            let folder = game_dir.join(kind.install_folder().unwrap_or_default());
            let files: Vec<PathBuf> = items
                .iter()
                .filter(|item| !item.is_directory)
                .map(|item| folder.join(&item.file_name))
                .collect();
            let mut sha1s = cache.sha1s(&files, now()).into_iter();
            Ok::<_, crate::content::ContentError>(
                items
                    .into_iter()
                    .map(|item| {
                        let path = folder.join(&item.file_name);
                        let sha1 = if item.is_directory {
                            None
                        } else {
                            sha1s.next().flatten()
                        };
                        HashedFile { item, path, sha1 }
                    })
                    .collect(),
            )
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }

    /// Each file's hashes as `client` reads them; CurseForge's fingerprint
    /// is computed (off the async threads, kept by SHA-1) only for a source
    /// that needs it.
    pub(crate) async fn file_hashes(
        &self,
        client: &ContentClient<'_>,
        files: &[(String, PathBuf)],
    ) -> Vec<FileHashes> {
        if !client.wants_murmur2() {
            return files
                .iter()
                .map(|(sha1, _)| FileHashes::sha1(sha1.clone()))
                .collect();
        }
        let files = files.to_vec();
        let cache = self.recognition.clone();
        tokio::task::spawn_blocking(move || {
            let fingerprints = cache.fingerprints(&files, now());
            files
                .into_iter()
                .zip(fingerprints)
                .map(|((sha1, _), murmur2)| FileHashes { murmur2, sha1 })
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    /// Asks every enabled source, in order, about the files the ones before
    /// it did not know, and (when `record` is given) for the newest version
    /// of the enabled ones that fits the game. A source that cannot answer is
    /// noted and skipped; the next one may still know the files, and what it
    /// said before still stands.
    pub(crate) async fn recognize(
        &self,
        files: &[&HashedFile],
        record: Option<&InstanceRecord>,
        lookup: ContentLookup,
    ) -> Recognition {
        let mut found = Recognition::default();
        let clients = self.plugins.content_clients().await;
        if clients.is_empty() {
            let why = match self.plugins.content_client().await {
                Err(error) => error.to_string(),
                Ok(_) => crate::content::discover::DiscoverError::NoSource.to_string(),
            };
            found.failures.push(why);
            return found;
        }
        let mut unknown: Vec<(String, PathBuf, bool)> = files
            .iter()
            .filter_map(|file| Some((file.sha1.clone()?, file.path.clone(), file.item.enabled)))
            .collect();
        unknown.sort_by(|a, b| a.0.cmp(&b.0));
        unknown.dedup_by(|a, b| a.0 == b.0);
        for client in clients {
            if unknown.is_empty() {
                break;
            }
            let asked: Vec<(String, PathBuf)> = unknown
                .iter()
                .map(|(sha1, path, _)| (sha1.clone(), path.clone()))
                .collect();
            let (identified, failure) = self.identified(&client, &asked, lookup).await;
            if let Some(failure) = failure {
                found.failures.push(format!("{}: {failure}", client.name()));
            }
            if identified.is_empty() {
                continue;
            }
            let enabled: Vec<(String, PathBuf)> = unknown
                .iter()
                .filter(|(sha1, _, enabled)| *enabled && identified.contains_key(sha1))
                .map(|(sha1, path, _)| (sha1.clone(), path.clone()))
                .collect();
            let mut ids: Vec<String> = identified
                .values()
                .map(|version| version.project_id.clone())
                .collect();
            ids.sort();
            ids.dedup();
            let latest = async {
                match record {
                    Some(record) => self.newest(&client, &enabled, record, lookup).await,
                    None => BTreeMap::new(),
                }
            };
            // Labels and updates are decoration: a failure leaves them out.
            let (latest, summaries) = tokio::join!(latest, self.summaries(&client, &ids, lookup));
            found.latest.extend(latest);
            for summary in summaries {
                found
                    .projects
                    .insert((client.plugin_id().to_owned(), summary.id.clone()), summary);
            }
            unknown.retain(|(sha1, _, _)| !identified.contains_key(sha1));
            for (sha1, version) in identified {
                found.identified.insert(
                    sha1,
                    Recognized {
                        source: client.plugin_id().to_owned(),
                        source_name: client.name().to_owned(),
                        version,
                    },
                );
            }
        }
        found
    }

    /// The version each file is in `client`'s catalog, by SHA-1, and why the
    /// source could not be asked when it could not.
    async fn identified(
        &self,
        client: &ContentClient<'_>,
        files: &[(String, PathBuf)],
        lookup: ContentLookup,
    ) -> (BTreeMap<String, Version>, Option<String>) {
        let source = client.plugin_id();
        let keys: Vec<String> = files
            .iter()
            .map(|(sha1, _)| identify_key(source, sha1))
            .collect();
        let kept = self
            .recognition
            .recall_now::<Option<Version>>(keys.clone())
            .await;
        let (known, ask) = split(&keys, &kept, lookup);
        let mut found: BTreeMap<String, Version> = known
            .into_iter()
            .filter_map(|(index, version)| Some((files[index].0.clone(), version?)))
            .collect();
        if ask.is_empty() {
            return (found, None);
        }
        let asked: Vec<(String, PathBuf)> = ask.iter().map(|&index| files[index].clone()).collect();
        let hashes = self.file_hashes(client, &asked).await;
        match client.identify(&hashes).await {
            Ok(answer) => {
                let keep = ask
                    .iter()
                    .map(|&index| {
                        let version = answer.get(&files[index].0).cloned();
                        let fresh_for = if version.is_some() {
                            KNOWN_FOR
                        } else {
                            UNKNOWN_FOR
                        };
                        (keys[index].clone(), version, fresh_for)
                    })
                    .collect();
                self.recognition.keep_now(keep).await;
                found.extend(answer);
                (found, None)
            }
            Err(error) => {
                for &index in &ask {
                    if let Some(Stored {
                        value: Some(version),
                        ..
                    }) = kept.get(&keys[index])
                    {
                        found.insert(files[index].0.clone(), version.clone());
                    }
                }
                (found, Some(error.to_string()))
            }
        }
    }

    /// The newest version of each file that fits `record`'s game, by SHA-1.
    async fn newest(
        &self,
        client: &ContentClient<'_>,
        files: &[(String, PathBuf)],
        record: &InstanceRecord,
        lookup: ContentLookup,
    ) -> BTreeMap<String, Version> {
        let source = client.plugin_id();
        let keys: Vec<String> = files
            .iter()
            .map(|(sha1, _)| latest_key(source, sha1, record.loader, &record.game_version))
            .collect();
        let kept = self
            .recognition
            .recall_now::<Option<Version>>(keys.clone())
            .await;
        let (known, ask) = split(&keys, &kept, lookup);
        let mut found: BTreeMap<String, Version> = known
            .into_iter()
            .filter_map(|(index, version)| Some((files[index].0.clone(), version?)))
            .collect();
        if ask.is_empty() {
            return found;
        }
        let asked: Vec<(String, PathBuf)> = ask.iter().map(|&index| files[index].clone()).collect();
        let hashes = self.file_hashes(client, &asked).await;
        match client
            .latest_for(&hashes, record.loader, &record.game_version)
            .await
        {
            Ok(answer) => {
                let keep = ask
                    .iter()
                    .map(|&index| {
                        let newest = answer.get(&files[index].0).cloned();
                        (keys[index].clone(), newest, DETAILS_FOR)
                    })
                    .collect();
                self.recognition.keep_now(keep).await;
                found.extend(answer);
            }
            Err(_) => {
                for &index in &ask {
                    if let Some(Stored {
                        value: Some(newest),
                        ..
                    }) = kept.get(&keys[index])
                    {
                        found.insert(files[index].0.clone(), newest.clone());
                    }
                }
            }
        }
        found
    }

    /// What `client` says about each project: name, author, icon, page.
    async fn summaries(
        &self,
        client: &ContentClient<'_>,
        ids: &[String],
        lookup: ContentLookup,
    ) -> Vec<ProjectSummary> {
        let source = client.plugin_id();
        let keys: Vec<String> = ids.iter().map(|id| project_key(source, id)).collect();
        let kept = self
            .recognition
            .recall_now::<ProjectSummary>(keys.clone())
            .await;
        let (known, ask) = split(&keys, &kept, lookup);
        let mut found: Vec<ProjectSummary> =
            known.into_iter().map(|(_, summary)| summary).collect();
        if ask.is_empty() {
            return found;
        }
        let asked: Vec<String> = ask.iter().map(|&index| ids[index].clone()).collect();
        match client.project_summaries(&asked).await {
            Ok(answer) => {
                let keep = answer
                    .iter()
                    .map(|summary| {
                        (
                            project_key(source, &summary.id),
                            summary.clone(),
                            DETAILS_FOR,
                        )
                    })
                    .collect();
                self.recognition.keep_now(keep).await;
                found.extend(answer);
            }
            Err(_) => found.extend(
                ask.iter()
                    .filter_map(|&index| kept.get(&keys[index]))
                    .map(|stored| stored.value.clone()),
            ),
        }
        found
    }
}

#[cfg(test)]
mod tests;
