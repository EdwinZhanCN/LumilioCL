use std::path::Path;

use super::LauncherService;
use crate::activity::CancellationToken;
use crate::error::ServiceError;
use crate::instances::snapshot_files::{SnapshotInfo, SnapshotScope};
use crate::instances::{InstanceRecord, Library, Loader};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn library(&self) -> Library {
        self.library.library().await
    }

    pub async fn instance(&self, id: &str) -> Result<InstanceRecord, ServiceError> {
        self.library.instance(id).await
    }

    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), ServiceError> {
        self.library.set_favorite(id, favorite).await
    }

    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, ServiceError> {
        self.library.toggle_favorite(id).await
    }

    pub async fn create_collection(&self, name: &str) -> Result<(), ServiceError> {
        self.library.create_collection(name).await
    }

    pub async fn rename_collection(&self, from: &str, to: &str) -> Result<(), ServiceError> {
        self.library.rename_collection(from, to).await
    }

    pub async fn delete_collection(&self, name: &str) -> Result<(), ServiceError> {
        self.library.delete_collection(name).await
    }

    pub async fn set_game_collections(
        &self,
        id: &str,
        collections: &[String],
    ) -> Result<(), ServiceError> {
        self.library.set_game_collections(id, collections).await
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<(), ServiceError> {
        self.library.rename(id, name).await
    }

    pub async fn delete_instance(&self, id: &str) -> Result<(), ServiceError> {
        self.library.delete_instance(id).await
    }

    pub async fn create_instance(
        &self,
        name: &str,
        game_version: Option<&str>,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library
            .create_instance(name, game_version, loader, loader_version)
            .await
    }

    pub async fn new_game_folder(&self, id: &str) -> Result<(), ServiceError> {
        self.library.new_game_folder(id).await
    }

    pub async fn snapshots(&self, id: &str) -> Result<Vec<SnapshotInfo>, ServiceError> {
        self.library.snapshots(id).await
    }

    pub async fn create_snapshot(
        &self,
        id: &str,
        scope: SnapshotScope,
        label: &str,
    ) -> Result<String, ServiceError> {
        self.library.create_snapshot(id, scope, label).await
    }

    pub async fn restore_snapshot(
        &self,
        id: &str,
        snapshot: &str,
    ) -> Result<Vec<std::path::PathBuf>, ServiceError> {
        self.library.restore_snapshot(id, snapshot).await
    }

    pub async fn delete_snapshot(&self, id: &str, snapshot: &str) -> Result<(), ServiceError> {
        self.library.delete_snapshot(id, snapshot).await
    }

    pub async fn detect_games(
        &self,
        folder: &Path,
    ) -> Result<Vec<crate::instances::import_game::FoundGame>, ServiceError> {
        self.library.detect_games(folder).await
    }

    pub async fn import_game(
        &self,
        game: crate::instances::import_game::FoundGame,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library.import_game(game, cancel).await
    }

    pub async fn backup_instance(
        &self,
        id: &str,
        destination: &Path,
        cancel: CancellationToken,
    ) -> Result<u64, ServiceError> {
        self.library.backup_instance(id, destination, cancel).await
    }

    pub async fn restore_backup(
        &self,
        archive: &Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library.restore_backup(archive, cancel).await
    }

    pub async fn copy_instance(
        &self,
        source_id: &str,
        new_name: &str,
        include_worlds: bool,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library
            .copy_instance(source_id, new_name, include_worlds, cancel)
            .await
    }

    pub async fn install_modpack(
        &self,
        source: &str,
        project: &str,
        version: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library
            .install_modpack(&self.content, source, project, version, cancel)
            .await
    }

    pub async fn import_modpack_file(
        &self,
        pack: &std::path::Path,
        cancel: CancellationToken,
    ) -> Result<InstanceRecord, ServiceError> {
        self.library
            .import_modpack_file(&self.content, pack, cancel)
            .await
    }
}
