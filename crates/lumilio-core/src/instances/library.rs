use std::sync::Arc;

use tokio::sync::Mutex;

use super::{Collection, InstanceRecord, Instances};
use crate::activity::Tasks;
use crate::context::ContextHub;
use crate::error::ServiceError;
use crate::network::Network;
use crate::plugins::PluginHost;
use crate::settings::SettingsStore;
use crate::storage::Layout;
use crate::transfer::Transport;

/// What the library page shows.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Library {
    pub instances: Vec<InstanceRecord>,
    pub collections: Vec<Collection>,
}

/// The games a person owns: making, copying, importing, backing up and
/// deleting them, their collections and snapshots.
pub(crate) struct InstanceLibrary<T> {
    pub(super) instances: Arc<Instances>,
    pub(super) layout: Layout,
    pub(super) network: Network<T>,
    pub(super) tasks: Arc<Tasks>,
    pub(super) context: ContextHub,
    pub(super) settings: Arc<Mutex<SettingsStore>>,
    /// Where a CurseForge pack's files are looked up.
    pub(super) plugins: Arc<PluginHost>,
}

impl<T> InstanceLibrary<T> {
    pub(crate) fn new(
        instances: Arc<Instances>,
        layout: Layout,
        network: Network<T>,
        tasks: Arc<Tasks>,
        context: ContextHub,
        settings: Arc<Mutex<SettingsStore>>,
        plugins: Arc<PluginHost>,
    ) -> Self {
        Self {
            instances,
            layout,
            network,
            tasks,
            context,
            settings,
            plugins,
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_plugins(&mut self, plugins: Arc<PluginHost>) {
        self.plugins = plugins;
    }
}

impl<T: Transport + Clone> InstanceLibrary<T> {
    pub(crate) async fn library(&self) -> Library {
        let store = self.instances.lock().await;
        Library {
            instances: store.instances().to_vec(),
            collections: store.collections().to_vec(),
        }
    }

    /// Reads the registered instance even when its profile folder is missing.
    pub(crate) async fn instance(&self, id: &str) -> Result<InstanceRecord, ServiceError> {
        self.instances
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))
    }

    pub(crate) async fn set_favorite(&self, id: &str, favorite: bool) -> Result<(), ServiceError> {
        Ok(self.instances.lock().await.set_favorite(id, favorite)?)
    }

    /// Flips the favorite flag and returns the new value.
    pub(crate) async fn toggle_favorite(&self, id: &str) -> Result<bool, ServiceError> {
        let mut store = self.instances.lock().await;
        let favorite = !store
            .get(id)
            .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?
            .favorite;
        store.set_favorite(id, favorite)?;
        Ok(favorite)
    }

    pub(crate) async fn create_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.instances.lock().await.create_collection(name)?)
    }

    pub(crate) async fn rename_collection(&self, from: &str, to: &str) -> Result<(), ServiceError> {
        Ok(self.instances.lock().await.rename_collection(from, to)?)
    }

    /// Removes the collection only; its games stay in the library.
    pub(crate) async fn delete_collection(&self, name: &str) -> Result<(), ServiceError> {
        Ok(self.instances.lock().await.delete_collection(name)?)
    }

    /// Puts a game in exactly these collections.
    pub(crate) async fn set_game_collections(
        &self,
        id: &str,
        collections: &[String],
    ) -> Result<(), ServiceError> {
        Ok(self
            .instances
            .lock()
            .await
            .set_collections_of(id, collections)?)
    }

    pub(crate) async fn rename(&self, id: &str, name: &str) -> Result<(), ServiceError> {
        Ok(self.instances.lock().await.rename(id, name)?)
    }
}
