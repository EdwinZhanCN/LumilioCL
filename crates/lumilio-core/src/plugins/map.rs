//! Map calls isolate faults to a provider, never the plugin's sticky status.
use super::{Context, PluginHost, PluginStatus, enabled, isolated};
use crate::activity::CancellationToken;
use lumilio_plugin_api::map::{
    BaseMapInfo, MapObject, ObjectEdit, OverlayInfo, OverlayRequest, TileReply, TileRequest,
    WorldContext,
};
use lumilio_plugin_api::{FileStat, HostContext, Plugin, PluginError};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

const MAP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapFailure {
    Off,
    Cancelled,
    ProviderStopped,
    Failed(String),
}

/// A file a tile is drawn from and what it looked like (`None`: missing).
pub type SourceStamp = (String, Option<FileStat>);

#[derive(Clone, Debug, Default)]
pub struct MapProviders {
    pub base_maps: Vec<(String, BaseMapInfo)>,
    pub overlays: Vec<(String, OverlayInfo)>,
}

impl PluginHost {
    pub async fn map_providers(&self) -> MapProviders {
        let mut providers = MapProviders::default();
        for info in self.list().await {
            if info.status != PluginStatus::Enabled {
                continue;
            }
            let result = self
                .map_call(
                    &info.manifest.id,
                    "catalog",
                    None,
                    CancellationToken::new(),
                    MAP_TIMEOUT,
                    |plugin, _| {
                        Ok((
                            plugin
                                .base_map_provider()
                                .map_or_else(Vec::new, |p| p.base_maps()),
                            plugin
                                .overlay_provider()
                                .map_or_else(Vec::new, |p| p.overlays()),
                        ))
                    },
                )
                .await;
            if let Ok((bases, overlays)) = result {
                providers.base_maps.extend(
                    bases
                        .into_iter()
                        .map(|base| (info.manifest.id.clone(), base)),
                );
                providers.overlays.extend(
                    overlays
                        .into_iter()
                        .map(|overlay| (info.manifest.id.clone(), overlay)),
                );
            }
        }
        providers
    }

    /// The overlay layers each enabled plugin offers for one world.
    pub async fn map_overlays(&self, context: &WorldContext) -> Vec<(String, OverlayInfo)> {
        let mut layers = Vec::new();
        for info in self.list().await {
            if info.status != PluginStatus::Enabled {
                continue;
            }
            let context = context.clone();
            let result = self
                .map_call(
                    &info.manifest.id,
                    "catalog",
                    None,
                    CancellationToken::new(),
                    MAP_TIMEOUT,
                    move |plugin, _| {
                        Ok(plugin
                            .overlay_provider()
                            .map_or_else(Vec::new, |p| p.overlays_for(&context)))
                    },
                )
                .await;
            if let Ok(overlays) = result {
                layers.extend(
                    overlays
                        .into_iter()
                        .map(|overlay| (info.manifest.id.clone(), overlay)),
                );
            }
        }
        layers
    }

    pub async fn map_tile(
        &self,
        plugin: &str,
        game_dir: PathBuf,
        request: TileRequest,
        cancel: CancellationToken,
    ) -> Result<TileReply, MapFailure> {
        if request.key.provider != plugin
            || request.pixels != 256
            || request.key.level > 4
            || request.context.world != request.key.world
            || request.context.dimension != request.key.dimension
        {
            return Err(MapFailure::Failed("invalid tile request".into()));
        }
        let provider = format!("base:{}", request.key.base_map);
        self.map_call(
            plugin,
            &provider,
            Some(game_dir),
            cancel,
            MAP_TIMEOUT,
            move |plugin, ctx| {
                let reply = plugin
                    .base_map_provider()
                    .ok_or_else(|| PluginError::Unavailable("no base provider".into()))?
                    .tile(ctx, &request)?;
                if !reply.is_valid() {
                    return Err(PluginError::InvalidInput("invalid tile reply".into()));
                }
                Ok(reply)
            },
        )
        .await
    }

    /// The files a tile is drawn from, each with its length and modification
    /// time as the host sees them; `None` for a base map not drawn from files.
    /// See [`lumilio_plugin_api::map::BaseMapProvider::sources`].
    pub async fn map_sources(
        &self,
        plugin: &str,
        game_dir: PathBuf,
        request: TileRequest,
        cancel: CancellationToken,
    ) -> Result<Option<Vec<SourceStamp>>, MapFailure> {
        if request.key.provider != plugin
            || request.key.level > 4
            || request.context.world != request.key.world
            || request.context.dimension != request.key.dimension
        {
            return Err(MapFailure::Failed("invalid tile request".into()));
        }
        let provider = format!("base:{}", request.key.base_map);
        self.map_call(
            plugin,
            &provider,
            Some(game_dir),
            cancel,
            MAP_TIMEOUT,
            move |plugin, ctx| {
                let Some(paths) = plugin
                    .base_map_provider()
                    .ok_or_else(|| PluginError::Unavailable("no base provider".into()))?
                    .sources(ctx, &request)?
                else {
                    return Ok(None);
                };
                if paths.len() > lumilio_plugin_api::map::MAX_SOURCES {
                    return Err(PluginError::InvalidInput("too many tile sources".into()));
                }
                // Stat through the plugin's own grants: a source outside them
                // is refused like any other read.
                let mut stamps = paths
                    .into_iter()
                    .map(|path| Ok((path.clone(), ctx.file_stat(&path)?)))
                    .collect::<Result<Vec<_>, PluginError>>()?;
                stamps.sort_by(|a, b| a.0.cmp(&b.0));
                stamps.dedup_by(|a, b| a.0 == b.0);
                Ok(Some(stamps))
            },
        )
        .await
    }

