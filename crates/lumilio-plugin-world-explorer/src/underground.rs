//! Underground layers of the Overworld seed map: where large ore veins and
//! ravines run. Both come from the seed alone, so they are exact; which
//! blocks they leave as ore or open as air depends on the terrain there.
use crate::structures::world_version;
use lumilio_cubiomes::{Vein, Version};
use lumilio_plugin_api::map::{
    Dimension, MapBounds, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    WorldContext,
};
use lumilio_plugin_api::{HostContext, PluginError};

pub(crate) const GROUP: &str = "map-group-underground";
const SOURCE: &str = crate::ID;
/// Zone squares, in blocks: a vein is tens of blocks across, and the host
/// draws at most a few thousand squares a frame.
const CELL: i32 = 16;

#[derive(Clone, Copy)]
enum Kind {
    Vein(Vein),
    Ravines,
}

struct Layer {
    id: &'static str,
    label_id: &'static str,
    kind: Kind,
    icon: MapIcon,
    color: [u8; 3],
    /// Blocks per pixel past which the layer is not drawn.
    max_scale: u32,
}

const LAYERS: [Layer; 3] = [
    Layer {
        id: "underground.copper-vein",
        label_id: "map-underground-copper-vein",
        kind: Kind::Vein(Vein::Copper),
        icon: MapIcon::CopperVein,
        color: [222, 120, 70],
        max_scale: 2,
    },
    Layer {
        id: "underground.iron-vein",
        label_id: "map-underground-iron-vein",
        kind: Kind::Vein(Vein::Iron),
        icon: MapIcon::IronVein,
        color: [226, 196, 164],
        max_scale: 2,
    },
    Layer {
        id: "underground.ravine",
        label_id: "map-underground-ravine",
        kind: Kind::Ravines,
        icon: MapIcon::Ravine,
        color: [120, 96, 82],
        // One ravine per hundred chunks, each a line of about a hundred points.
        max_scale: 4,
    },
];
/// The colour of a ravine's walk under an ocean.
const UNDERWATER: [u8; 3] = [64, 120, 196];

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
        max_scale: Some(layer.max_scale),
        creatable: vec![],
    }
}

fn offered(layer: &Layer, version: Version) -> bool {
    match layer.kind {
        Kind::Vein(_) => Vein::available(version),
        Kind::Ravines => lumilio_cubiomes::ravines_available(version),
    }
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    LAYERS.iter().map(info).collect()
}

/// The Overworld of a seeded world in a release with these features.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    let (Ok(version), Dimension::Overworld, Some(_)) =
        (world_version(context), &context.dimension, context.seed)
    else {
        return vec![];
    };
    LAYERS
        .iter()
        .filter(|layer| offered(layer, version))
        .map(info)
        .collect()
}

fn checked(bounds: &MapBounds) -> Result<(), PluginError> {
    if [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z]
        .iter()
        .any(|edge| !edge.is_finite() || edge.abs() > 30_000_000.)
        || bounds.min.x >= bounds.max.x
        || bounds.min.z >= bounds.max.z
    {
        return Err(PluginError::InvalidInput(
            "invalid underground bounds".into(),
        ));
    }
    Ok(())
}

fn failure(error: lumilio_cubiomes::Error) -> PluginError {
    match error {
        lumilio_cubiomes::Error::Cancelled => PluginError::Transient("map-cancelled".into()),
        _ => PluginError::InvalidInput("underground area too large".into()),
    }
}

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
    if !offered(layer, version) {
        return Ok(vec![]);
    }
    checked(&request.bounds)?;
    match layer.kind {
        Kind::Vein(vein) => veins(ctx, request, layer, version, seed, vein),
        Kind::Ravines => ravines(ctx, request, layer, version, seed),
    }
}

/// Each ravine starting in the request: its start, and its walk as a line.
fn ravines(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
    layer: &Layer,
    version: Version,
    seed: i64,
) -> Result<Vec<MapObject>, PluginError> {
    let bounds = &request.bounds;
    let area = [
        bounds.min.x.floor() as i32,
        bounds.min.z.floor() as i32,
        bounds.max.x.ceil() as i32,
        bounds.max.z.ceil() as i32,
    ];
    let found =
        lumilio_cubiomes::ravines(version, seed, area, || ctx.cancelled()).map_err(failure)?;
    let context = &request.context;
    let mut objects = vec![];
    for ravine in found {
        let [x, _, z] = ravine.start;
        let (icon, label_id, color) = if ravine.underwater {
            (
                MapIcon::UnderwaterRavine,
                "map-underground-underwater-ravine",
                UNDERWATER,
            )
        } else {
            (layer.icon, layer.label_id, layer.color)
        };
        let point = |[x, z]: [i32; 2]| MapPoint {
            x: f64::from(x) + 0.5,
            z: f64::from(z) + 0.5,
        };
        let object = |id: String, kind: MapObjectKind, priority: i32| MapObject {
            id,
            raw_id: format!("{x},{z}"),
            source: SOURCE.into(),
            world: context.world.clone(),
            dimension: context.dimension.clone(),
            kind,
            label: None,
            label_id: Some(label_id.into()),
            priority,
            approximate: false,
            color: Some(color),
            note: None,
            share: None,
            editable: vec![],
        };
        objects.push(object(
            format!("{}:{x}:{z}:path", layer.id),
            MapObjectKind::Polyline(ravine.path.iter().copied().map(point).collect()),
            -5,
        ));
        objects.push(object(
            format!("{}:{x}:{z}", layer.id),
            MapObjectKind::Icon {
                icon,
                at: point([x, z]),
            },
            0,
        ));
    }
    Ok(objects)
}

/// One `Heat` object for the request: a square per zone cell, stronger where
/// the toggle passes its threshold by more.
fn veins(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
    layer: &Layer,
    version: Version,
    seed: i64,
    vein: Vein,
) -> Result<Vec<MapObject>, PluginError> {
    let context = &request.context;
    let bounds = &request.bounds;
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
        vein,
        [x0 * CELL, z0 * CELL],
        CELL,
        size,
        || ctx.cancelled(),
    )
    .map_err(failure)?;
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
