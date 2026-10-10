//! Xaero World Map region tiles. Region framing and palette decoding use the
//! MIT-licensed XaeroTools `crates/xaero-core/src/{codec,model}` snapshot at
//! `7bc650bdf445ec06d0d3fc0fb9e98ba4b86a6b83` (see ATTRIBUTIONS.md).
//! Color lookup uses LumilioCL's generated versioned block tables.
use crate::save::colors;
use lumilio_plugin_api::map::{
    BaseMapInfo, Dimension, SourceLink, TILE_PIXELS, TileReply, TileRequest, WorldId,
};
use lumilio_plugin_api::{HostContext, ImageData, PluginError};
use std::collections::BTreeSet;
use xaero_core::model::{BiomeRef, DecodedRegion, Pixel};

#[cfg(test)]
mod tests;

pub(crate) const BASE: &str = "xaero";
const MAX_ZIP: u64 = 64 << 20;
const RANGE: usize = 8 << 20;

pub(crate) fn info() -> BaseMapInfo {
    BaseMapInfo {
        id: BASE.into(),
        kind_id: "map-base-xaero".into(),
        dimensions: vec![Dimension::Overworld, Dimension::Nether, Dimension::End],
        levels: vec![0],
    }
}

fn dimension_dir(dimension: &Dimension) -> Option<&'static str> {
    match dimension {
        Dimension::Overworld => Some("null"),
        Dimension::Nether => Some("DIM-1"),
        Dimension::End => Some("DIM1"),
        Dimension::Custom(_) => None,
    }
}

/// The tile key carries the person's choice, giving each multiworld id its
/// own cache entries. Only ids linked to this context are accepted.
fn region_dir(request: &TileRequest) -> Result<Option<String>, PluginError> {
    let Some(dimension) = dimension_dir(&request.key.dimension) else {
        return Ok(None);
    };
    let candidates: Vec<&str> = request
        .context
        .sources
        .iter()
        .filter_map(|source| {
            let SourceLink::XaeroWorldMap(path) = source else {
                return None;
            };
            let mut parts = path.split('/');
            let (Some(_world), Some(found_dimension), Some(id), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return None;
            };
            (found_dimension == dimension && id.starts_with("mw$")).then_some(path.as_str())
        })
        .collect();
    let selected = if let Some(selected) = request.key.base_map.strip_prefix("xaero@") {
        if !candidates.contains(&selected) {
            return Err(PluginError::InvalidInput("unlinked Xaero map".into()));
        }
        selected
    } else {
        match candidates.as_slice() {
            [] => return Ok(None),
            [only] => *only,
            _ => return Err(PluginError::Unavailable("map-xaero-choose-map".into())),
        }
    };
    Ok(Some(format!("xaero/world-map/{selected}")))
}

pub(crate) fn sources(
    ctx: &dyn HostContext,
    request: &TileRequest,
) -> Result<Option<Vec<String>>, PluginError> {
    if !valid_request(request) {
        return Err(PluginError::InvalidInput("invalid Xaero tile".into()));
    }
    let Some(dir) = region_dir(request)? else {
        return Ok(Some(vec![]));
    };
    let scale = i64::from(request.key.blocks_per_pixel().unwrap());
    let span = 256 * scale;
    let x0 = i64::from(request.key.tx) * span;
    let z0 = i64::from(request.key.tz) * span;
    let (rx0, rz0, rx1, rz1) = (
        x0.div_euclid(512),
        z0.div_euclid(512),
        (x0 + span - 1).div_euclid(512),
        (z0 + span - 1).div_euclid(512),
    );
    if request.key.level == 0 {
        return Ok(Some(vec![format!("{dir}/{rx0}_{rz0}.zip")]));
    }
    let mut files = Vec::new();
    let mut after = None;
    loop {
        if ctx.cancelled() {
            return Err(PluginError::Transient("map-cancelled".into()));
        }
        let page = ctx.list_dir(&dir, after.as_deref(), 1000)?;
        for entry in page.entries {
            if !entry.is_dir
                && let Some((rx, rz)) = entry
                    .name
                    .strip_suffix(".zip")
                    .and_then(|stem| stem.split_once('_'))
                    .and_then(|(rx, rz)| Some((rx.parse::<i64>().ok()?, rz.parse::<i64>().ok()?)))
                && (rx0..=rx1).contains(&rx)
                && (rz0..=rz1).contains(&rz)
            {
                files.push(format!("{dir}/{}", entry.name));
            }
        }
        if files.len() >= lumilio_plugin_api::map::MAX_SOURCES {
            files.truncate(lumilio_plugin_api::map::MAX_SOURCES);
            break;
        }
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    Ok(Some(files))
}

fn valid_request(request: &TileRequest) -> bool {
    (request.key.base_map == BASE || request.key.base_map.starts_with("xaero@"))
        && request.key.provider == crate::ID
        && request.key.blocks_per_pixel().is_some()
        && request.pixels == TILE_PIXELS
        && request.context.world == request.key.world
        && request.context.dimension == request.key.dimension
        && matches!(
            request.key.world,
            WorldId::Save { .. } | WorldId::Server { .. }
        )
}

fn read_zip(ctx: &dyn HostContext, path: &str) -> Result<Option<Vec<u8>>, PluginError> {
    let Some(stat) = ctx.file_stat(path)? else {
        return Ok(None);
    };
    if stat.len > MAX_ZIP {
        return Err(PluginError::Unavailable(
            "map-xaero-region-too-large".into(),
        ));
    }
    let mut bytes = Vec::with_capacity(stat.len as usize);
    while (bytes.len() as u64) < stat.len {
        if ctx.cancelled() {
            return Err(PluginError::Transient("map-cancelled".into()));
        }
        let read = (stat.len - bytes.len() as u64).min(RANGE as u64) as usize;
        let chunk = ctx.read_range(path, bytes.len() as u64, read)?;
        if chunk.is_empty() {
            return Err(PluginError::Unavailable("map-xaero-unreadable".into()));
        }
        bytes.extend(chunk);
    }
    Ok(Some(bytes))
}

fn pixel(region: &DecodedRegion, x: usize, z: usize) -> Option<&Pixel> {
    let chunk = region.region.chunk((x / 64) as u8, (z / 64) as u8)?;
    let tile = chunk
        .tiles
        .get((x % 64 / 16) * 4 + (z % 64 / 16))?
        .as_ref()?;
    tile.pixels.get((x % 16) * 16 + (z % 16))
}

fn block_name<'a>(region: &'a DecodedRegion, pixel: &Pixel) -> &'a str {
    pixel
        .state
        .and_then(|index| region.palettes.state_names.get(index as usize))
        .map(String::as_str)
        .unwrap_or("minecraft:grass_block")
}

