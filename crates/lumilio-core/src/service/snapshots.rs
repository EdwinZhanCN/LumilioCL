use super::LauncherService;
use super::error::ServiceError;
use super::types::now;
use crate::history::{ChangeKind, HistoryEvent, HistoryLog};
use crate::snapshots;
use crate::snapshots::{SnapshotInfo, SnapshotScope};
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// The instance's snapshots, newest first. Reading needs no lease.
    pub async fn snapshots(&self, id: &str) -> Result<Vec<SnapshotInfo>, ServiceError> {
        self.instance(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || snapshots::list(&root, &id))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// Backs up the instance's worlds and settings (or one world). Takes the
    /// instance lease so the files are not changing underneath it.
    pub async fn create_snapshot(
        &self,
        id: &str,
        scope: SnapshotScope,
        label: &str,
    ) -> Result<String, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, label) = (id.to_owned(), label.to_owned());
        let created = tokio::task::spawn_blocking(move || {
            snapshots::create(&root, &instance, &game_dir, scope, &label, now())
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotCreated, &created);
        Ok(created)
    }

    /// Replaces the snapshot's worlds and settings in the game. A failure or a
    /// crash puts everything back as it was; the snapshot stays for a retry.
    /// Returns the restored units. Takes the instance lease.
    pub async fn restore_snapshot(
        &self,
        id: &str,
        snapshot: &str,
    ) -> Result<Vec<std::path::PathBuf>, ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, game_dir) = (self.layout.root().to_path_buf(), self.layout.game(id));
        let (instance, name) = (id.to_owned(), snapshot.to_owned());
        let units = tokio::task::spawn_blocking(move || {
            snapshots::restore(&root, &instance, &name, &game_dir)
        })
        .await
        .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotRestored, snapshot);
        Ok(units)
    }

    /// Deletes one snapshot. Never part of automatic clean-up. Takes the lease.
    pub async fn delete_snapshot(&self, id: &str, snapshot: &str) -> Result<(), ServiceError> {
        let _lease = self.reserve_instance(id)?;
        self.instance(id).await?;
        let (root, instance, name) = (
            self.layout.root().to_path_buf(),
            id.to_owned(),
            snapshot.to_owned(),
        );
        tokio::task::spawn_blocking(move || snapshots::delete(&root, &instance, &name))
            .await
            .map_err(std::io::Error::other)??;
        self.record_change(id, ChangeKind::SnapshotDeleted, snapshot);
        Ok(())
    }

    /// Notes a change that really happened. A history write failure must not
    /// turn a finished file operation into an error.
    pub(super) fn record_change(&self, id: &str, kind: ChangeKind, subject: &str) {
        let _ = HistoryLog::for_instance(self.layout.root(), id).append(&HistoryEvent::Change {
            at: now(),
            kind,
            subject: subject.to_owned(),
        });
    }
}
