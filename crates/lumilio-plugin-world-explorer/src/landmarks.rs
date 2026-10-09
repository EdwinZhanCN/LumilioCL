//! Overworld landmarks that come from the seed rather than a structure grid:
//! the world spawn and the slime chunks.
use crate::structures::world_version;
use lumilio_cubiomes::Version;
use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext,
};
use std::sync::Mutex;

pub(crate) const GROUP: &str = "map-group-world";
pub(crate) const SPAWN: &str = "world.spawn";
pub(crate) const SLIME: &str = "world.slime";
/// Slime chunks are 16-block squares; coarser than this they blur into noise.
const SLIME_MAX_SCALE: u32 = 1;
const SOURCE: &str = crate::ID;

pub(crate) fn is_landmark(overlay: &str) -> bool {
    overlay == SPAWN || overlay == SLIME
}

/// Every landmark layer, whatever the world.
pub(crate) fn catalog() -> Vec<OverlayInfo> {
    layers(false)
}

/// The landmark layers a world offers: the Overworld of a world with a seed.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    let (Ok(version), Dimension::Overworld, Some(_)) =
        (world_version(context), &context.dimension, context.seed)
    else {
        return vec![];
    };
    layers(lumilio_cubiomes::spawn(version, 0).estimated)
}

fn layers(spawn_estimated: bool) -> Vec<OverlayInfo> {
    vec![
        OverlayInfo {
            id: SPAWN.into(),
            kind_id: "map-world-spawn".into(),
            dimensions: vec![Dimension::Overworld],
            icon: Some(MapIcon::Spawn),
            group_id: Some(GROUP.into()),
            approximate: spawn_estimated,
            max_scale: None,
            creatable: vec![],
        },
        OverlayInfo {
            id: SLIME.into(),
            kind_id: "map-world-slime".into(),
            dimensions: vec![Dimension::Overworld],
            icon: Some(MapIcon::SlimeChunk),
            group_id: Some(GROUP.into()),
            approximate: false,
            max_scale: Some(SLIME_MAX_SCALE),
            creatable: vec![],
        },
    ]
}

/// Spawn is one biome search per world, so the last few are kept.
type Spawns = Vec<((i64, i32), lumilio_cubiomes::Spawn)>;
static SPAWNS: Mutex<Spawns> = Mutex::new(Vec::new());
const SPAWN_WORLDS: usize = 8;

fn spawn(version: Version, seed: i64) -> lumilio_cubiomes::Spawn {
    let key = (seed, version.index());
    let mut cache = SPAWNS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, found)) = cache.iter().find(|(known, _)| *known == key) {
        return *found;
    }
    let found = lumilio_cubiomes::spawn(version, seed);
    cache.push((key, found));
    if cache.len() > SPAWN_WORLDS {
        cache.remove(0);
    }
    found
}

fn object(
    request: &OverlayRequest,
    id: String,
    raw_id: String,
    kind: MapObjectKind,
    label_id: &str,
    approximate: bool,
) -> MapObject {
    MapObject {
        id,
        raw_id,
        source: SOURCE.into(),
        world: request.context.world.clone(),
        dimension: request.context.dimension.clone(),
        kind,
        label: None,
        label_id: Some(label_id.into()),
        priority: 20,
        approximate,
        color: None,
        note: None,
        share: None,
        editable: vec![],
    }
}

fn contains(bounds: &MapBounds, x: f64, z: f64) -> bool {
    x >= bounds.min.x && x < bounds.max.x && z >= bounds.min.z && z < bounds.max.z
}

pub(crate) fn objects(request: &OverlayRequest) -> Result<Vec<MapObject>, PluginError> {
    let context = &request.context;
    let version = world_version(context)?;
    let (Some(seed), Dimension::Overworld) = (context.seed, &context.dimension) else {
        return Ok(vec![]);
    };
    let bounds = &request.bounds;
    if [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z]
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 30_000_000.)
        || bounds.min.x >= bounds.max.x
        || bounds.min.z >= bounds.max.z
    {
        return Err(PluginError::InvalidInput("invalid landmark bounds".into()));
    }
    match request.overlay.as_str() {
        SPAWN => {
            let found = spawn(version, seed);
            let [x, z] = found.at;
            let (x, z) = (f64::from(x), f64::from(z));
            Ok(if contains(bounds, x, z) {
                vec![object(
                    request,
                    format!("{SPAWN}:{}:{}", found.at[0], found.at[1]),
                    format!("{},{}", found.at[0], found.at[1]),
                    MapObjectKind::Icon {
                        icon: MapIcon::Spawn,
                        at: MapPoint { x, z },
                    },
                    "map-world-spawn",
                    found.estimated,
                )]
            } else {
                vec![]
            })
        }
        SLIME => slime(request, seed),
        _ => Err(PluginError::InvalidInput("unknown landmark layer".into())),
    }
}

/// One `Heat` object per request holding the corner of every slime chunk in
/// the bounds, so a cell is a single object however many chunks it has.
fn slime(request: &OverlayRequest, seed: i64) -> Result<Vec<MapObject>, PluginError> {
    let bounds = &request.bounds;
    let first = |edge: f64| (edge / 16.).floor() as i32;
    let last = |edge: f64| (edge / 16.).ceil() as i32;
    let (cx, cz) = (first(bounds.min.x), first(bounds.min.z));
    let (width, height) = (last(bounds.max.x) - cx, last(bounds.max.z) - cz);
    let flags = lumilio_cubiomes::slime_chunks(seed, cx, cz, width, height)
        .map_err(|_| PluginError::InvalidInput("slime area too large".into()))?;
    let values: Vec<(MapPoint, f32)> = flags
        .iter()
        .enumerate()
        .filter(|(_, slime)| **slime)
        .map(|(at, _)| {
            let (x, z) = (at as i32 % width, at as i32 / width);
            (
                MapPoint {
                    x: f64::from((cx + x) * 16),
                    z: f64::from((cz + z) * 16),
                },
                1.,
            )
        })
        .collect();
    if values.is_empty() {
        return Ok(vec![]);
    }
    let mut found = object(
        request,
        format!("{SLIME}:{cx}:{cz}"),
        format!("{cx},{cz}"),
        MapObjectKind::Heat { cell: 16., values },
        "map-world-slime",
        false,
    );
    found.priority = -10;
    found.color = Some([88, 214, 74]);
    Ok(vec![found])
}
