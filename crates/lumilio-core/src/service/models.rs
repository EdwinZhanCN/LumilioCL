//! An inline 3D preview's schematic and the block assets it is drawn with
//! (ADR 0028, plan schematic-studio D2/D3/D23): the bundle's, with the mods
//! and resource packs the studio folder switched on laid over them.

use super::support::blocking_value;
use super::{LauncherService, ServiceError};
use crate::activity::CancellationToken;
use crate::activity_log::{TaskAction, TaskCategory, TaskLabel};
use crate::schematic_assets::{
    AssetNotice, AssetPlan, BUNDLE_RELEASE, BundleFile, BundleIndex, BundleReach, BundleStore,
    EntityGeometry, InstanceAssets, LocalCache, PlannedSource, SchematicAssetsError,
    installed_names, instance_assets, load, look, provided, resolve, switched_on,
};
use crate::transfer::Transport;
use lumilio_block_assets::{BakedPack, BlockCatalog, BlockNames, EntityLayers};

/// Everything the viewer is served.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelPreview {
    pub schematic: Vec<u8>,
    /// `None` when no source could supply block assets; `plan.notice` says why.
    pub pack: Option<BakedPack>,
    /// The sources the pack was made from, lowest priority first;
    /// `plan.entities` says where `entities` came from.
    pub plan: AssetPlan,
    /// The model layers of the plan's game version (T6), by layer id; empty
    /// when no geometry could be had.
    pub entities: EntityLayers,
}

impl<T: Transport + Clone> LauncherService<T> {
    /// The schematic `file` (a path the plugin may read) and the block assets
    /// its data version calls for (D3): the bundle's, with the overlays the
    /// studio folder holding `file` switched on (T21). Fetching the bundle is
    /// an Activity task; out of reach, the plan carries a notice.
    pub async fn plugin_model(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
    ) -> Result<ModelPreview, ServiceError> {
        self.model_switched(instance, plugin, file, None).await
    }

    /// [`Self::plugin_model`], with `switched` on instead of the sources
    /// kept for the file's folder when it is given.
    async fn model_switched(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
        switched: Option<Vec<std::path::PathBuf>>,
    ) -> Result<ModelPreview, ServiceError> {
        let record = self.instance(instance).await?;
        let schematic = self
            .plugins
            .read_model(plugin, self.layout.game(instance), file)
            .await
            .map_err(ServiceError::Plugin)?;
        let data_version = crate::schematic_assets::data_version(&schematic);
        let (layout, within) = (self.layout.clone(), file.to_owned());
        let facts = blocking_value(move || {
            let mut facts = instance_assets(&layout, &record);
            facts.switched_on = switched
                .unwrap_or_else(|| switched_on(&layout.schematic_sources(&record.id), &within));
            facts
        })
        .await?;
        let (pack, mut plan, index) = self.schematic_assets(&facts, data_version).await;
        let entities = if pack.is_some() {
            self.entity_layers(&mut plan, index.as_ref()).await
        } else {
            plan.entities = EntityGeometry::None;
            EntityLayers::new()
        };
        Ok(ModelPreview {
            schematic,
            pack,
            plan,
            entities,
        })
    }

