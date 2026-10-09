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
            let (x, z) = (at % 16, at / 16);
            let start = match &surface {
                Some(heights) => heights[at]?,
                None => i32::MAX,
            };
            let mut water_top: Option<i32> = None;
            for section in &chunk.sections {
                if section.is_empty() || section.y * 16 > start {
                    continue;
                }
                for y in (0..16).rev() {
                    let height = section.y * 16 + y as i32;
                    if height > start {
                        continue;
                    }
                    let name = section.block(x, y, z);
                    if is_air(name) {
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
        })
        .collect()
}
