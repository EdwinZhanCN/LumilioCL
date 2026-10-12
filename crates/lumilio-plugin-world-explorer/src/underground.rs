//! Underground layers of the Overworld seed map: where large ore veins run.
//! A vein zone comes from noise alone, so it is exact for the seed; which
//! blocks inside it become ore depends on stone being there.
use crate::structures::world_version;
use lumilio_cubiomes::Vein;
use lumilio_plugin_api::map::{
    Dimension, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext,
};
use lumilio_plugin_api::{HostContext, PluginError};

pub(crate) const GROUP: &str = "map-group-underground";
const SOURCE: &str = crate::ID;
/// Zone squares, in blocks: a vein is tens of blocks across.
const CELL: i32 = 8;
/// Zones read at one column per square; coarser than this they are noise.
const MAX_SCALE: u32 = 4;

struct Layer {
    id: &'static str,
    label_id: &'static str,
    vein: Vein,
    icon: MapIcon,
    color: [u8; 3],
}

const LAYERS: [Layer; 2] = [
    Layer {
        id: "underground.copper-vein",
        label_id: "map-underground-copper-vein",
        vein: Vein::Copper,
        icon: MapIcon::CopperVein,
        color: [222, 120, 70],
    },
    Layer {
        id: "underground.iron-vein",
        label_id: "map-underground-iron-vein",
        vein: Vein::Iron,
        icon: MapIcon::IronVein,
        color: [226, 196, 164],
    },
];

pub(crate) fn is_layer(overlay: &str) -> bool {
    LAYERS.iter().any(|layer| layer.id == overlay)
}

fn info(layer: &Layer) -> OverlayInfo {
    OverlayInfo {
        id: layer.id.into(),
        kind_id: layer.label_id.into(),
        dimensions: vec![Dimension::Overworld],
        icon: Some(layer.icon),
        group_id: Some(GROUP.into()),
        approximate: false,
        max_scale: Some(MAX_SCALE),
        creatable: vec![],
    }
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    LAYERS.iter().map(info).collect()
}

/// The Overworld of a seeded world in a release with large veins.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    match (world_version(context), &context.dimension, context.seed) {
        (Ok(version), Dimension::Overworld, Some(_)) if Vein::available(version) => catalog(),
        _ => vec![],
    }
}

/// One `Heat` object for the request: a square per zone cell, stronger where
/// the toggle passes its threshold by more.
pub(crate) fn objects(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
) -> Result<Vec<MapObject>, PluginError> {
    let layer = LAYERS
        .iter()
        .find(|layer| layer.id == request.overlay)
        .ok_or_else(|| PluginError::InvalidInput("unknown underground layer".into()))?;
    let context = &request.context;
    let version = world_version(context)?;
    let (Some(seed), Dimension::Overworld) = (context.seed, &context.dimension) else {
        return Ok(vec![]);
    };
    if !Vein::available(version) {
        return Ok(vec![]);
    }
    let bounds = &request.bounds;
    if [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z]
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 30_000_000.)
        || bounds.min.x >= bounds.max.x
        || bounds.min.z >= bounds.max.z
    {
        return Err(PluginError::InvalidInput("invalid vein bounds".into()));
    }
    let cell = f64::from(CELL);
    let (x0, z0) = (
        (bounds.min.x / cell).floor() as i32,
        (bounds.min.z / cell).floor() as i32,
    );
    let (x1, z1) = (
        (bounds.max.x / cell).ceil() as i32,
        (bounds.max.z / cell).ceil() as i32,
    );
    let size = [x1 - x0, z1 - z0];
    let zones = lumilio_cubiomes::vein_zones(
        version,
        seed,
        layer.vein,
        [x0 * CELL, z0 * CELL],
        CELL,
        size,
        || ctx.cancelled(),
    )
    .map_err(|error| match error {
        lumilio_cubiomes::Error::Cancelled => PluginError::Transient("map-cancelled".into()),
        _ => PluginError::InvalidInput("vein area too large".into()),
    })?;
    let values: Vec<(MapPoint, f32)> = zones
        .iter()
        .enumerate()
        .filter_map(|(at, strength)| {
            let strength = (*strength)?;
            let (i, j) = (at as i32 % size[0], at as i32 / size[0]);
            let point = MapPoint {
                x: f64::from((x0 + i) * CELL),
                z: f64::from((z0 + j) * CELL),
            };
            // Past the threshold by 0.15 is the strongest a vein gets.
            Some((point, 0.35 + 0.65 * (strength / 0.15).min(1.)))
        })
        .collect();
    if values.is_empty() {
        return Ok(vec![]);
    }
    Ok(vec![MapObject {
        id: format!("{}:{x0}:{z0}", layer.id),
        raw_id: format!("{x0},{z0}"),
        source: SOURCE.into(),
        world: context.world.clone(),
        dimension: context.dimension.clone(),
        kind: MapObjectKind::Heat { cell, values },
        label: None,
        label_id: Some(layer.label_id.into()),
        priority: -10,
        approximate: false,
        color: Some(layer.color),
        note: None,
        share: None,
        editable: vec![],
    }])
}
