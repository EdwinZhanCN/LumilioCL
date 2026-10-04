//! Checking installed content for newer versions, and applying them.
//!
//! Behavior notes: `docs/behavior/updates.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::Path;

use crate::activity::CancellationToken;
use crate::content::{self, ContentError};
use crate::discover::{
    DiscoverError, IntentError, ModrinthClient, ProjectKind, Version, install_request,
};
use crate::instance::Loader;
use crate::transfer::{TransferEngine, TransferError, Transport};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentUpdate {
    pub kind: ProjectKind,
    /// The installed file that would be replaced.
    pub file_name: String,
    pub current_sha1: String,
    pub latest: Version,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UpdateReport {
    pub updates: Vec<ContentUpdate>,
    /// Files Modrinth knows that are already the newest compatible version.
    pub up_to_date: usize,
    /// Enabled files Modrinth does not know (hand-installed or from elsewhere).
    pub unknown: Vec<String>,
}

#[derive(Debug)]
pub enum UpdateError {
    Content(ContentError),
    Discover(DiscoverError),
    Intent(IntentError),
    Transfer(TransferError),
}

impl Display for UpdateError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Content(error) => write!(f, "{error}"),
            Self::Discover(error) => write!(f, "{error}"),
            Self::Intent(error) => write!(f, "{error}"),
            Self::Transfer(error) => write!(f, "{error}"),
        }
    }
}

impl Error for UpdateError {}

impl From<ContentError> for UpdateError {
    fn from(error: ContentError) -> Self {
        Self::Content(error)
    }
}
impl From<DiscoverError> for UpdateError {
    fn from(error: DiscoverError) -> Self {
        Self::Discover(error)
    }
}
impl From<IntentError> for UpdateError {
    fn from(error: IntentError) -> Self {
        Self::Intent(error)
    }
}
impl From<TransferError> for UpdateError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error)
    }
}

/// Checks the enabled files of one kind against Modrinth.
///
/// Disabled files and folder packs are left out (a disabled file is the user's
/// decision; a folder has no single hash). A file whose newest compatible
/// version lists the file's own hash is up to date. Files that cannot be read
/// are skipped.
pub async fn check<T: Transport>(
    client: &ModrinthClient<T>,
    game_dir: &Path,
    kind: ProjectKind,
    game_version: &str,
    loader: Loader,
) -> Result<UpdateReport, UpdateError> {
    let folder = kind.install_folder().ok_or(ContentError::NotAFileKind)?;
    let mut by_hash: Vec<(String, String)> = Vec::new();
    for item in content::scan(game_dir, kind)? {
        if !item.enabled || item.is_directory {
            continue;
        }
        if let Ok(hash) = content::sha1_hex(&game_dir.join(folder).join(&item.file_name)) {
            by_hash.push((hash, item.file_name));
        }
    }
    let hashes: Vec<String> = by_hash.iter().map(|(hash, _)| hash.clone()).collect();
    let known = client.identify(&hashes).await?;
    let latest = client.latest_for(&hashes, loader, game_version).await?;

    let mut report = UpdateReport::default();
    for (hash, file_name) in by_hash {
        if !known.contains_key(&hash) {
            report.unknown.push(file_name);
            continue;
        }
        match latest.get(&hash) {
            Some(newest)
                if !newest
                    .files
                    .iter()
                    .any(|file| file.sha1.as_deref() == Some(&hash)) =>
            {
                report.updates.push(ContentUpdate {
                    kind,
                    file_name,
                    current_sha1: hash,
                    latest: newest.clone(),
                });
            }
            _ => report.up_to_date += 1,
        }
    }
    Ok(report)
}

/// Downloads the new file (verified), then removes the old one. If the new
/// version has the same file name, the download replaces it in place. Nothing
/// is removed unless the download succeeded. Returns the new file name.
pub async fn apply<T: Transport>(
    engine: &TransferEngine<T>,
    update: &ContentUpdate,
    game_dir: &Path,
    sources: Vec<String>,
    cancel: CancellationToken,
) -> Result<String, UpdateError> {
    let request = install_request(update.kind, &update.latest, game_dir, sources)?;
    let new_name = update
        .latest
        .install_file()
        .map(|file| file.filename.clone())
        .ok_or(IntentError::NoFile)?;
    engine.transfer(request, cancel).await?;
    if new_name != update.file_name {
        content::remove(game_dir, update.kind, &update.file_name)?;
    }
    Ok(new_name)
}

#[cfg(test)]
mod tests;
