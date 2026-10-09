//! Structure layers from cubiomes' finders, one overlay per kind so each has
//! its own switch. Positions are exact for the seed; viability is cubiomes'
//! biome check (an estimate for the height-bound kinds since 1.18).
use lumilio_cubiomes::{Dimension as Cube, Structure, Version};
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext,
};
use lumilio_plugin_api::{HostContext, PluginError};
use std::sync::Mutex;

pub(crate) const GROUP: &str = "map-group-structures";
const SOURCE: &str = crate::ID;

/// One switchable layer, named `structure.<name>` with the catalog label
/// `map-structure-<name>`. A `None` kind is the strongholds, which are not a
/// region grid.
struct Layer {
    name: &'static str,
    kind: Option<Structure>,
    icon: MapIcon,
}

const LAYERS: [Layer; 18] = [
    layer("village", Some(Structure::Village), MapIcon::Village),
    layer(
        "desert-pyramid",
        Some(Structure::DesertPyramid),
        MapIcon::DesertPyramid,
    ),
    layer(
        "jungle-temple",
        Some(Structure::JungleTemple),
        MapIcon::JungleTemple,
    ),
    layer("swamp-hut", Some(Structure::SwampHut), MapIcon::SwampHut),
    layer("igloo", Some(Structure::Igloo), MapIcon::Igloo),
    layer("outpost", Some(Structure::Outpost), MapIcon::Outpost),
    layer("monument", Some(Structure::Monument), MapIcon::Monument),
    layer("mansion", Some(Structure::Mansion), MapIcon::Mansion),
    layer("ocean-ruin", Some(Structure::OceanRuin), MapIcon::OceanRuin),
    layer("shipwreck", Some(Structure::Shipwreck), MapIcon::Shipwreck),
    layer(
        "ruined-portal",
        Some(Structure::RuinedPortal),
        MapIcon::RuinedPortal,
    ),
    layer(
        "ancient-city",
        Some(Structure::AncientCity),
        MapIcon::AncientCity,
    ),
    layer(
        "trail-ruins",
        Some(Structure::TrailRuins),
        MapIcon::TrailRuins,
    ),
    layer(
        "trial-chambers",
        Some(Structure::TrialChambers),
        MapIcon::TrialChambers,
    ),
    layer("stronghold", None, MapIcon::Stronghold),
    layer("fortress", Some(Structure::Fortress), MapIcon::Fortress),
    layer("bastion", Some(Structure::Bastion), MapIcon::Bastion),
    layer("end-city", Some(Structure::EndCity), MapIcon::EndCity),
];

const fn layer(name: &'static str, kind: Option<Structure>, icon: MapIcon) -> Layer {
    Layer { name, kind, icon }
}

impl Layer {
    fn id(&self) -> String {
        format!("structure.{}", self.name)
    }
    fn label_id(&self) -> String {
        format!("map-structure-{}", self.name)
    }
}

/// Version and dimension of a world, as the tile provider reads them.
pub(crate) fn world_version(context: &WorldContext) -> Result<Version, PluginError> {
    match context.version.as_deref() {
        Some(name) => Version::from_name(name),
        None => context
            .data_version
            .ok_or(lumilio_cubiomes::Error::Unsupported)
            .and_then(Version::from_data_version),
    }
    .map_err(|_| PluginError::Unavailable("map-version-unsupported".into()))
}

fn cube(dimension: &Dimension) -> Option<Cube> {
    match dimension {
        Dimension::Overworld => Some(Cube::Overworld),
        Dimension::Nether => Some(Cube::Nether),
        Dimension::End => Some(Cube::End),
        Dimension::Custom(_) => None,
    }
}

fn offered(layer: &Layer, version: Version, dimension: Cube) -> bool {
    match layer.kind {
        Some(kind) => kind.available(version, dimension),
        None => dimension == Cube::Overworld,
    }
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    LAYERS
        .iter()
        .map(|layer| OverlayInfo {
            id: layer.id(),
            kind_id: layer.label_id(),
            dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
            icon: Some(layer.icon),
            group_id: Some(GROUP.into()),
            approximate: false,
            max_scale: None,
        })
        .collect()
}