fn biome_name<'a>(region: &'a DecodedRegion, pixel: &Pixel) -> Option<&'a str> {
    match pixel.biome {
        Some(BiomeRef::Palette(index)) => region
            .palettes
            .biome_names
            .get(index as usize)
            .map(String::as_str),
        None => None,
    }
}

fn shade(rgb: [u8; 3], height: i16, north: i16) -> [u8; 3] {
    let factor: u16 = if height > north {
        255
    } else if height < north {
        180
    } else {
        220
    };
    rgb.map(|value| (u16::from(value) * factor / 255) as u8)
}

fn render_quarter(region: &DecodedRegion, request: &TileRequest) -> TileReply {
    let table = colors::pick(
        colors::tables(),
        request.context.data_version,
        request.context.version.as_deref(),
    );
    let mut rgba = vec![0u8; (TILE_PIXELS * TILE_PIXELS * 4) as usize];
    let mut coverage = vec![0u8; (TILE_PIXELS * TILE_PIXELS) as usize];
    let mut unknown = BTreeSet::new();
    let x0 = request.key.tx.rem_euclid(2) as usize * 256;
    let z0 = request.key.tz.rem_euclid(2) as usize * 256;
    for z in 0..256 {
        for x in 0..256 {
            let (wx, wz) = (x0 + x, z0 + z);
            let Some(pixel) = pixel(region, wx, wz) else {
                continue;
            };
            let name = block_name(region, pixel);
            let biome = biome_name(region, pixel);
            let mut rgb = match table.block(name) {
                Some(block) => table.color(block, biome),
                None => {
                    if unknown.len() < lumilio_plugin_api::map::MAX_UNKNOWN {
                        unknown.insert(name.to_owned());
                    }
                    colors::UNKNOWN
                }
            };
            for overlay in &pixel.overlays {
                let name = overlay
                    .state
                    .and_then(|index| region.palettes.state_names.get(index as usize))
                    .map(String::as_str)
                    .unwrap_or("minecraft:water");
                let over = table
                    .block(name)
                    .map(|block| table.color(block, biome))
                    .unwrap_or(colors::UNKNOWN);
                let alpha = overlay.effective_opacity().clamp(0, 15) as u16;
                rgb = [0, 1, 2].map(|channel| {
                    ((u16::from(rgb[channel]) * (15 - alpha) + u16::from(over[channel]) * alpha)
                        / 15) as u8
                });
            }
            let north = if wz > 0 {
                self::pixel(region, wx, wz - 1).map_or(pixel.height, |other| other.height)
            } else {
                pixel.height
            };
            let rgb = shade(rgb, pixel.height, north);
            let at = z * 256 + x;
            rgba[at * 4..at * 4 + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            coverage[at] = 255;
        }
    }
    if coverage.iter().all(|byte| *byte == 0) {
        TileReply::Empty
    } else {
        TileReply::Partial {
            image: ImageData {
                width: 256,
                height: 256,
                rgba,
            },
            coverage,
            unknown: unknown.into_iter().collect(),
        }
    }
}

pub(crate) fn tile(ctx: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError> {
    if !valid_request(request) || request.key.level != 0 {
        return Err(PluginError::InvalidInput("invalid Xaero tile".into()));
    }
    let Some(dir) = region_dir(request)? else {
        return Ok(TileReply::Empty);
    };
    let path = format!(
        "{dir}/{}_{}.zip",
        request.key.tx.div_euclid(2),
        request.key.tz.div_euclid(2)
    );
    let Some(bytes) = read_zip(ctx, &path)? else {
        return Ok(TileReply::Empty);
    };
    let stream = xaero_core::read_region_container(&bytes)
        .map_err(|_| PluginError::Unavailable("map-xaero-unreadable".into()))?;
    let region = xaero_core::decode_region(&stream).map_err(|error| match error {
        xaero_core::codec::CodecError::UnsupportedVersion { .. } => {
            PluginError::Unavailable("map-xaero-unsupported".into())
        }
        _ => PluginError::Unavailable("map-xaero-unreadable".into()),
    })?;
    if region.truncated || region.trailing != 0 {
        return Err(PluginError::Unavailable("map-xaero-unreadable".into()));
    }
    Ok(render_quarter(&region, request))
}
