use super::InstanceLibrary;
use crate::clock::now;
use crate::error::ServiceError;
use crate::history::ChangeKind;
use crate::snapshots;
use crate::snapshots::{SnapshotInfo, SnapshotScope};
use crate::transfer::Transport;

impl<T: Transport + Clone> InstanceLibrary<T> {
    /// The instance's snapshots, newest first. Reading needs no lease.
    pub(crate) async fn snapshots(&self, id: &str) -> Result<Vec<SnapshotInfo>, ServiceError> {
        self.instances.record(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || snapshots::list(&root, &id))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Backs up the instance's worlds and settings (or one world). Takes the
    /// instance lease so the files are not changing underneath it.
    pub(crate) async fn create_snapshot(
        &self,
        id: &str,
        scope: SnapshotScope,
        label: &str,
    ) -> Result<String, ServiceError> {
        let _lease = self.instances.reserve(id)?;
        self.instances.record(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, label) = (id.to_owned(), label.to_owned());
        let created = tokio::task::spawn_blocking(move || {
            snapshots::create(&root, &instance, &game_dir, scope, &label, now())
        })
        .await
        .map_err(std::io::Error::other)??;
        crate::history::record_change(
            self.layout.root(),
            id,
            ChangeKind::SnapshotCreated,
            &created,
        );
        Ok(created)
    }

    /// Replaces the snapshot's worlds and settings in the game. A failure or a
    /// crash puts everything back as it was; the snapshot stays for a retry.
    /// Returns the restored units. Takes the instance lease.
    pub(crate) async fn restore_snapshot(
        &self,
        id: &str,
        snapshot: &str,
    ) -> Result<Vec<std::path::PathBuf>, ServiceError> {
        let _lease = self.instances.reserve(id)?;
        self.instances.record(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, name) = (id.to_owned(), snapshot.to_owned());
        let units = tokio::task::spawn_blocking(move || {
            snapshots::restore(&root, &instance, &name, &game_dir)
        })
        .await
        .map_err(std::io::Error::other)??;
        crate::history::record_change(
            self.layout.root(),
            id,
            ChangeKind::SnapshotRestored,
            snapshot,
        );
        Ok(units)
    }

    /// Deletes one snapshot. Never part of automatic clean-up. Takes the lease.
    pub(crate) async fn delete_snapshot(
        &self,
        id: &str,
        snapshot: &str,
    ) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        self.instances.record(id).await?;
        let (root, instance, name) = (
            self.layout.root().to_path_buf(),
            id.to_owned(),
            snapshot.to_owned(),
        );
        tokio::task::spawn_blocking(move || snapshots::delete(&root, &instance, &name))
            .await
            .map_err(std::io::Error::other)??;
        crate::history::record_change(
            self.layout.root(),
            id,
            ChangeKind::SnapshotDeleted,
            snapshot,
        );
        Ok(())
    }
}