/// The layers this world's version and dimension can show. Without a seed or
/// with a version cubiomes lacks there are none.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    let (Ok(version), Some(dimension), Some(_)) = (
        world_version(context),
        cube(&context.dimension),
        context.seed,
    ) else {
        return vec![];
    };
    LAYERS
        .iter()
        .filter(|layer| offered(layer, version, dimension))
        .map(|layer| OverlayInfo {
            id: layer.id(),
            kind_id: layer.label_id(),
            dimensions: vec![context.dimension.clone()],
            icon: Some(layer.icon),
            group_id: Some(GROUP.into()),
            approximate: layer.kind.is_some_and(|kind| kind.estimated(version)),
            max_scale: None,
        })
        .collect()
}

/// Strongholds depend on biomes and cost a search each, so the 128 of a world
/// are computed once and kept for the few worlds being looked at.
type Worlds = Vec<((i64, i32), Vec<[i32; 2]>)>;
static STRONGHOLDS: Mutex<Worlds> = Mutex::new(Vec::new());
const STRONGHOLD_WORLDS: usize = 4;

fn strongholds(
    version: Version,
    seed: i64,
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<[i32; 2]>, lumilio_cubiomes::Error> {
    let key = (seed, version.index());
    {
        let cache = STRONGHOLDS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some((_, found)) = cache.iter().find(|(known, _)| *known == key) {
            return Ok(found.clone());
        }
    }
    let found = lumilio_cubiomes::strongholds(version, seed, 128, cancelled)?;
    let mut cache = STRONGHOLDS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    cache.retain(|(known, _)| *known != key);
    cache.push((key, found.clone()));
    if cache.len() > STRONGHOLD_WORLDS {
        cache.remove(0);
    }
    Ok(found)
}

fn area(bounds: &MapBounds) -> Result<[i32; 4], PluginError> {
    let edges = [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z];
    if edges
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 30_000_000.)
        || bounds.min.x >= bounds.max.x
        || bounds.min.z >= bounds.max.z
    {
        return Err(PluginError::InvalidInput("invalid structure bounds".into()));
    }
    Ok([
        bounds.min.x.floor() as i32,
        bounds.min.z.floor() as i32,
        bounds.max.x.ceil() as i32,
        bounds.max.z.ceil() as i32,
    ])
}

pub(crate) fn objects(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
) -> Result<Vec<MapObject>, PluginError> {
    let layer = LAYERS
        .iter()
        .find(|layer| layer.id() == request.overlay)
        .ok_or_else(|| PluginError::InvalidInput("unknown structure layer".into()))?;
    let context = &request.context;
    let version = world_version(context)?;
    let (Some(seed), Some(dimension)) = (context.seed, cube(&context.dimension)) else {
        return Ok(vec![]);
    };
    if !offered(layer, version, dimension) {
        return Ok(vec![]);
    }
    let [x0, z0, x1, z1] = area(&request.bounds)?;
    let cancelled = || ctx.cancelled();
    let found = match layer.kind {
        Some(kind) => lumilio_cubiomes::structures(
            version,
            seed,
            dimension,
            kind,
            [x0, z0, x1, z1],
            cancelled,
        ),
        None => strongholds(version, seed, cancelled).map(|all| {
            all.into_iter()
                .filter(|[x, z]| (x0..x1).contains(x) && (z0..z1).contains(z))
                .collect()
        }),
    }
    .map_err(|error| match error {
        lumilio_cubiomes::Error::Cancelled => PluginError::Transient("map-cancelled".into()),
        lumilio_cubiomes::Error::InvalidRange => {
            PluginError::InvalidInput("structure area too large".into())
        }
        _ => PluginError::InvalidInput("structure search failed".into()),
    })?;
    let approximate = layer.kind.is_some_and(|kind| kind.estimated(version));
    Ok(found
        .into_iter()
        .map(|[x, z]| MapObject {
            id: format!("{}:{x}:{z}", request.overlay),
            raw_id: format!("{x},{z}"),
            source: SOURCE.into(),
            world: context.world.clone(),
            dimension: context.dimension.clone(),
            kind: MapObjectKind::Icon {
                icon: layer.icon,
                at: MapPoint {
                    x: f64::from(x),
                    z: f64::from(z),
                },
            },
            label: None,
            label_id: Some(layer.label_id()),
            priority: if layer.kind.is_none() { 10 } else { 0 },
            approximate,
            color: None,
        })
        .collect())
}
