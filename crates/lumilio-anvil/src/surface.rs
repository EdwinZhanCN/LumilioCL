//! The top block of every column of a chunk.
use crate::chunk::{Chunk, Heightmap, is_air};

const WATER: &str = "minecraft:water";

/// What a map shows for one column: the block seen from above, and how much
/// water lies over it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Column<'a> {
    pub block: &'a str,
    /// Height of the block's top face.
    pub y: i32,
    /// Blocks of water above it; 0 when dry.
    pub water: u32,
    /// The biome at the block, when the chunk records one.
    pub biome: Option<&'a str>,
}

/// The 256 columns of a chunk, `z * 16 + x`, or `None` for a column with
/// nothing in it. `skip` says which blocks the map looks through (plants,
/// torches, glass): the block below is what the column shows. Water is never
/// skipped; it is measured instead.
///
/// The search starts at the chunk's `WORLD_SURFACE` heightmap when it has one
/// (the game keeps it for every finished chunk), so the air above the ground
/// is not walked block by block; without it every section is searched.
pub fn columns<'a>(chunk: &'a Chunk, skip: impl Fn(&str) -> bool) -> Vec<Option<Column<'a>>> {
    let surface = chunk.heightmap(Heightmap::WorldSurface);
    (0..256)
        .map(|at| {
            let start = match &surface {
                Some(heights) => heights[at]?,
                None => i32::MAX,
            };
            column(chunk, at % 16, at / 16, start, false, &skip)
        })
        .collect()
}

/// The columns of a chunk under a solid roof, such as the Nether's bedrock
/// ceiling: from `roof` down, the roof's blocks are passed over until the
/// first air, and the column shows the first block below that. A column that
/// is solid all the way down is `None`.
pub fn columns_below<'a>(
    chunk: &'a Chunk,
    roof: i32,
    skip: impl Fn(&str) -> bool,
) -> Vec<Option<Column<'a>>> {
    (0..256)
        .map(|at| column(chunk, at % 16, at / 16, roof, true, &skip))
        .collect()
}

fn column<'a>(
    chunk: &'a Chunk,
    x: usize,
    z: usize,
    start: i32,
    roofed: bool,
    skip: &impl Fn(&str) -> bool,
) -> Option<Column<'a>> {
    let mut water_top: Option<i32> = None;
    // Under a roof nothing counts until the first air below it.
    let mut open = !roofed;
    for section in &chunk.sections {
        if section.y * 16 > start {
            continue;
        }
        if section.is_empty() {
            open = true;
            continue;
        }
        for y in (0..16).rev() {
            let height = section.y * 16 + y as i32;
            if height > start {
                continue;
            }
            let name = section.block(x, y, z);
            if is_air(name) {
                open = true;
                continue;
            }
            if !open {
                continue;
            }
            if name == WATER {
                water_top.get_or_insert(height);
                continue;
            }
            if skip(name) {
                continue;
            }
            return Some(Column {
                block: name,
                y: height,
                water: water_top.map_or(0, |top| (top - height).max(0) as u32),
                biome: chunk.biome(x, height, z),
            });
        }
    }
    // Water all the way down, or nothing at all.
    water_top.map(|top| Column {
        block: WATER,
        y: top,
        water: 0,
        biome: chunk.biome(x, top, z),
    })
}
