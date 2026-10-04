use super::LauncherService;
use super::error::ServiceError;
use super::support::blocking_value;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    /// How much of the shared game files no game uses (ADR 0017). Reads only;
    /// walks the disk.
    pub async fn reclaimable(&self) -> Result<crate::reclaim::Reclaimable, ServiceError> {
        let instances = self.store.lock().await.instances().to_vec();
        let layout = self.layout.clone();
        blocking_value(move || crate::reclaim::scan(&layout, &instances))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// Removes the shared game files no game uses and returns the bytes freed.
    /// Refuses while anything is installing, repairing, launching or otherwise
    /// in use, then looks again before deleting, so what was shown a moment
    /// ago cannot remove something a game has started to need.
    pub async fn reclaim(&self) -> Result<u64, ServiceError> {
        let busy = {
            let operations = self
                .operations
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            operations.iter().next().cloned()
        };
        if let Some(busy) = busy {
            return Err(ServiceError::InstanceBusy(busy));
        }
        // Held to the end: nothing else can start meanwhile.
        let _lease = self.reserve_instance("(shared files)")?;
        let found = self.reclaimable().await?;
        let layout = self.layout.clone();
        blocking_value(move || crate::reclaim::remove(&layout, &found))
            .await?
            .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// How much disk each part of the launcher's folder uses. Walks the disk
    /// on a worker thread; reading needs no lease.
    pub async fn storage_usage(&self) -> crate::storage::StorageUsage {
        let layout = self.layout.clone();
        tokio::task::spawn_blocking(move || crate::storage::measure(&layout))
            .await
            .unwrap_or_default()
    }

    /// Deletes extracted natives and the cache folder, returning the bytes
    /// freed. Natives are rebuilt by the next start that needs them. Where the
    /// system will not delete a file a running game still holds, this reports
    /// the error and the rest stays as it was.
    pub async fn clear_cache(&self) -> Result<u64, ServiceError> {
        let layout = self.layout.clone();
        let freed = tokio::task::spawn_blocking(move || crate::storage::clear_cache(&layout))
            .await
            .map_err(std::io::Error::other)??;
        Ok(freed)
    }
}
