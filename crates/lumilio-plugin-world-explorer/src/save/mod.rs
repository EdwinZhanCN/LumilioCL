//! The save base map: a single-player world's own region files drawn from
//! above. Only level 0 is drawn here; the host builds the coarser levels from
//! it and keeps them until a region file changes (plan W4, W6).
pub(crate) mod colors;
mod render;

use lumilio_anvil::{Error as AnvilError, Region, Source, region_coords};
use lumilio_plugin_api::map::{
    BaseMapInfo, Dimension, MAX_SOURCES, TileReply, TileRequest, WorldId,
};
use lumilio_plugin_api::{HostContext, MAX_PAGE, MAX_RANGE, PluginError};
use render::View;

pub(crate) const BASE: &str = "save";
/// An external chunk file is read whole; the game writes one only for a
/// chunk over 1 MiB, so this is far above any real one.
const MAX_EXTERNAL: usize = 64 * 1024 * 1024;
/// Region directories larger than this many entries are not listed further.
const MAX_LISTED: usize = 200_000;

pub(crate) fn info() -> BaseMapInfo {
    BaseMapInfo {
        id: BASE.into(),
        kind_id: "map-base-save".into(),
        dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
        levels: vec![0],
    }
}

/// Where a dimension's region files may be: 26.x keeps every dimension under
/// `dimensions/<namespace>/<path>/`; earlier versions keep the Overworld at
/// the top, the Nether in `DIM-1` and the End in `DIM1`. Both are named, the
/// newer first, so a world from either era is found.
fn region_dirs(folder: &str, dimension: &Dimension) -> Vec<String> {
    let world = format!("saves/{folder}");
    let modern = |id: &str| {
        let (namespace, path) = id.split_once(':').unwrap_or(("minecraft", id));
        format!("{world}/dimensions/{namespace}/{path}/region")
    };
    match dimension {
        Dimension::Overworld => vec![modern("overworld"), format!("{world}/region")],
        Dimension::Nether => vec![modern("the_nether"), format!("{world}/DIM-1/region")],
        Dimension::End => vec![modern("the_end"), format!("{world}/DIM1/region")],
        Dimension::Custom(id) => vec![modern(id)],
    }
}

fn folder(request: &TileRequest) -> Option<&str> {
    match &request.context.world {
        WorldId::Save { folder, .. } => Some(folder),
        _ => None,
    }
}

/// The region range `[x0, x1] × [z0, z1]` a tile covers.
fn regions(request: &TileRequest) -> Option<[i32; 4]> {
    let span = i64::from(request.key.blocks_per_pixel()?) * 256;
    let range = |tile: i32| {
        let start = i64::from(tile) * span;
        let first = start.div_euclid(512);
        let last = (start + span - 1).div_euclid(512);
        Some((i32::try_from(first).ok()?, i32::try_from(last).ok()?))
    };
    let (x0, x1) = range(request.key.tx)?;
    let (z0, z1) = range(request.key.tz)?;
    Some([x0, z0, x1, z1])
}

/// The region files a tile is drawn from. Level 0 names its one region in
/// every place it may be, existing or not; a coarser tile names the region
/// files that exist in its area.
pub(crate) fn sources(
    ctx: &dyn HostContext,
    request: &TileRequest,
) -> Result<Option<Vec<String>>, PluginError> {
    let Some(folder) = folder(request) else {
        return Ok(Some(Vec::new()));
    };
    let [x0, z0, x1, z1] =
        regions(request).ok_or_else(|| PluginError::InvalidInput("invalid save tile".into()))?;
    let dirs = region_dirs(folder, &request.key.dimension);
    if request.key.level == 0 {
        return Ok(Some(
            dirs.iter()
                .map(|dir| format!("{dir}/r.{x0}.{z0}.mca"))
                .collect(),
        ));
    }
    let mut found = Vec::new();
    for dir in &dirs {
        let mut after: Option<String> = None;
        let mut listed = 0;
        loop {
            if ctx.cancelled() {
                return Err(PluginError::Transient("map-cancelled".into()));
            }
            let page = ctx.list_dir(dir, after.as_deref(), MAX_PAGE)?;
            listed += page.entries.len();
            for entry in &page.entries {
                if let Some((x, z)) = region_coords(&entry.name)
                    && !entry.is_dir
                    && (x0..=x1).contains(&x)
                    && (z0..=z1).contains(&z)
                {
                    found.push(format!("{dir}/{}", entry.name));
                }
            }
            match page.next {
                Some(next) if listed < MAX_LISTED => after = Some(next),
                _ => break,
            }
        }
    }
    found.truncate(MAX_SOURCES);
    Ok(Some(found))
}

