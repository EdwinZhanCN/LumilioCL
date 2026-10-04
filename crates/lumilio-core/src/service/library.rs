use super::LauncherService;
use super::error::ServiceError;
use super::types::Library;
use crate::instance::InstanceRecord;
use crate::settings::LauncherSettings;
use crate::transfer::Transport;

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn library(&self) -> Library {
        let store = self.store.lock().await;
        Library {
            instances: store.instances().to_vec(),
            collections: store.collections().to_vec(),
        }
    }

    /// Reads the registered instance even when its profile folder is missing.
    pub async fn instance(&self, id: &str) -> Result<InstanceRecord, ServiceError> {
        self.store
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))
    }

    pub async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.set_favorite(id, favorite)?)
    }

    /// Flips the favorite flag and returns the new value.
    pub async fn toggle_favorite(&self, id: &str) -> Result<bool, ServiceError> {
        let mut store = self.store.lock().await;
        let favorite = !store
            .get(id)
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?
            .favorite;
        store.set_favorite(id, favorite)?;
        Ok(favorite)
    }

    pub async fn create_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.create_collection(name)?)
    }

    pub async fn rename_collection(&self, from: &str, to: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.rename_collection(from, to)?)
    }

    /// Removes the collection only; its games stay in the library.
    pub async fn delete_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.delete_collection(name)?)
    }

    /// Puts a game in exactly these collections.
    pub async fn set_game_collections(
        &self,
        id: &str,
        collections: &[String],
    ) -> Result<(), ServiceError> {
        Ok(self
            .store
            .lock()
            .await
            .set_collections_of(id, collections)?)
    }

    pub async fn rename(&self, id: &str, name: &str) -> Result<(), ServiceError> {
        Ok(self.store.lock().await.rename(id, name)?)
    }

    /// The last successfully saved launcher defaults and public account settings.
    pub async fn settings(&self) -> LauncherSettings {
        self.settings.lock().await.get().clone()
    }
}
