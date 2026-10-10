//! Read-only placement footprints from Litematica's per-world config files.
//! The corner transformation is adapted from Litematica's
//! `SchematicPlacement.getSelectionBoxForRegion` (maruohon/litematica,
//! `src/main/java/litematica/schematic/placement/SchematicPlacement.java`,
//! LGPL-3.0, copyright Litematica contributors). See ATTRIBUTIONS.md.
use crate::{FOLDER, ID, Litematica, format};
use lumilio_plugin_api::map::{
    Dimension, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayProvider, OverlayRequest,
    WorldId,
};
use lumilio_plugin_api::{HostContext, PluginError};
use serde_json::Value;

#[cfg(test)]
mod tests;

const LAYER: &str = "litematica.placements";
const CONFIG: &str = "config/litematica";
const LIMIT: usize = 256;

impl OverlayProvider for Litematica {
    fn overlays(&self) -> Vec<OverlayInfo> {
        // ia[plugin.world-explorer]: 查看 Litematica 投影范围 | 地图图层 ·「Litematica 投影位置」 | 按当前世界和维度读取已启用投影，在底图上画出各子区域的边框和位置
        vec![OverlayInfo {
            id: LAYER.into(),
            kind_id: "map-litematica-placements".into(),
            dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
            icon: None,
            group_id: Some("map-group-world".into()),
            approximate: false,
            max_scale: None,
            creatable: vec![],
        }]
    }

    fn objects(
        &self,
        ctx: &dyn HostContext,
        request: &OverlayRequest,
    ) -> Result<Vec<MapObject>, PluginError> {
        if request.overlay != LAYER {
            return Err(PluginError::InvalidInput(
                "unknown Litematica overlay".into(),
            ));
        }
        let Some(path) = config_path(&request.context.world, &request.context.dimension) else {
            return Ok(vec![]);
        };
        if !ctx.list_files(CONFIG)?.contains(&path) {
            return Ok(vec![]);
        }
        let bytes = ctx.read_file(&path)?;
        let document: Value = serde_json::from_slice(&bytes)
            .map_err(|_| PluginError::Unavailable("map-litematica-unreadable".into()))?;
        let placements = document
            .pointer("/placements/placements")
            .and_then(Value::as_array)
            .ok_or_else(|| PluginError::Unavailable("map-litematica-unreadable".into()))?;
        let available = ctx.list_files(FOLDER)?;
        let mut objects = Vec::new();
        for (index, placement) in placements.iter().take(LIMIT).enumerate() {
            if ctx.cancelled() {
                return Err(PluginError::Transient("map-cancelled".into()));
            }
            if placement.get("enabled").and_then(Value::as_bool) != Some(true) {
                continue;
            }
            let Some(schematic) = placement.get("schematic").and_then(Value::as_str) else {
                continue;
            };
            let Some(file) = schematic
                .replace('\\', "/")
                .rsplit('/')
                .next()
                .map(str::to_owned)
            else {
                continue;
            };
            if !file.ends_with(".litematic") || file == ".litematic" {
                continue;
            }
            let path = format!("{FOLDER}/{file}");
            if !available.contains(&path) {
                continue;
            }
            let bytes = ctx.read_file(&path)?;
            let model = format::parse(&bytes)
                .map_err(|_| PluginError::Unavailable("map-litematica-unreadable".into()))?;
            let Some(origin) = placement.get("origin").and_then(vector) else {
                continue;
            };
            let Some(global) = transform(placement) else {
                continue;
            };
            let name = placement
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(file.trim_end_matches(".litematic"));
            let color = placement
                .get("bb_color")
                .and_then(Value::as_i64)
                .map(|value| {
                    [
                        ((value >> 16) & 255) as u8,
                        ((value >> 8) & 255) as u8,
                        (value & 255) as u8,
                    ]
                });
            let Some(regions) = placement.get("placements").and_then(Value::as_array) else {
                continue;
            };
            for (region_index, region) in regions.iter().take(LIMIT).enumerate() {
                if objects.len() >= LIMIT {
                    break;
                }
                let Some(region_name) = region.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let Some(size) = model.region_size(region_name) else {
                    continue;
                };
                let Some(inner) = region.get("placement") else {
                    continue;
                };
                if inner.get("enabled").and_then(Value::as_bool) != Some(true) {
                    continue;
                }
                let Some(relative) = inner.get("pos").and_then(vector) else {
                    continue;
                };
                let Some(local) = transform(inner) else {
                    continue;
                };
                let Some(box_points) = footprint(origin, relative, size, global, local) else {
                    continue;
                };
                let (min, max) = (box_points[0], box_points[2]);
                if max.x < request.bounds.min.x
                    || min.x >= request.bounds.max.x
                    || max.z < request.bounds.min.z
                    || min.z >= request.bounds.max.z
                {
                    continue;
                }
                let raw_id = format!("{index}:{region_index}");
                objects.push(MapObject {
                    id: format!("{LAYER}:{raw_id}"),
                    raw_id,
                    source: ID.into(),
                    world: request.context.world.clone(),
                    dimension: request.context.dimension.clone(),
                    kind: MapObjectKind::Polyline(box_points),
                    label: Some(if regions.len() == 1 {
                        name.to_owned()
                    } else {
                        format!("{name} · {region_name}")
                    }),
                    label_id: None,
                    priority: 20,
                    approximate: false,
                    color,
                    note: Some(format!(
                        "Y {} · {}×{}×{}",
                        origin[1],
                        size.0.unsigned_abs(),
                        size.1.unsigned_abs(),
                        size.2.unsigned_abs()
                    )),
                    share: None,
                    editable: vec![],
                });
            }
        }
        Ok(objects)
    }
}