    pub async fn map_objects(
        &self,
        plugin: &str,
        game_dir: PathBuf,
        request: OverlayRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<MapObject>, MapFailure> {
        let provider = format!("overlay:{}", request.overlay);
        self.map_call(
            plugin,
            &provider,
            Some(game_dir),
            cancel,
            MAP_TIMEOUT,
            move |plugin, ctx| {
                let objects = plugin
                    .overlay_provider()
                    .ok_or_else(|| PluginError::Unavailable("no overlay provider".into()))?
                    .objects(ctx, &request)?;
                if objects.len() > 100_000
                    || objects.iter().any(|object| {
                        object.world != request.context.world
                            || object.dimension != request.context.dimension
                    })
                {
                    return Err(PluginError::InvalidInput("invalid map objects".into()));
                }
                Ok(objects)
            },
        )
        .await
    }

    async fn map_call<R, F>(
        &self,
        id: &str,
        provider: &str,
        game_dir: Option<PathBuf>,
        cancel: CancellationToken,
        timeout: Duration,
        call: F,
    ) -> Result<R, MapFailure>
    where
        R: Send + 'static,
        F: FnOnce(&dyn Plugin, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        self.map_call_with(id, provider, game_dir, None, cancel, timeout, call)
            .await
    }

    /// Applies an edit. The caller must hold the instance (so the game is not
    /// running) for as long as this runs; `backups` is where replaced files
    /// are copied. An edit is not cancelled halfway: it runs to the end or not
    /// at all.
    pub async fn map_apply(
        &self,
        plugin: &str,
        game_dir: PathBuf,
        backups: PathBuf,
        edit: ObjectEdit,
    ) -> Result<(), MapFailure> {
        let provider = format!("edit:{}", edit.overlay);
        self.map_call_with(
            plugin,
            &provider,
            Some(game_dir),
            Some(backups),
            CancellationToken::new(),
            MAP_TIMEOUT,
            move |plugin, ctx| {
                plugin
                    .overlay_provider()
                    .ok_or_else(|| PluginError::Unavailable("no overlay provider".into()))?
                    .apply(ctx, &edit)
            },
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn map_call_with<R, F>(
        &self,
        id: &str,
        provider: &str,
        game_dir: Option<PathBuf>,
        writer: Option<PathBuf>,
        cancel: CancellationToken,
        timeout: Duration,
        call: F,
    ) -> Result<R, MapFailure>
    where
        R: Send + 'static,
        F: FnOnce(&dyn Plugin, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        let key = (id.to_owned(), provider.to_owned());
        let entry = self
            .registry()
            .await
            .entries
            .get(id)
            .cloned()
            .ok_or(MapFailure::Off)?;
        let active = || {
            let preferences = self
                .preferences
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let state = preferences.states.get(id).cloned().unwrap_or_default();
            if !enabled(&entry.manifest, &state)
                || entry
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some()
            {
                return Err(MapFailure::Off);
            }
            Ok((state, preferences.revisions.get(id).copied().unwrap_or(0)))
        };
        active()?;
        let permit = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(MapFailure::Cancelled),
            permit = self.map_calls.clone().acquire_owned() => permit.map_err(|_| MapFailure::Off)?,
        };
        let (state, revision) = active()?;
        if self
            .map_failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&key)
            .copied()
            .unwrap_or(0)
            >= 5
        {
            return Err(MapFailure::ProviderStopped);
        }
        let mut context = Context::new(&entry.manifest, &state, game_dir, None, self.locale_tag());
        context.cancel = Some(cancel.clone());
        context.writer = writer;
        context.revision = revision;
        let plugin = Arc::clone(&entry.plugin);
        let work = isolated(timeout, move || {
            // Permit belongs to the blocking worker, including after timeout:
            // an uncooperative plugin cannot exceed the concurrency cap.
            let _permit = permit;
            call(plugin.as_ref(), &context)
        });
        let result = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(MapFailure::Cancelled),
            result = work => result,
        };
        if cancel.is_cancelled() {
            return Err(MapFailure::Cancelled);
        }
        if active()?.1 != revision {
            return Err(MapFailure::Off);
        }
        let mut failures = self
            .map_failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match result {
            Ok(value) => {
                failures.insert(key, 0);
                Ok(value)
            }
            Err(fault) => {
                cancel.cancel();
                // An explicit version refusal, or a data file the provider
                // cannot read, is a limit of this world, not a broken provider.
                // An unsupported save must not prevent the user from opening a
                // supported manual seed afterward.
                if fault.message.starts_with("map-version-")
                    || fault.message.starts_with("map-xaero-")
                    || fault.message.starts_with("map-save-")
                    || fault.message.starts_with("map-edit-")
                {
                    return Err(MapFailure::Failed(fault.message));
                }
                let count = failures.entry(key).or_default();
                *count = count.saturating_add(1);
                Err(MapFailure::Failed(fault.message))
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/map.rs"]
mod tests;
