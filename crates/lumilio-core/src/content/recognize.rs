//! Which project each installed file is, asking the enabled content sources
//! in turn (plan curseforge-source D4): a file belongs to the first source
//! that knows it, and only files still unknown go on to the next one.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::Content;
use super::{Recognized, Summaries};
use crate::content::{ContentItem, scan as scan_content};
use crate::discover::{ContentClient, FileHashes, ProjectKind, Version};
use crate::error::ServiceError;
use crate::instances::InstanceRecord;
use crate::transfer::Transport;

/// One installed file: what the list shows, where it is, and its SHA-1
/// (`None` for folder packs and unreadable files).
pub(crate) struct HashedFile {
    pub(crate) item: ContentItem,
    pub(crate) path: PathBuf,
    pub(crate) sha1: Option<String>,
}

/// What the sources said about a game's files.
#[derive(Default)]
pub(crate) struct Recognition {
    /// By SHA-1.
    pub(crate) identified: BTreeMap<String, Recognized>,
    /// The newest compatible version, by SHA-1, from the source that knew
    /// the file; only for files asked about.
    pub(crate) latest: BTreeMap<String, Version>,
    pub(crate) projects: Summaries,
    /// Sources that could not be asked, with why.
    pub(crate) failures: Vec<String>,
}

impl<T: Transport + Clone> Content<T> {
    /// The installed files of one kind, hashed off the async threads.
    pub(crate) async fn hashed_content(
        &self,
        id: &str,
        kind: ProjectKind,
    ) -> Result<Vec<HashedFile>, ServiceError> {
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || {
            let items = scan_content(&game_dir, kind)?;
            let folder = game_dir.join(kind.install_folder().unwrap_or_default());
            Ok::<_, crate::content::ContentError>(
                items
                    .into_iter()
                    .map(|item| {
                        let path = folder.join(&item.file_name);
                        let sha1 = (!item.is_directory)
                            .then(|| crate::content::sha1_hex(&path).ok())
                            .flatten();
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
    /// is computed (off the async threads) only for a source that needs it.
    pub(crate) async fn file_hashes(
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
        tokio::task::spawn_blocking(move || {
            files
                .into_iter()
                .map(|(sha1, path)| FileHashes {
                    murmur2: crate::fingerprint::of_file(&path).ok(),
                    sha1,
                })
                .collect()
        })
        .await
        .unwrap_or_default()
    }

    /// Asks every enabled source, in order, about the files the ones before
    /// it did not know, and (when `record` is given) for the newest version
    /// of the enabled ones that fits the game. A source that cannot answer is
    /// noted and skipped; the next one may still know the files.
    pub(crate) async fn recognize(
        &self,
        files: &[&HashedFile],
        record: Option<&InstanceRecord>,
    ) -> Recognition {
        let mut found = Recognition::default();
        let clients = self.plugins.content_clients().await;
        if clients.is_empty() {
            let why = match self.plugins.content_client().await {
                Err(error) => error.to_string(),
                Ok(_) => crate::discover::DiscoverError::NoSource.to_string(),
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
            let hashes = Self::file_hashes(&client, &asked).await;
            let identified = match client.identify(&hashes).await {
                Ok(identified) => identified,
                Err(error) => {
                    found.failures.push(format!("{}: {error}", client.name()));
                    continue;
                }
            };
            if identified.is_empty() {
                continue;
            }
            let enabled: Vec<FileHashes> = hashes
                .into_iter()
                .filter(|hash| identified.contains_key(&hash.sha1))
                .filter(|hash| {
                    unknown
                        .iter()
                        .any(|(sha1, _, enabled)| *enabled && *sha1 == hash.sha1)
                })
                .collect();
            let mut ids: Vec<String> = identified
                .values()
                .map(|version| version.project_id.clone())
                .collect();
            ids.sort();
            ids.dedup();
            let latest = async {
                match record {
                    Some(record) => client
                        .latest_for(&enabled, record.loader, &record.game_version)
                        .await
                        .unwrap_or_default(),
                    None => BTreeMap::new(),
                }
            };
            // Labels and updates are decoration: a failure leaves them out.
            let (latest, summaries) = tokio::join!(latest, client.project_summaries(&ids));
            found.latest.extend(latest);
            for summary in summaries.unwrap_or_default() {
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
}