    /// Resolves and loads the block assets; failures become a notice. Also
    /// returns the bundle index, when it could be had, for the entities.
    async fn schematic_assets(
        &self,
        facts: &InstanceAssets,
        data_version: Option<u32>,
    ) -> (Option<BakedPack>, AssetPlan, Option<BundleIndex>) {
        let cache_root = self.layout.model_cache();
        let store = BundleStore::new(&cache_root);
        let cache = LocalCache::new(&cache_root);
        let index = self.bundle_index(&store).await;
        let mut plan = resolve(facts, data_version, reach(&index));
        let mut file = None;
        if let (Some(PlannedSource::Bundle { version }), Ok(index)) = (plan.base(), &index) {
            let wanted = index.file_of(index.version(version).expect("resolve picks from index"));
            match self.bundle_file(&store, &wanted, version).await {
                Ok(()) => file = Some(wanted),
                // The bundle could not be had after all: plan as if offline.
                Err(unreachable) => plan = resolve(facts, data_version, unreachable.reach()),
            }
        }
        let index = index.ok();
        if plan.base().is_none() {
            return (None, plan, index);
        }
        let (loading, store_for_load, offered) = (plan.clone(), store.clone(), facts.clone());
        let loaded = blocking_value(move || {
            let baked = load(
                &loading,
                &cache,
                file.as_ref().map(|file| (&store_for_load, file)),
            )?;
            // Every overlay on offer is counted, switched on or not.
            let sources = provided(&loading, &baked.pack, &offered, |path| cache.scan(path));
            Ok::<_, SchematicAssetsError>((baked.pack, sources))
        })
        .await;
        match loaded {
            Ok(Ok((pack, sources))) => {
                plan.provided = sources;
                (Some(pack), plan, index)
            }
            _ => {
                plan.notice = Some(AssetNotice::Unreadable);
                (None, plan, index)
            }
        }
    }

    /// The entity layers `plan.entities` names (D4): the bundle's. A bundle
    /// file that cannot be had leaves none, and `plan.entities` saying so.
    /// Never an error: entities only add to a preview.
    async fn entity_layers(
        &self,
        plan: &mut AssetPlan,
        index: Option<&BundleIndex>,
    ) -> EntityLayers {
        let EntityGeometry::Bundle { version } = plan.entities.clone() else {
            return EntityLayers::new();
        };
        let store = BundleStore::new(&self.layout.model_cache());
        let layers = match index.and_then(BundleIndex::entities_file) {
            Some(file) if self.bundle_file(&store, &file, &version).await.is_ok() => {
                blocking_value(move || store.entity_layers(&file, &version))
                    .await
                    .ok()
                    .and_then(Result::ok)
            }
            _ => None,
        };
        layers.unwrap_or_else(|| {
            plan.entities = EntityGeometry::None;
            EntityLayers::new()
        })
    }

    /// The bundle index: kept from before, else fetched when allowed.
    async fn bundle_index(&self, store: &BundleStore) -> Result<BundleIndex, BundleUnreachable> {
        let cached = {
            let store = store.clone();
            blocking_value(move || store.cached_index())
                .await
                .ok()
                .flatten()
        };
        if let Some(index) = cached {
            return Ok(index);
        }
        if !self.settings.lock().await.get().auto_download_block_assets {
            return Err(BundleUnreachable::Disabled);
        }
        let chain = self.chain().await.map_err(|_| BundleUnreachable::Offline)?;
        store
            .fetch_index(&self.transport, &chain)
            .await
            .map_err(|error| match error {
                SchematicAssetsError::NotServed(_) => BundleUnreachable::Unavailable,
                _ => BundleUnreachable::Offline,
            })
    }

    /// The game's names for blocks, items and entities (plan
    /// schematic-studio, T26/D28): the bundle's `names.dat.z`, fetched
    /// quietly like the index, else the language files of the games
    /// installed here, the instance's own first. Empty when neither can be
    /// had; the studio then writes IDs.
    pub async fn schematic_names(&self, instance: &str) -> Result<BlockNames, ServiceError> {
        let record = self.instance(instance).await?;
        let store = BundleStore::new(&self.layout.model_cache());
        if let Ok(index) = self.bundle_index(&store).await
            && let Some(file) = index.names_file()
            && self.names_file(&store, &file).await
        {
            let names = blocking_value(move || store.names(&file)).await?;
            if let Ok(names) = names
                && !names.is_empty()
            {
                return Ok(names);
            }
        }
        let layout = self.layout.clone();
        blocking_value(move || {
            let facts = instance_assets(&layout, &record);
            let clients: Vec<_> = facts.client.iter().chain(&facts.other_clients).collect();
            installed_names(&clients, layout.launch_directories(&record).assets())
        })
        .await
    }

