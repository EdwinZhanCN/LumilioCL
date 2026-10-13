//! Files a game still needs that the launcher could not download: their
//! authors let people download them only on the source's site (plan
//! curseforge-source D3). The person downloads them and adds them here.

use std::path::PathBuf;

use super::LauncherService;
use super::error::ServiceError;
use crate::content::ContentError;
use crate::history::ChangeKind;
use crate::modpack::PendingDownload;
use crate::transfer::Transport;

/// What happened to one chosen file.
#[derive(Debug)]
pub enum AddedFile {
    /// Put in place; the path in the game directory.
    Placed(String),
    /// Not one of the files the game is waiting for.
    NotWanted,
    Failed(ContentError),
}

impl<T: Transport + Clone> LauncherService<T> {
    /// Where an instance's list of files to add by hand is kept.
    fn pending_path(&self, id: &str) -> std::path::PathBuf {
        self.layout.profile(id).join("pending-downloads.json")
    }

    pub(super) async fn save_pending_downloads(
        &self,
        id: &str,
        files: &[PendingDownload],
    ) -> Result<(), ServiceError> {
        let path = self.pending_path(id);
        if files.is_empty() {
            return match tokio::fs::remove_file(&path).await {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
                _ => Ok(()),
            };
        }
        let json = serde_json::to_vec_pretty(files).map_err(std::io::Error::other)?;
        let mut temporary = path.clone().into_os_string();
        temporary.push(".tmp");
        tokio::fs::write(&temporary, &json).await?;
        tokio::fs::rename(&temporary, &path).await?;
        Ok(())
    }

    /// Files the game still needs that the launcher could not download (a
    /// pack's files whose authors allow downloads only on the source's site).
    pub async fn pending_downloads(&self, id: &str) -> Result<Vec<PendingDownload>, ServiceError> {
        match tokio::fs::read(self.pending_path(id)).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    /// Puts files the person downloaded where the game needs them. Each is
    /// matched to a waiting file by its SHA-1, so a renamed download still
    /// counts and a different file never takes another's place. A file
    /// already there is not replaced unless it is the same. Needs the
    /// instance lease.
    pub async fn add_pending_files(
        &self,
        id: &str,
        files: Vec<PathBuf>,
    ) -> Result<Vec<(String, AddedFile)>, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let mut waiting = self.pending_downloads(id).await?;
        let game_dir = self.layout.game(id);
        let mut results = Vec::with_capacity(files.len());
        for file in files {
            let shown = file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| file.display().to_string());
            let hash = {
                let file = file.clone();
                tokio::task::spawn_blocking(move || crate::content::sha1_hex(&file))
                    .await
                    .map_err(std::io::Error::other)?
            };
            let Ok(hash) = hash else {
                results.push((shown, AddedFile::NotWanted));
                continue;
            };
            let Some(at) = waiting.iter().position(|pending| {
                !pending.path.is_empty()
                    && pending
                        .sha1
                        .as_deref()
                        .is_some_and(|sha1| sha1.eq_ignore_ascii_case(&hash))
            }) else {
                results.push((shown, AddedFile::NotWanted));
                continue;
            };
            let target = game_dir.join(&waiting[at].path);
            let placed = {
                let (from, to, hash) = (file.clone(), target.clone(), hash.clone());
                tokio::task::spawn_blocking(move || place(&from, &to, &hash))
                    .await
                    .map_err(std::io::Error::other)?
            };
            match placed {
                Ok(()) => {
                    let pending = waiting.remove(at);
                    self.record_change(id, ChangeKind::ContentAdded, &pending.file_name);
                    results.push((shown, AddedFile::Placed(pending.path)));
                }
                Err(error) => results.push((shown, AddedFile::Failed(error))),
            }
        }
        self.save_pending_downloads(id, &waiting).await?;
        Ok(results)
    }
}

/// Copies `from` to `to` unless a different file is already there.
fn place(from: &std::path::Path, to: &std::path::Path, sha1: &str) -> Result<(), ContentError> {
    if to.symlink_metadata().is_ok() {
        let same = to.is_file()
            && crate::content::sha1_hex(to)
                .is_ok_and(|existing| existing.eq_ignore_ascii_case(sha1));
        if same {
            return Ok(());
        }
        let name = to
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(ContentError::Conflict(name));
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(ContentError::from)?;
    }
    let mut temporary = to.as_os_str().to_owned();
    temporary.push(".part");
    std::fs::copy(from, &temporary).map_err(ContentError::from)?;
    std::fs::rename(&temporary, to).map_err(ContentError::from)
}
