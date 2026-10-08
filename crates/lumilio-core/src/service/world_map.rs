use super::{LauncherService, ServiceError};
use crate::{CancellationToken, MapFailure, MapProviders, Transport, world_map};
use lumilio_plugin_api::map::{TileReply, TileRequest};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn map_contexts(
        &self,
        instance: &str,
    ) -> Result<Vec<world_map::WorldMapContext>, ServiceError> {
        self.instance(instance).await?;
        let dir = self.layout.game(instance);
        let instance = instance.to_owned();
        tokio::task::spawn_blocking(move || world_map::contexts(&instance, &dir))
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }
    pub async fn map_providers(&self) -> MapProviders {
        self.plugins.map_providers().await
    }
    pub async fn map_tile(
        &self,
        instance: &str,
        request: TileRequest,
        cancel: CancellationToken,
    ) -> Result<TileReply, MapFailure> {
        self.instance(instance).await.map_err(|_| MapFailure::Off)?;
        let provider = request.key.provider.clone();
        // Check plugin status even on cache hits, so disabled providers vanish.
        if !self
            .plugins
            .list()
            .await
            .iter()
            .any(|info| info.manifest.id == provider && info.status == crate::PluginStatus::Enabled)
        {
            return Err(MapFailure::Off);
        }
        let cache = world_map::TileCache::new(self.layout.map_cache());
        let (read_cache, read_request) = (cache.clone(), request.clone());
        if let Ok(Ok(Some(reply))) =
            tokio::task::spawn_blocking(move || read_cache.get(&read_request)).await
        {
            return if cancel.is_cancelled() {
                Err(MapFailure::Cancelled)
            } else {
                Ok(reply)
            };
        }
        let reply = self
            .plugins
            .map_tile(
                &provider,
                self.layout.game(instance),
                request.clone(),
                cancel.clone(),
            )
            .await?;
        if cancel.is_cancelled() {
            return Err(MapFailure::Cancelled);
        }
        let saved = reply.clone();
        let _ = tokio::task::spawn_blocking(move || cache.put(&request, &saved)).await;
        Ok(reply)
    }
    pub async fn clear_map_cache(&self) -> Result<(), ServiceError> {
        let root = self.layout.map_cache();
        tokio::task::spawn_blocking(move || world_map::TileCache::clear(&root))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }
}
