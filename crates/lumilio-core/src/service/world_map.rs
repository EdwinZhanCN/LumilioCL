use super::{LauncherService, ServiceError};
use crate::{CancellationToken, MapFailure, MapProviders, Transport, world_map};
use lumilio_plugin_api::map::{
    MapObject, OverlayInfo, OverlayRequest, TileReply, TileRequest, WorldContext,
};

impl<T: Transport + Clone> LauncherService<T> {
    pub async fn map_contexts(
        &self,
        instance: &str,
    ) -> Result<Vec<world_map::WorldMapContext>, ServiceError> {
        self.instance(instance).await?;
        let dir = self.layout.game(instance);
        let instance = instance.to_owned();
        let database = self.layout.database();
        tokio::task::spawn_blocking(move || {
            let mut contexts =
                world_map::contexts(&instance, &dir).map_err(std::io::Error::other)?;
            let links =
                world_map::store::links(&database, &instance).map_err(std::io::Error::other)?;
            world_map::xaero::attach(
                &mut contexts,
                &instance,
                &world_map::xaero::minimap_dirs(&dir),
                &links,
            );
            contexts.extend(
                world_map::store::load(&database, &instance).map_err(std::io::Error::other)?,
            );
            Ok::<_, std::io::Error>(contexts)
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }
    /// Records that the person confirmed a save's Minimap directory. The
    /// directory must exist, so a stale suggestion cannot be linked.
    pub async fn link_map_xaero(
        &self,
        instance: &str,
        folder: &str,
        dir: &str,
    ) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        let game = self.layout.game(instance);
        let (database, instance, folder, dir) = (
            self.layout.database(),
            instance.to_owned(),
            folder.to_owned(),
            dir.to_owned(),
        );
        tokio::task::spawn_blocking(move || {
            if !world_map::xaero::minimap_dirs(&game).contains(&dir) {
                return Err(std::io::Error::other("unknown Minimap directory"));
            }
            world_map::store::link(&database, &instance, &folder, &dir)
                .map_err(std::io::Error::other)
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }
    pub async fn unlink_map_xaero(&self, instance: &str, folder: &str) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        let (database, instance, folder) = (
            self.layout.database(),
            instance.to_owned(),
            folder.to_owned(),
        );
        tokio::task::spawn_blocking(move || {
            world_map::store::unlink(&database, &instance, &folder)
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(std::io::Error::other)?;
        Ok(())
    }
    pub async fn map_providers(&self) -> MapProviders {
        self.plugins.map_providers().await
    }
    pub async fn map_overlays(&self, context: &WorldContext) -> Vec<(String, OverlayInfo)> {
        self.plugins.map_overlays(context).await
    }
    pub async fn map_objects(
        &self,
        instance: &str,
        plugin: &str,
        request: OverlayRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<MapObject>, MapFailure> {
        self.instance(instance).await.map_err(|_| MapFailure::Off)?;
        self.plugins
            .map_objects(plugin, self.layout.game(instance), request, cancel)
            .await
    }
    pub async fn save_map_seed(
        &self,
        instance: &str,
        seed: i64,
        version: &str,
    ) -> Result<(), ServiceError> {
        self.instance(instance).await?;
        if version.is_empty() || version.len() > 64 {
            return Err(ServiceError::Plugin(
                lumilio_plugin_api::PluginError::InvalidInput("invalid map version".into()),
            ));
        }
        let (database, instance, version) = (
            self.layout.database(),
            instance.to_owned(),
            version.to_owned(),
        );
        tokio::task::spawn_blocking(move || {
            world_map::store::save(&database, &instance, seed, &version)
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(std::io::Error::other)?;
        Ok(())
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
    /// Whether an edit could start now: no game running on the instance and
    /// no other operation holding it.
    pub fn map_can_edit(&self, instance: &str) -> bool {
        self.reserve_instance(instance).is_ok()
    }
    /// Applies a confirmed edit to the instance's files. The instance is held
    /// for the duration, which is how a running game (it holds the same
    /// lease) keeps edits out: Xaero rewrites its files when the player leaves
    /// a world, so a write during play would be lost or would fight it.
    pub async fn map_apply(
        &self,
        instance: &str,
        plugin: &str,
        edit: lumilio_plugin_api::map::ObjectEdit,
    ) -> Result<(), MapFailure> {
        self.instance(instance).await.map_err(|_| MapFailure::Off)?;
        let _lease = self
            .reserve_instance(instance)
            .map_err(|_| MapFailure::Failed("map-edit-running".into()))?;
        self.plugins
            .map_apply(
                plugin,
                self.layout.game(instance),
                self.layout.profile(instance).join("xaero-backups"),
                edit,
            )
            .await
    }
    pub async fn clear_map_cache(&self) -> Result<(), ServiceError> {
        let root = self.layout.map_cache();
        tokio::task::spawn_blocking(move || world_map::TileCache::clear(&root))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }
}
