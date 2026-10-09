//! The "Xaero waypoints" layer: the waypoints of the Minimap world directory a
//! world is linked to, in the dimension on screen.
use super::naming::{self, MINIMAP_ROOT};
use super::waypoints::{DEFAULT_SET, Waypoint, WaypointFile, color_rgb, share_string};
use lumilio_plugin_api::map::{
    Dimension, MapIcon, MapObject, MapObjectKind, MapPoint, OverlayInfo, OverlayRequest,
    SourceLink, WorldContext,
};
use lumilio_plugin_api::{HostContext, PluginError};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

pub(crate) const GROUP: &str = "map-group-xaero";
pub(crate) const LAYER: &str = "xaero.waypoints";
const SOURCE: &str = crate::ID;

pub(crate) fn is_layer(overlay: &str) -> bool {
    overlay == LAYER
}

fn info() -> OverlayInfo {
    OverlayInfo {
        id: LAYER.into(),
        kind_id: "map-xaero-waypoints".into(),
        dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
        icon: Some(MapIcon::Waypoint),
        group_id: Some(GROUP.into()),
        approximate: false,
        max_scale: None,
        creatable: super::edit::fields(None, None),
    }
}

pub(crate) fn catalog() -> Vec<OverlayInfo> {
    vec![info()]
}

/// The Minimap directory a world was linked to, if it was.
pub(crate) fn world_dir(context: &WorldContext) -> Option<&str> {
    context.sources.iter().find_map(|source| match source {
        SourceLink::XaeroMinimap(dir) => Some(dir.as_str()),
        _ => None,
    })
}

/// Offered only for a world with a linked Minimap directory.
pub(crate) fn catalog_for(context: &WorldContext) -> Vec<OverlayInfo> {
    world_dir(context).map_or_else(Vec::new, |_| catalog())
}

type Files = Vec<(String, WaypointFile)>;
type Recent = Option<((String, Dimension), Instant, Files)>;

/// A view asks for dozens of 4096-block cells at once, each wanting the same
/// few small files; the last read is reused for this long.
static RECENT: Mutex<Recent> = Mutex::new(None);
const REUSE: Duration = Duration::from_secs(1);

/// Drops the reused read; called after the files change.
pub(crate) fn forget_recent() {
    *RECENT.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

/// Each waypoint file of the world's directory in this dimension, read and
/// parsed. A file that cannot be read or has lines that do not parse makes the
/// whole layer unavailable rather than showing a partial list as if complete.
fn read_files(
    ctx: &dyn HostContext,
    dir: &str,
    dimension: &Dimension,
) -> Result<Files, PluginError> {
    let key = (dir.to_owned(), dimension.clone());
    if let Some((known, at, files)) = RECENT
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
        && *known == key
        && at.elapsed() < REUSE
    {
        return Ok(files.clone());
    }
    let files = read_uncached(ctx, dir, dimension)?;
    *RECENT.lock().unwrap_or_else(PoisonError::into_inner) =
        Some((key, Instant::now(), files.clone()));
    Ok(files)
}

fn read_uncached(
    ctx: &dyn HostContext,
    dir: &str,
    dimension: &Dimension,
) -> Result<Files, PluginError> {
    let listed = ctx.list_files(&format!("{MINIMAP_ROOT}/{dir}"))?;
    let mut files = Vec::new();
    for path in &listed {
        let Some((found, name)) = naming::waypoint_file(path, dir) else {
            continue;
        };
        if &found != dimension {
            continue;
        }
        if ctx.cancelled() {
            return Err(PluginError::Transient("map-cancelled".into()));
        }
        let bytes = ctx.read_file(path)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| PluginError::Unavailable("map-xaero-unreadable".into()))?;
        let file = WaypointFile::parse(&text);
        if file.malformed() > 0 {
            return Err(PluginError::Unavailable("map-xaero-unreadable".into()));
        }
        files.push((name.to_owned(), file));
    }
    Ok(files)
}

fn object(
    request: &OverlayRequest,
    file: &str,
    index: usize,
    raw: &str,
    waypoint: &Waypoint,
) -> Option<MapObject> {
    let (x, z) = (f64::from(waypoint.x), f64::from(waypoint.z));
    let bounds = &request.bounds;
    if x < bounds.min.x || x >= bounds.max.x || z < bounds.min.z || z >= bounds.max.z {
        return None;
    }
    let death = waypoint.is_death();
    let label_id = match (death, waypoint.disabled) {
        (true, _) => "map-xaero-death",
        (false, true) => "map-xaero-waypoint-disabled",
        (false, false) => "map-xaero-waypoint",
    };
    Some(MapObject {
        id: super::edit::object_id(file, index, raw),
        raw_id: format!("{file}#{index}"),
        source: SOURCE.into(),
        world: request.context.world.clone(),
        dimension: request.context.dimension.clone(),
        kind: MapObjectKind::Icon {
            icon: if death {
                MapIcon::Death
            } else {
                MapIcon::Waypoint
            },
            at: MapPoint { x, z },
        },
        label: Some(waypoint.name.clone()),
        label_id: Some(label_id.into()),
        priority: if waypoint.disabled { -5 } else { 5 },
        approximate: false,
        color: Some(color_rgb(waypoint.color)),
        note: (waypoint.set != DEFAULT_SET).then(|| waypoint.set.clone()),
        share: Some(share_string(waypoint, &request.context.dimension)),
        editable: super::edit::fields(Some(waypoint), None),
    })
}

pub(crate) fn objects(
    ctx: &dyn HostContext,
    request: &OverlayRequest,
) -> Result<Vec<MapObject>, PluginError> {
    let Some(dir) = world_dir(&request.context) else {
        return Ok(vec![]);
    };
    let bounds = &request.bounds;
    if [bounds.min.x, bounds.min.z, bounds.max.x, bounds.max.z]
        .iter()
        .any(|edge| !edge.is_finite())
    {
        return Err(PluginError::InvalidInput("invalid waypoint bounds".into()));
    }
    let files = read_files(ctx, dir, &request.context.dimension)?;
    Ok(files
        .iter()
        .flat_map(|(name, file)| {
            file.lines()
                .iter()
                .enumerate()
                .filter_map(move |(index, line)| {
                    object(request, name, index, line.raw(), line.waypoint.as_ref()?)
                })
        })
        .collect())
}
