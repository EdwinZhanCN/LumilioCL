use super::InstanceLibrary;
use crate::blocking::blocking;
use crate::clock::now;
use crate::error::ServiceError;
use crate::instances::deletion::Deletion;
use crate::instances::{InstanceRecord, Loader, NewInstance};
use crate::launching::LaunchServiceError;
use crate::minecraft::loader;
use crate::minecraft::loader::LAUNCHABLE_LOADERS;
use crate::transfer::Transport;

impl<T: Transport + Clone> InstanceLibrary<T> {
    /// Deletes an instance so that a failure or crash at any step can be
    /// undone or finished: journal, move the profile aside, commit the library,
    /// then remove the moved files. `meta/` is untouched because other
    /// instances may share it. If only the final cleanup fails the deletion
    /// still stands and the leftovers are removed at the next start.
    pub(crate) async fn delete_instance(&self, id: &str) -> Result<(), ServiceError> {
        let _lease = self.instances.reserve(id)?;
        let record = self
            .instances
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
        let layout = self.layout.clone();
        let deletion = {
            let record = record.clone();
            blocking(move || {
                let mut deletion = Deletion::begin(&layout, &record, now())?;
                match deletion.quarantine() {
                    Ok(()) => Ok(deletion),
                    Err(error) => {
                        deletion.discard();
                        Err(error)
                    }
                }
            })
            .await?
        };
        let committed = self.instances.lock().await.remove(id);
        if let Err(error) = committed {
            let restored = blocking(move || deletion.rollback()).await;
            // A failed restore keeps the journal; the library error stays the cause.
            drop(restored);
            return Err(error.into());
        }
        // The deletion is committed; leftovers are retried by recovery.
        let _ = blocking(move || deletion.finish()).await;
        // A failure of the deleted game has nothing left to recover.
        self.context.session(id, None);
        crate::settings::settle_current(&self.instances, &self.context, &self.settings).await;
        Ok(())
    }

    /// Creates an instance. A missing game version means the newest release;
    /// a missing loader version means the recommended one for that game.
    pub(crate) async fn create_instance(
        &self,
        name: &str,
        game_version: Option<&str>,
        loader: Loader,
        loader_version: Option<&str>,
    ) -> Result<InstanceRecord, ServiceError> {
        if !LAUNCHABLE_LOADERS.contains(&loader) {
            return Err(ServiceError::Launch(LaunchServiceError::LoaderUnsupported(
                loader,
            )));
        }
        let chain = self.network.chain().await?;
        let game_version = match game_version {
            Some(version) => version.to_owned(),
            None => crate::minecraft::fetch_catalog(&self.network.fetcher(), &chain)
                .await
                .map_err(|error| ServiceError::Remote(error.to_string()))?
                .latest_release()
                .map(|entry| entry.id().to_owned())
                .ok_or_else(|| ServiceError::Remote("the catalog lists no release".to_owned()))?,
        };
        let loader_version = match (loader, loader_version) {
            (Loader::Vanilla, _) => None,
            (_, Some(version)) => Some(version.to_owned()),
            (_, None) => {
                let url = loader::versions_url(loader, &game_version)
                    .ok_or_else(|| ServiceError::Remote("no loader source".to_owned()))?;
                let sources = chain.candidates(&url);
                let versions = loader::fetch_versions(&self.network.fetcher(), &sources)
                    .await
                    .map_err(|error| ServiceError::Remote(error.to_string()))?;
                let chosen = loader::recommended(&versions).ok_or_else(|| {
                    ServiceError::Remote(format!("{game_version} has no {loader:?} version"))
                })?;
                Some(chosen.version.clone())
            }
        };
        let record = self
            .instances
            .lock()
            .await
            .create(
                NewInstance {
                    name: name.to_owned(),
                    game_version,
                    loader,
                    loader_version,
                },
                now(),
            )?
            .clone();
        // Made at once, so a missing folder later means it was moved or
        // deleted, not that the game was never started.
        if let Err(error) = std::fs::create_dir_all(self.layout.game(&record.id)) {
            let _ = self.instances.lock().await.remove(&record.id);
            return Err(error.into());
        }
        crate::settings::settle_current(&self.instances, &self.context, &self.settings).await;
        Ok(record)
    }

    /// Gives a game whose folder was moved or deleted a new, empty one: it
    /// starts again with no mods, worlds or options of its own.
    pub(crate) async fn new_game_folder(&self, id: &str) -> Result<(), ServiceError> {
        self.instances.record(id).await?;
        let game = self.layout.game(id);
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(game))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }
}