/// A region file read through the host, one byte range at a time.
struct HostRegion<'a> {
    ctx: &'a dyn HostContext,
    path: String,
    dir: String,
}

impl<'a> HostRegion<'a> {
    fn new(ctx: &'a dyn HostContext, path: String) -> Self {
        let dir = path
            .rsplit_once('/')
            .map_or_else(String::new, |(dir, _)| dir.to_owned());
        Self { ctx, path, dir }
    }
}

impl Source for HostRegion<'_> {
    fn read(&self, offset: u64, len: usize) -> Result<Vec<u8>, AnvilError> {
        let mut out = Vec::with_capacity(len.min(MAX_RANGE));
        while out.len() < len {
            let want = (len - out.len()).min(MAX_RANGE);
            let part = self
                .ctx
                .read_range(&self.path, offset + out.len() as u64, want)
                .map_err(|error| AnvilError::Source(error.to_string()))?;
            let short = part.len() < want;
            out.extend(part);
            if short {
                break;
            }
        }
        Ok(out)
    }

    fn external(&self, name: &str) -> Result<Option<Vec<u8>>, AnvilError> {
        let path = format!("{}/{name}", self.dir);
        let missing = self
            .ctx
            .file_stat(&path)
            .map_err(|error| AnvilError::Source(error.to_string()))?
            .is_none();
        if missing {
            return Ok(None);
        }
        let bytes = HostRegion::new(self.ctx, path).read(0, MAX_EXTERNAL + 1)?;
        if bytes.len() > MAX_EXTERNAL {
            return Err(AnvilError::Format("external chunk is too large"));
        }
        Ok(Some(bytes))
    }
}

/// A level-0 tile: a quarter of one region file.
pub(crate) fn tile(ctx: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError> {
    if request.key.level != 0 {
        return Err(PluginError::InvalidInput("map-save-level".into()));
    }
    let Some(folder) = folder(request) else {
        return Ok(TileReply::Empty);
    };
    let [rx, rz, ..] =
        regions(request).ok_or_else(|| PluginError::InvalidInput("invalid save tile".into()))?;
    let view = match request.key.dimension {
        // The Nether's bedrock ceiling spans y 123–127.
        Dimension::Nether => View::Roofed(127),
        _ => View::Open,
    };
    let table = colors::pick(
        colors::tables(),
        request.context.data_version,
        request.context.version.as_deref(),
    );
    for dir in region_dirs(folder, &request.key.dimension) {
        let path = format!("{dir}/r.{rx}.{rz}.mca");
        if ctx.file_stat(&path)?.is_none() {
            continue;
        }
        let region = Region::open(HostRegion::new(ctx, path), rx, rz)
            .map_err(|_| PluginError::Unavailable("map-save-unreadable".into()))?;
        let quarter = (
            (request.key.tx.rem_euclid(2) * 16) as usize,
            (request.key.tz.rem_euclid(2) * 16) as usize,
        );
        return render::draw(&region, quarter, table, view, &|| ctx.cancelled());
    }
    Ok(TileReply::Empty)
}

#[cfg(test)]
mod tests;
