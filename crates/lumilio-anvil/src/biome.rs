//! Biomes of a chunk. From 1.18 each section has its own palette of biome
//! names over a 4×4×4 grid; before that the chunk held numeric ids, 256 of
//! them (one per column, before 1.15) or 1024 (4×4×4 cells, 1.15–1.17).
use crate::Error;
use lumilio_nbt::Tag;

/// A section's biomes: a palette and one index per 4×4×4 cell, or `None`
/// when the whole section is `palette[0]`.
#[derive(Clone, Debug)]
pub(crate) struct SectionBiomes {
    pub(crate) palette: Vec<String>,
    pub(crate) indices: Option<Vec<u16>>,
}

impl SectionBiomes {
    pub(crate) fn from_nbt(biomes: &Tag) -> Result<Option<Self>, Error> {
        let Some(Tag::List(palette)) = biomes.get("palette") else {
            return Ok(None);
        };
        let palette: Vec<String> = palette
            .iter()
            .filter_map(|entry| crate::chunk::palette_name(entry).map(str::to_owned))
            .collect();
        if palette.is_empty() {
            return Ok(None);
        }
        let indices = match biomes.get("data") {
            Some(Tag::LongArray(longs)) if palette.len() > 1 => {
                Some(crate::chunk::unpack(longs, palette.len(), 64, 1, false)?)
            }
            _ => None,
        };
        Ok(Some(Self { palette, indices }))
    }

    /// The biome of local block `x`, `y`, `z` (each 0..16).
    pub(crate) fn at(&self, x: usize, y: usize, z: usize) -> &str {
        let index = self.indices.as_ref().map_or(0, |indices| {
            indices.get(cell(x, y, z)).copied().map_or(0, usize::from)
        });
        &self.palette[index.min(self.palette.len() - 1)]
    }
}

/// The 4×4×4 cell holding block `x`, `y`, `z`, with `y` counted in cells
/// from wherever the grid starts.
fn cell(x: usize, y: usize, z: usize) -> usize {
    ((y >> 2) << 4) | ((z >> 2) << 2) | (x >> 2)
}

/// Biome ids before 1.18, as numbers: 256 per column or 1024 per cell.
pub(crate) fn legacy(ids: &[i32], x: usize, y: i32, z: usize) -> Option<&'static str> {
    let id = match ids.len() {
        256 => ids[z * 16 + x],
        // 64 layers of cells from y = 0.
        1024 => ids[cell(x, y.clamp(0, 255) as usize, z)],
        _ => return None,
    };
    legacy_name(id)
}

/// The vanilla biome names of 1.13–1.17 by numeric id (the ids were stable
/// across those versions; 1.18 renamed several, and the names here are the
/// ones those versions used).
pub fn legacy_name(id: i32) -> Option<&'static str> {
    Some(match id {
        0 => "minecraft:ocean",
        1 => "minecraft:plains",
        2 => "minecraft:desert",
        3 => "minecraft:mountains",
        4 => "minecraft:forest",
        5 => "minecraft:taiga",
        6 => "minecraft:swamp",
        7 => "minecraft:river",
        8 => "minecraft:nether_wastes",
        9 => "minecraft:the_end",
        10 => "minecraft:frozen_ocean",
        11 => "minecraft:frozen_river",
        12 => "minecraft:snowy_tundra",
        13 => "minecraft:snowy_mountains",
        14 => "minecraft:mushroom_fields",
        15 => "minecraft:mushroom_field_shore",
        16 => "minecraft:beach",
        17 => "minecraft:desert_hills",
        18 => "minecraft:wooded_hills",
        19 => "minecraft:taiga_hills",
        20 => "minecraft:mountain_edge",
        21 => "minecraft:jungle",
        22 => "minecraft:jungle_hills",
        23 => "minecraft:jungle_edge",
        24 => "minecraft:deep_ocean",
        25 => "minecraft:stone_shore",
        26 => "minecraft:snowy_beach",
        27 => "minecraft:birch_forest",
        28 => "minecraft:birch_forest_hills",
        29 => "minecraft:dark_forest",
        30 => "minecraft:snowy_taiga",
        31 => "minecraft:snowy_taiga_hills",
        32 => "minecraft:giant_tree_taiga",
        33 => "minecraft:giant_tree_taiga_hills",
        34 => "minecraft:wooded_mountains",
        35 => "minecraft:savanna",
        36 => "minecraft:savanna_plateau",
        37 => "minecraft:badlands",
        38 => "minecraft:wooded_badlands_plateau",
        39 => "minecraft:badlands_plateau",
        40 => "minecraft:small_end_islands",
        41 => "minecraft:end_midlands",
        42 => "minecraft:end_highlands",
        43 => "minecraft:end_barrens",
        44 => "minecraft:warm_ocean",
        45 => "minecraft:lukewarm_ocean",
        46 => "minecraft:cold_ocean",
        47 => "minecraft:deep_warm_ocean",
        48 => "minecraft:deep_lukewarm_ocean",
        49 => "minecraft:deep_cold_ocean",
        50 => "minecraft:deep_frozen_ocean",
        127 => "minecraft:the_void",
        129 => "minecraft:sunflower_plains",
        130 => "minecraft:desert_lakes",
        131 => "minecraft:gravelly_mountains",
        132 => "minecraft:flower_forest",
        133 => "minecraft:taiga_mountains",
        134 => "minecraft:swamp_hills",
        140 => "minecraft:ice_spikes",
        149 => "minecraft:modified_jungle",
        151 => "minecraft:modified_jungle_edge",
        155 => "minecraft:tall_birch_forest",
        156 => "minecraft:tall_birch_hills",
        157 => "minecraft:dark_forest_hills",
        158 => "minecraft:snowy_taiga_mountains",
        160 => "minecraft:giant_spruce_taiga",
        161 => "minecraft:giant_spruce_taiga_hills",
        162 => "minecraft:modified_gravelly_mountains",
        163 => "minecraft:shattered_savanna",
        164 => "minecraft:shattered_savanna_plateau",
        165 => "minecraft:eroded_badlands",
        166 => "minecraft:modified_wooded_badlands_plateau",
        167 => "minecraft:modified_badlands_plateau",
        168 => "minecraft:bamboo_jungle",
        169 => "minecraft:bamboo_jungle_hills",
        170 => "minecraft:soul_sand_valley",
        171 => "minecraft:crimson_forest",
        172 => "minecraft:warped_forest",
        173 => "minecraft:basalt_deltas",
        174 => "minecraft:dripstone_caves",
        175 => "minecraft:lush_caves",
        _ => return None,
    })
}