    /// A thumbnail of `file` (PNG bytes), by the key of its blocks (plan
    /// schematic-studio, T28/D29): the content fingerprint, so a file that
    /// is renamed or has new metadata keeps its picture. It is kept per look
    /// too (T21): the sources switched on for the file's folder, read once,
    /// so the picture and the name it is kept under agree even when a switch
    /// flips meanwhile, and in the order they are layered (T36). One drawn before is read back; otherwise `draw`
    /// makes it from the preview on a blocking thread, and it is kept.
    pub async fn schematic_thumbnail<D>(
        &self,
        instance: &str,
        plugin: &str,
        file: &str,
        key: &str,
        draw: D,
    ) -> Result<Vec<u8>, ServiceError>
    where
        D: FnOnce(ModelPreview) -> Result<Vec<u8>, String> + Send + 'static,
    {
        let plain =
            !key.is_empty() && key.len() <= 128 && key.chars().all(|c| c.is_ascii_hexdigit());
        if !plain {
            return Err(ServiceError::Install(format!("not a thumbnail key: {key}")));
        }
        let record = self.instance(instance).await?;
        let (layout, within) = (self.layout.clone(), file.to_owned());
        let (switched, look) = blocking_value(move || {
            let on = switched_on(&layout.schematic_sources(&record.id), &within);
            // The look is the stack in effect, in the order it is layered
            // (T36): a reorder draws the picture again.
            let stack: Vec<_> = instance_assets(&layout, &record)
                .overlays
                .into_iter()
                .filter(|overlay| on.contains(overlay))
                .collect();
            let look = look(&stack);
            (on, look)
        })
        .await?;
        let path = self
            .layout
            .model_cache()
            .join("thumbnails")
            .join(format!("{key}{look}-{BUNDLE_RELEASE}.png"));
        if let Ok(png) = tokio::fs::read(&path).await {
            return Ok(png);
        }
        let preview = self
            .model_switched(instance, plugin, file, Some(switched))
            .await?;
        let png = blocking_value(move || draw(preview))
            .await?
            .map_err(ServiceError::Install)?;
        let kept = png.clone();
        // A picture that cannot be kept is still shown; it is drawn again
        // next time.
        let _ = blocking_value(move || -> std::io::Result<()> {
            let dir = path.parent().expect("a file under the thumbnails folder");
            std::fs::create_dir_all(dir)?;
            let partial = path.with_extension("partial");
            std::fs::write(&partial, kept)?;
            std::fs::rename(partial, path)
        })
        .await;
        Ok(png)
    }

    /// Whether the names `file` is here, fetching it when allowed.
    async fn names_file(&self, store: &BundleStore, file: &BundleFile) -> bool {
        if store.has(file) {
            return true;
        }
        if !self.settings.lock().await.get().auto_download_block_assets {
            return false;
        }
        let Ok(chain) = self.chain().await else {
            return false;
        };
        store
            .fetch_small(&self.transport, &chain, file)
            .await
            .is_ok()
    }

    /// The game versions the block-asset bundle holds, newest first, with
    /// their data versions: what the studio can convert a schematic to
    /// (plan schematic-studio, T14). Fails when the index cannot be had.
    pub async fn schematic_versions(&self) -> Result<Vec<(String, u32)>, ServiceError> {
        let store = BundleStore::new(&self.layout.model_cache());
        let index = self
            .bundle_index(&store)
            .await
            .map_err(BundleUnreachable::error)?;
        let mut versions: Vec<(String, u32)> = index
            .versions
            .into_iter()
            .map(|version| (version.id, version.data_version))
            .collect();
        versions.sort_by_key(|version| std::cmp::Reverse(version.1));
        Ok(versions)
    }