fn config_path(world: &WorldId, dimension: &Dimension) -> Option<String> {
    let world = match world {
        WorldId::Save { folder, .. } => folder,
        WorldId::Server { address, .. } => address,
        WorldId::Seed { .. } => return None,
    };
    if world.contains(['/', '\\']) || world.contains("..") {
        return None;
    }
    let dimension = match dimension {
        Dimension::Overworld => "overworld",
        Dimension::Nether => "the_nether",
        Dimension::End => "the_end",
        Dimension::Custom(_) => return None,
    };
    Some(format!(
        "{CONFIG}/litematica_{world}_dim_minecraft_{dimension}.json"
    ))
}

fn vector(value: &Value) -> Option<[i64; 3]> {
    let array = value.as_array()?;
    Some([
        array.first()?.as_i64()?,
        array.get(1)?.as_i64()?,
        array.get(2)?.as_i64()?,
    ])
}

#[derive(Clone, Copy)]
struct Transform {
    mirror: &'static str,
    rotation: &'static str,
}

fn transform(value: &Value) -> Option<Transform> {
    let mirror = match value.get("mirror")?.as_str()? {
        "NONE" => "NONE",
        "FRONT_BACK" => "FRONT_BACK",
        "LEFT_RIGHT" => "LEFT_RIGHT",
        _ => return None,
    };
    let rotation = match value.get("rotation")?.as_str()? {
        "NONE" => "NONE",
        "CLOCKWISE_90" => "CLOCKWISE_90",
        "CLOCKWISE_180" => "CLOCKWISE_180",
        "COUNTERCLOCKWISE_90" => "COUNTERCLOCKWISE_90",
        _ => return None,
    };
    Some(Transform { mirror, rotation })
}

fn apply([x, y, z]: [i64; 3], transform: Transform) -> [i64; 3] {
    let (x, z) = match transform.mirror {
        "FRONT_BACK" => (-x, z),
        "LEFT_RIGHT" => (x, -z),
        _ => (x, z),
    };
    let (x, z) = match transform.rotation {
        "CLOCKWISE_90" => (-z, x),
        "CLOCKWISE_180" => (-x, -z),
        "COUNTERCLOCKWISE_90" => (z, -x),
        _ => (x, z),
    };
    [x, y, z]
}

fn footprint(
    origin: [i64; 3],
    relative: [i64; 3],
    size: (i64, i64, i64),
    global: Transform,
    local: Transform,
) -> Option<Vec<MapPoint>> {
    if origin
        .into_iter()
        .chain(relative)
        .chain([size.0, size.1, size.2])
        .any(|coord| coord.unsigned_abs() > 30_000_001)
    {
        return None;
    }
    let moved = apply(relative, global);
    let start = [
        origin[0].checked_add(moved[0])?,
        origin[1].checked_add(moved[1])?,
        origin[2].checked_add(moved[2])?,
    ];
    let end = |size: i64| size.checked_sub(size.signum());
    let far = apply(
        apply([end(size.0)?, end(size.1)?, end(size.2)?], global),
        local,
    );
    let x1 = start[0].checked_add(far[0])?;
    let z1 = start[2].checked_add(far[2])?;
    let (x0, x1) = (start[0].min(x1), start[0].max(x1).checked_add(1)?);
    let (z0, z1) = (start[2].min(z1), start[2].max(z1).checked_add(1)?);
    if [x0, x1, z0, z1]
        .iter()
        .any(|coord| coord.unsigned_abs() > 30_000_001)
    {
        return None;
    }
    let point = |x, z| MapPoint {
        x: x as f64,
        z: z as f64,
    };
    Some(vec![
        point(x0, z0),
        point(x1, z0),
        point(x1, z1),
        point(x0, z1),
        point(x0, z0),
    ])
}
