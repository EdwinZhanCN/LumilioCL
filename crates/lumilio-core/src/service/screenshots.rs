use super::LauncherService;
use super::error::ServiceError;
use crate::screenshots::{self, ScreenshotInfo};
use crate::transfer::Transport;
use std::path::PathBuf;
use tokio::sync::Semaphore;

/// Decoding a large picture takes memory: only a couple at a time, however
/// many thumbnails a page asks for at once.
static THUMBNAILS: Semaphore = Semaphore::const_new(2);

impl<T: Transport + Clone> LauncherService<T> {
    /// The instance's screenshots, newest first. Needs no instance lease: the
    /// game takes new ones while it runs.
    pub async fn screenshots(&self, id: &str) -> Result<Vec<ScreenshotInfo>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || screenshots::scan(&game_dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// The thumbnail file of one screenshot, made first if needed.
    pub async fn screenshot_thumbnail(
        &self,
        id: &str,
        file: &str,
    ) -> Result<PathBuf, ServiceError> {
        self.instance(id).await?;
        let (game_dir, cache_dir) = (self.layout.game(id), self.layout.thumbnails(id));
        let file = file.to_owned();
        let _turn = THUMBNAILS.acquire().await.map_err(std::io::Error::other)?;
        tokio::task::spawn_blocking(move || screenshots::thumbnail(&game_dir, &cache_dir, &file))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// The picture's bytes (to copy it).
    pub async fn screenshot_bytes(&self, id: &str, file: &str) -> Result<Vec<u8>, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let file = file.to_owned();
        tokio::task::spawn_blocking(move || screenshots::read(&game_dir, &file))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Deletes one screenshot. Allowed while the game runs: it never edits
    /// a screenshot it has taken.
    pub async fn delete_screenshot(&self, id: &str, file: &str) -> Result<(), ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        let file = file.to_owned();
        tokio::task::spawn_blocking(move || screenshots::delete(&game_dir, &file))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }
}
