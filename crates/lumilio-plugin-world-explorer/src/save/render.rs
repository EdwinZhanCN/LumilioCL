//! One level-0 tile of the save map: 16×16 chunks of a region, each column
//! drawn as the colour of its top block, shaded by height.
//!
//! The shading follows fastanvil's `TopShadeRenderer`
//! (fastanvil/src/render.rs, MIT OR Apache-2.0, Copyright (c) 2020 Owen Gage;
//! ADR 0022): a column higher than the one north of it is drawn at full
//! brightness, a level one a little darker and a lower one darker still, which
//! reads as light falling from the north. Differences: water is blended over
//! the ground by its depth rather than drilled through block by block, and
//! the colours come from our own per-version tables (plan W13).
use super::colors::{self, SEE_THROUGH, Table};
use lumilio_anvil::{Chunk, Column, Region, Source, columns, columns_below};
use lumilio_plugin_api::map::{MAX_UNKNOWN, TILE_PIXELS, TileReply};
use lumilio_plugin_api::{ImageData, PluginError};
use std::collections::BTreeSet;

/// How a dimension is looked at from above.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum View {
    /// The highest block of each column.
    Open,
    /// The first floor below a roof at this height (the Nether's bedrock).
    Roofed(i32),
}

/// A column with nothing in it at all, such as the End's void.
const VOID: [u8; 3] = [12, 12, 18];
/// Brightness for a column lower than, level with, and higher than the one
/// north of it (out of 255).
const SHADE: [u16; 3] = [180, 220, 255];

/// Draws the 16×16 chunks of `region` starting at chunk `(x0, z0)` (0 or 16
/// each). A chunk that is missing, unfinished or unreadable stays uncovered;
/// the rest of the tile is drawn regardless. `None` when no chunk in the tile
/// exists at all.
pub(crate) fn draw<S: Source>(
    region: &Region<S>,
    (x0, z0): (usize, usize),
    table: &Table,
    view: View,
    cancelled: &dyn Fn() -> bool,
) -> Result<TileReply, PluginError> {
    let side = TILE_PIXELS as usize;
    let mut rgba = vec![0u8; side * side * 4];
    let mut coverage = vec![0u8; side * side];
    let mut unknown = BTreeSet::new();
    let mut any = false;
    // The surface height of every pixel, for the shading of the row below it.
    let mut heights: Vec<Option<i32>> = vec![None; side * side];
    for cz in 0..16 {
        for cx in 0..16 {
            if cancelled() {
                return Err(PluginError::Transient("map-cancelled".into()));
            }
            let Ok(Some(tag)) = region.chunk(x0 + cx, z0 + cz) else {
                continue;
            };
            any = true;
            let Ok(chunk) = Chunk::from_nbt(&tag) else {
                continue;
            };
            if !chunk.is_complete() {
                continue;
            }
            let see_through = |name: &str| {
                table
                    .block(name)
                    .is_some_and(|block| block.flags & SEE_THROUGH != 0)
            };
            let found = match view {
                View::Open => columns(&chunk, see_through),
                View::Roofed(roof) => columns_below(&chunk, roof, see_through),
            };
            for (at, column) in found.iter().enumerate() {
                let (px, pz) = (cx * 16 + at % 16, cz * 16 + at / 16);
                let pixel = pz * side + px;
                let color = match column {
                    Some(column) => {
                        heights[pixel] = Some(column.y + column.water as i32);
                        color_of(table, column, &mut unknown)
                    }
                    None => VOID,
                };
                rgba[pixel * 4..pixel * 4 + 4]
                    .copy_from_slice(&[color[0], color[1], color[2], 255]);
                coverage[pixel] = 255;
            }
        }
    }
    if !any {
        return Ok(TileReply::Empty);
    }
    for pixel in 0..side * side {
        let (Some(height), true) = (heights[pixel], pixel >= side) else {
            continue;
        };
        let shade = match heights[pixel - side] {
            Some(north) => SHADE[usize::from(height >= north) + usize::from(height > north)],
            None => SHADE[1],
        };
        for channel in &mut rgba[pixel * 4..pixel * 4 + 3] {
            *channel = (u16::from(*channel) * shade / 255) as u8;
        }
    }
    // The top row has no northern neighbour in this tile; it keeps the level
    // shade so the tile edge does not draw a line.
    for pixel in 0..side {
        if heights[pixel].is_some() {
            for channel in &mut rgba[pixel * 4..pixel * 4 + 3] {
                *channel = (u16::from(*channel) * SHADE[1] / 255) as u8;
            }
        }
    }
    Ok(TileReply::Partial {
        image: ImageData {
            width: TILE_PIXELS,
            height: TILE_PIXELS,
            rgba,
        },
        coverage,
        unknown: unknown.into_iter().take(MAX_UNKNOWN).collect(),
    })
}

/// A column's colour: its block in its biome, with any water over it blended
/// on top, deeper water hiding more of the ground.
fn color_of(table: &Table, column: &Column, unknown: &mut BTreeSet<String>) -> [u8; 3] {
    let ground = match table.block(column.block) {
        Some(block) => table.color(block, column.biome),
        None => {
            if unknown.len() < MAX_UNKNOWN {
                unknown.insert(column.block.to_owned());
            }
            colors::UNKNOWN
        }
    };
    if column.water == 0 {
        return ground;
    }
    let Some(water) = table.block("minecraft:water") else {
        return ground;
    };
    let water = table.color(water, column.biome);
    let cover = (0.6 + 0.04 * f64::from(column.water)).min(0.92);
    [0, 1, 2].map(|at| {
        (f64::from(water[at]) * cover + f64::from(ground[at]) * (1.0 - cover)).round() as u8
    })
}