    /// The block states bundle version `target` knows, and what the
    /// blockstates of the bundle version nearest `source_data_version`
    /// mention (`None` when that is `target` or cannot be read): what a
    /// backward conversion is checked against (plan schematic-studio, T14).
    /// Downloads the bundle file(s) as an Activity task when needed.
    pub async fn schematic_catalogs(
        &self,
        target: &str,
        source_data_version: u32,
    ) -> Result<(BlockCatalog, Option<BlockCatalog>), ServiceError> {
        let store = BundleStore::new(&self.layout.model_cache());
        let index = self
            .bundle_index(&store)
            .await
            .map_err(BundleUnreachable::error)?;
        let wanted = index
            .version(target)
            .ok_or_else(|| ServiceError::Install(format!("no block assets for {target}")))?;
        let source = index
            .versions
            .iter()
            .min_by_key(|version| version.data_version.abs_diff(source_data_version))
            .filter(|version| version.id != wanted.id);
        let target_file = index.file_of(wanted);
        self.bundle_file(&store, &target_file, &wanted.id)
            .await
            .map_err(BundleUnreachable::error)?;
        let source_file = match source {
            Some(version) => {
                let file = index.file_of(version);
                self.bundle_file(&store, &file, &version.id)
                    .await
                    .ok()
                    .map(|()| (file, version.id.clone()))
            }
            None => None,
        };
        let target = wanted.id.clone();
        blocking_value(move || {
            let target = store
                .catalogs(&target_file, &[&target])?
                .pop()
                .unwrap_or_default();
            let source = source_file
                .and_then(|(file, id)| store.catalogs(&file, &[&id]).ok())
                .and_then(|mut catalogs| catalogs.pop());
            Ok::<_, SchematicAssetsError>((target, source))
        })
        .await?
        .map_err(|error| ServiceError::Install(error.to_string()))
    }

    /// Makes sure `file` is downloaded, as an Activity task when it is not.
    async fn bundle_file(
        &self,
        store: &BundleStore,
        file: &BundleFile,
        version: &str,
    ) -> Result<(), BundleUnreachable> {
        if store.has(file) {
            return Ok(());
        }
        if !self.settings.lock().await.get().auto_download_block_assets {
            return Err(BundleUnreachable::Disabled);
        }
        let chain = self.chain().await.map_err(|_| BundleUnreachable::Offline)?;
        let cancel = CancellationToken::new();
        let task = self.begin(
            TaskCategory::Download,
            TaskLabel::Typed {
                action: TaskAction::PrepareBlockAssets,
                subject: version.to_owned(),
            },
            None,
            Some(&cancel),
        );
        let engine = self.transfer_engine().await;
        let result = self
            .metered(
                task.id,
                engine.subscribe(),
                store.download(&engine, &chain, file, cancel.clone()),
            )
            .await
            .map_err(|error| {
                if cancel.is_cancelled() {
                    ServiceError::Cancelled
                } else {
                    ServiceError::Install(error.to_string())
                }
            });
        self.end(task, &result);
        result.map(|_| ()).map_err(|_| BundleUnreachable::Offline)
    }
}

/// Why the bundle cannot be used for this preview.
#[derive(Clone, Copy, Debug)]
enum BundleUnreachable {
    Offline,
    /// The host answered without the index.
    Unavailable,
    Disabled,
}

impl BundleUnreachable {
    const fn reach(self) -> BundleReach<'static> {
        match self {
            Self::Offline => BundleReach::Offline,
            Self::Unavailable => BundleReach::Unavailable,
            Self::Disabled => BundleReach::Disabled,
        }
    }

    fn error(self) -> ServiceError {
        ServiceError::Install(
            match self {
                Self::Offline => "block asset index unreachable",
                Self::Unavailable => "block asset index not on the server",
                Self::Disabled => "block asset download is off",
            }
            .to_owned(),
        )
    }
}

fn reach(index: &Result<BundleIndex, BundleUnreachable>) -> BundleReach<'_> {
    match index {
        Ok(index) => BundleReach::Index(index),
        Err(unreachable) => unreachable.reach(),
    }
}
