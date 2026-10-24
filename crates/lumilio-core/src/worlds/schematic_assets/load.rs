//! Turning an [`AssetPlan`] into one pack: the bundle's base is read from the
//! downloaded bundle, and the overlays switched on, baked over it (so models
//! inheriting vanilla parents resolve against the bundle's templates, T17)
//! and cached as a layer, are laid over it.

use lumilio_block_assets::BakedPack;

use super::cache::Keyed;
use super::{
    AssetPlan, Baked, BundleFile, BundleStore, LocalCache, PlannedSource, SchematicAssetsError,
};

/// The pack `plan` describes. `bundle` is the store and the downloaded file
/// a bundle base is read from. Blocking.
pub fn load(
    plan: &AssetPlan,
    cache: &LocalCache,
    bundle: Option<(&BundleStore, &BundleFile)>,
) -> Result<Baked, SchematicAssetsError> {
    let overlays: Vec<Keyed> = plan
        .overlays()
        .into_iter()
        .map(|path| Keyed::Stamp(path.to_owned()))
        .collect();
    let (Some(PlannedSource::Bundle { version }), Some((store, file))) = (plan.base(), bundle)
    else {
        return Err(SchematicAssetsError::NoBase);
    };
    let mut pack = store.slice(file, version)?;
    if overlays.is_empty() {
        return Ok(Baked {
            pack,
            from_cache: true,
        });
    }
    let base_key = format!("{}:{version}", file.sha256);
    let layer = cache.bake_over(&base_key, &pack, &overlays)?;
    lay_over(&mut pack, layer.pack);
    Ok(Baked {
        pack,
        from_cache: layer.from_cache,
    })
}

/// `top`'s blockstates, models and textures replace `base`'s of the same id.
fn lay_over(base: &mut BakedPack, top: BakedPack) {
    base.meta.sources.extend(top.meta.sources);
    base.meta.skipped.extend(top.meta.skipped);
    let (into, from) = (&mut base.assets, top.assets);
    into.blockstates.extend(from.blockstates);
    into.models.extend(from.models);
    into.textures.extend(from.textures);
    into.images.extend(from.images);
    into.tint_samples.extend(from.tint_samples);
    into.parents.extend(from.parents);
}
