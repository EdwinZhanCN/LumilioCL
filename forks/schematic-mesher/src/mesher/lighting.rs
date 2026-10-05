//! Lighting system with block light and sky light BFS flood-fill.
//!
//! Computes per-block light levels and converts them to brightness multipliers
//! that get baked into vertex colors alongside ambient occlusion.

use crate::types::{BlockPosition, Direction, InputBlock};
use std::collections::{HashMap, VecDeque};

/// Configuration for the lighting system.
#[derive(Debug, Clone)]
pub struct LightingConfig {
    /// Enable block light (torches, glowstone, etc.).
    pub enable_block_light: bool,
    /// Enable sky light (sunlight from above).
    pub enable_sky_light: bool,
    /// Sky light level (0-15, default 15 for daytime).
    pub sky_light_level: u8,
    /// Ambient light floor (0.0 = pitch black in unlit areas, higher = brighter minimum).
    pub ambient_light: f32,
}

impl Default for LightingConfig {
    fn default() -> Self {
        Self {
            enable_block_light: false,
            enable_sky_light: false,
            sky_light_level: 15,
            ambient_light: 0.05,
        }
    }
}

impl LightingConfig {
    pub fn is_enabled(&self) -> bool {
        self.enable_block_light || self.enable_sky_light
    }
}

/// Get the light emission level of a block (0-15).
pub fn emission_level(block: &InputBlock) -> u8 {
    let block_id = block.block_id();

    match block_id {
        // Level 15
        "beacon" | "conduit" | "end_gateway" | "end_portal" | "fire"
        | "glowstone" | "jack_o_lantern" | "lava" | "lantern"
        | "sea_lantern" | "shroomlight" | "respawn_anchor" => 15,

        // Lit variants
        "campfire" | "redstone_lamp" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                15
            } else {
                0
            }
        }
        "furnace" | "blast_furnace" | "smoker" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                13
            } else {
                0
            }
        }

        // Level 14
        "torch" | "wall_torch" => 14,

        // Level 12
        "end_rod" | "crying_obsidian" => 12,

        // Level 11
        "nether_portal" => 11,

        // Level 10
        "soul_campfire" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                10
            } else {
                0
            }
        }
        "soul_fire" | "soul_torch" | "soul_wall_torch" | "soul_lantern" => 10,

        // Level 9
        "sculk_catalyst" => 9,

        // Level 7
        "enchanting_table" | "ender_chest" | "glow_lichen" | "sculk_sensor" => 7,

        // Level 6
        "amethyst_cluster" => 6,

        // Level 5
        "large_amethyst_bud" => 5,

        // Level 4
        "medium_amethyst_bud" => 4,

        // Level 3
        "magma_block" => 3,

        // Level 2
        "small_amethyst_bud" => 2,

        // Level 1
        "brewing_stand" | "brown_mushroom" | "sculk_shrieker" => 1,

        // Candles: 3 per candle when lit
        "candle" | "white_candle" | "orange_candle" | "magenta_candle"
        | "light_blue_candle" | "yellow_candle" | "lime_candle"
        | "pink_candle" | "gray_candle" | "light_gray_candle"
        | "cyan_candle" | "purple_candle" | "blue_candle"
        | "brown_candle" | "green_candle" | "red_candle"
        | "black_candle" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(false) {
                let count: u8 = block.properties.get("candles")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                (3 * count).min(15)
            } else {
                0
            }
        }

        // Redstone torch
        "redstone_torch" | "redstone_wall_torch" => {
            if block.properties.get("lit").map(|v| v == "true").unwrap_or(true) {
                7
            } else {
                0
            }
        }

        // Sea pickle: 6 + 3*(count-1) when in water
        "sea_pickle" => {
            let count: u8 = block.properties.get("pickles")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            if block.properties.get("waterlogged").map(|v| v == "true").unwrap_or(false) {
                (6 + 3 * (count - 1)).min(15)
            } else {
                0
            }
        }

        _ => 0,
    }
}

/// Get the light opacity of a block (how much it reduces light passing through).
/// 0 = fully transparent to light, 15 = fully blocks light.
pub fn block_opacity(block: &InputBlock) -> u8 {
    let block_id = block.block_id();

    // Air and transparent blocks
    if block.is_air() {
        return 0;
    }

    match block_id {
        // Fully transparent to light
        "glass" | "glass_pane" | "barrier" | "light" | "structure_void" => 0,
        _ if block_id.ends_with("_glass") => 0,
        _ if block_id.ends_with("_glass_pane") => 0,

        // Partial light blockers
        "water" | "ice" => 1,
        "frosted_ice" => 2,
        "cobweb" => 1,

        // Non-solid blocks that don't block light much
        "torch" | "wall_torch" | "soul_torch" | "soul_wall_torch"
        | "redstone_torch" | "redstone_wall_torch" => 0,
        "lantern" | "soul_lantern" => 0,
        "fire" | "soul_fire" => 0,
        "sign" | "wall_sign" | "hanging_sign" | "wall_hanging_sign" => 0,
        _ if block_id.ends_with("_sign") || block_id.ends_with("_wall_sign") => 0,
        _ if block_id.ends_with("_hanging_sign") || block_id.ends_with("_wall_hanging_sign") => 0,
        "flower_pot" => 0,
        _ if block_id.starts_with("potted_") => 0,
        "rail" | "powered_rail" | "detector_rail" | "activator_rail" => 0,
        "lever" | "button" => 0,
        _ if block_id.ends_with("_button") => 0,
        "pressure_plate" => 0,
        _ if block_id.ends_with("_pressure_plate") => 0,
        "tripwire" | "tripwire_hook" | "string" => 0,
        "carpet" => 0,
        _ if block_id.ends_with("_carpet") => 0,
        "snow" => 0,
        "ladder" | "vine" | "glow_lichen" => 0,
        "redstone_wire" => 0,
        "repeater" | "comparator" => 0,
        "end_rod" => 0,

        // Flowers and plants
        _ if block_id.ends_with("_sapling") => 0,
        "dandelion" | "poppy" | "blue_orchid" | "allium" | "azure_bluet"
        | "red_tulip" | "orange_tulip" | "white_tulip" | "pink_tulip"
        | "oxeye_daisy" | "cornflower" | "lily_of_the_valley" | "wither_rose"
        | "torchflower" | "pink_petals" => 0,
        "sunflower" | "lilac" | "rose_bush" | "peony" | "tall_grass"
        | "large_fern" | "pitcher_plant" => 0,
        "short_grass" | "fern" | "dead_bush" => 0,
        "brown_mushroom" | "red_mushroom" => 0,
        "sugar_cane" | "bamboo" | "kelp" | "kelp_plant" | "seagrass"
        | "tall_seagrass" => 0,

        // Leaves reduce light by 1
        _ if block_id.ends_with("_leaves") => 1,

        // Slabs block light by 0 when not full block
        _ if block_id.ends_with("_slab") => {
            match block.properties.get("type").map(|s| s.as_str()) {
                Some("double") => 15,
                _ => 0,
            }
        }

        // Default: solid blocks fully block light
        _ => 15,
    }
}

/// Computed light map for a scene.
pub struct LightMap {
    /// Block light level (0-15) per position.
    block_light: Vec<u8>,
    /// Sky light level (0-15) per position.
    sky_light: Vec<u8>,
    /// Emissive flag per position.
    emissive: Vec<bool>,
    /// Grid dimensions and offset (same convention as FaceCuller).
    grid_min: [i32; 3],
    grid_size: [usize; 3],
    /// Lighting configuration.
    config: LightingConfig,
}

impl LightMap {
    /// Compute lighting for a set of blocks.
    pub fn compute(
        blocks: &[(BlockPosition, &InputBlock)],
        config: &LightingConfig,
    ) -> Self {
        if blocks.is_empty() || !config.is_enabled() {
            return Self {
                block_light: Vec::new(),
                sky_light: Vec::new(),
                emissive: Vec::new(),
                grid_min: [0; 3],
                grid_size: [0; 3],
                config: config.clone(),
            };
        }

        // Compute bounding box with 1-block padding for light sampling at edges
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for (pos, _) in blocks {
            min[0] = min[0].min(pos.x);
            min[1] = min[1].min(pos.y);
            min[2] = min[2].min(pos.z);
            max[0] = max[0].max(pos.x);
            max[1] = max[1].max(pos.y);
            max[2] = max[2].max(pos.z);
        }
        // Pad by 1 for face light sampling (sample at neighbor positions)
        min[0] -= 1; min[1] -= 1; min[2] -= 1;
        max[0] += 1; max[1] += 1; max[2] += 1;

        let grid_size = [
            (max[0] - min[0] + 1) as usize,
            (max[1] - min[1] + 1) as usize,
            (max[2] - min[2] + 1) as usize,
        ];
        let total = grid_size[0] * grid_size[1] * grid_size[2];

        let mut block_light = vec![0u8; total];
        let mut sky_light = vec![0u8; total];
        let mut emissive = vec![false; total];

        // Build opacity map
        let mut opacity = vec![0u8; total];
        let mut block_map: HashMap<BlockPosition, &InputBlock> = HashMap::new();

        for (pos, block) in blocks {
            block_map.insert(*pos, block);
            if let Some(idx) = Self::grid_index_static(*pos, min, grid_size) {
                opacity[idx] = block_opacity(block);
            }
        }

        // === Block Light BFS ===
        if config.enable_block_light {
            let mut queue: VecDeque<(BlockPosition, u8)> = VecDeque::new();

            // Seed emitters
            for (pos, block) in blocks {
                let emission = emission_level(block);
                if emission > 0 {
                    if let Some(idx) = Self::grid_index_static(*pos, min, grid_size) {
                        block_light[idx] = emission;
                        emissive[idx] = true;
                        queue.push_back((*pos, emission));
                    }
                }
            }

            // BFS propagation
            while let Some((pos, level)) = queue.pop_front() {
                for &dir in Direction::ALL.iter() {
                    let neighbor = pos.neighbor(dir);
                    if let Some(idx) = Self::grid_index_static(neighbor, min, grid_size) {
                        let neighbor_opacity = opacity[idx].max(1);
                        let new_level = level.saturating_sub(neighbor_opacity);
                        if new_level > block_light[idx] {
                            block_light[idx] = new_level;
                            queue.push_back((neighbor, new_level));
                        }
                    }
                }
            }
        }

        // === Sky Light ===
        if config.enable_sky_light {
            let sky_level = config.sky_light_level;

            // Heightmap pass: scan down each column, set sky light at full level
            // until hitting an opaque block
            for x in min[0]..=max[0] {
                for z in min[2]..=max[2] {
                    let mut current_sky = sky_level;
                    for y in (min[1]..=max[1]).rev() {
                        let pos = BlockPosition::new(x, y, z);
                        if let Some(idx) = Self::grid_index_static(pos, min, grid_size) {
                            if current_sky > 0 {
                                sky_light[idx] = current_sky;
                            }
                            let op = opacity[idx];
                            if op >= 15 {
                                current_sky = 0; // Fully opaque, stop
                            } else if op > 0 {
                                current_sky = current_sky.saturating_sub(op);
                            }
                        }
                    }
                }
            }

            // Horizontal BFS spread for sky light
            let mut queue: VecDeque<(BlockPosition, u8)> = VecDeque::new();

            // Seed from all positions that got sky light
            for x in min[0]..=max[0] {
                for z in min[2]..=max[2] {
                    for y in min[1]..=max[1] {
                        let pos = BlockPosition::new(x, y, z);
                        if let Some(idx) = Self::grid_index_static(pos, min, grid_size) {
                            if sky_light[idx] > 0 {
                                queue.push_back((pos, sky_light[idx]));
                            }
                        }
                    }
                }
            }

            while let Some((pos, level)) = queue.pop_front() {
                for &dir in Direction::ALL.iter() {
                    let neighbor = pos.neighbor(dir);
                    if let Some(idx) = Self::grid_index_static(neighbor, min, grid_size) {
                        let neighbor_opacity = opacity[idx].max(1);
                        let new_level = level.saturating_sub(neighbor_opacity);
                        if new_level > sky_light[idx] {
                            sky_light[idx] = new_level;
                            queue.push_back((neighbor, new_level));
                        }
                    }
                }
            }
        }

        Self {
            block_light,
            sky_light,
            emissive,
            grid_min: min,
            grid_size,
            config: config.clone(),
        }
    }

    /// Convert a block position to a flat grid index.
    #[inline]
    fn grid_index_static(pos: BlockPosition, grid_min: [i32; 3], grid_size: [usize; 3]) -> Option<usize> {
        let x = pos.x - grid_min[0];
        let y = pos.y - grid_min[1];
        let z = pos.z - grid_min[2];
        if x < 0 || y < 0 || z < 0 {
            return None;
        }
        let x = x as usize;
        let y = y as usize;
        let z = z as usize;
        if x < grid_size[0] && y < grid_size[1] && z < grid_size[2] {
            Some(x + y * grid_size[0] + z * grid_size[0] * grid_size[1])
        } else {
            None
        }
    }

    /// Get light level at a position (returns 0 for out-of-bounds).
    #[inline]
    fn get_light(&self, pos: BlockPosition) -> (u8, u8) {
        if let Some(idx) = Self::grid_index_static(pos, self.grid_min, self.grid_size) {
            (self.block_light.get(idx).copied().unwrap_or(0),
             self.sky_light.get(idx).copied().unwrap_or(0))
        } else {
            (0, 0)
        }
    }

    /// Get the brightness multiplier for a face at a block position.
    /// Samples the light at the neighbor position in the face direction.
    pub fn face_brightness(&self, pos: BlockPosition, direction: Direction) -> f32 {
        let sample_pos = pos.neighbor(direction);
        let (bl, sl) = self.get_light(sample_pos);

        // Take the max of block light and sky light
        let max_light = bl.max(sl);

        brightness_from_level(max_light, self.config.ambient_light)
    }

    /// Check if a block is emissive (light source).
    pub fn is_emissive(&self, pos: BlockPosition) -> bool {
        if let Some(idx) = Self::grid_index_static(pos, self.grid_min, self.grid_size) {
            self.emissive.get(idx).copied().unwrap_or(false)
        } else {
            false
        }
    }
}

/// Convert a light level (0-15) to a brightness multiplier (0.0-1.0).
/// Uses Minecraft's brightness curve: `ratio / (4.0 - 3.0 * ratio)` with ambient floor.
pub fn brightness_from_level(level: u8, ambient: f32) -> f32 {
    let ratio = level as f32 / 15.0;
    // Minecraft's brightness curve gives a non-linear mapping
    let curve = ratio / (4.0 - 3.0 * ratio);
    // Lerp between ambient and 1.0
    ambient + (1.0 - ambient) * curve
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emission_levels() {
        assert_eq!(emission_level(&InputBlock::new("minecraft:glowstone")), 15);
        assert_eq!(emission_level(&InputBlock::new("minecraft:torch")), 14);
        assert_eq!(emission_level(&InputBlock::new("minecraft:soul_torch")), 10);
        assert_eq!(emission_level(&InputBlock::new("minecraft:magma_block")), 3);
        assert_eq!(emission_level(&InputBlock::new("minecraft:stone")), 0);
    }

    #[test]
    fn test_emission_state_dependent() {
        let lit_furnace = InputBlock::new("minecraft:furnace")
            .with_property("lit", "true");
        assert_eq!(emission_level(&lit_furnace), 13);

        let unlit_furnace = InputBlock::new("minecraft:furnace")
            .with_property("lit", "false");
        assert_eq!(emission_level(&unlit_furnace), 0);

        let lit_lamp = InputBlock::new("minecraft:redstone_lamp")
            .with_property("lit", "true");
        assert_eq!(emission_level(&lit_lamp), 15);
    }

    #[test]
    fn test_candle_emission() {
        let candle1 = InputBlock::new("minecraft:candle")
            .with_property("lit", "true")
            .with_property("candles", "1");
        assert_eq!(emission_level(&candle1), 3);

        let candle4 = InputBlock::new("minecraft:candle")
            .with_property("lit", "true")
            .with_property("candles", "4");
        assert_eq!(emission_level(&candle4), 12);

        let unlit = InputBlock::new("minecraft:candle")
            .with_property("lit", "false")
            .with_property("candles", "4");
        assert_eq!(emission_level(&unlit), 0);
    }

    #[test]
    fn test_block_opacity() {
        assert_eq!(block_opacity(&InputBlock::new("minecraft:air")), 0);
        assert_eq!(block_opacity(&InputBlock::new("minecraft:stone")), 15);
        assert_eq!(block_opacity(&InputBlock::new("minecraft:glass")), 0);
        assert_eq!(block_opacity(&InputBlock::new("minecraft:water")), 1);
        assert_eq!(block_opacity(&InputBlock::new("minecraft:torch")), 0);
    }

    #[test]
    fn test_brightness_curve() {
        // Level 0 → ambient
        let b0 = brightness_from_level(0, 0.05);
        assert!((b0 - 0.05).abs() < 0.01);

        // Level 15 → 1.0
        let b15 = brightness_from_level(15, 0.05);
        assert!((b15 - 1.0).abs() < 0.01);

        // Level 7 → somewhere in between
        let b7 = brightness_from_level(7, 0.05);
        assert!(b7 > 0.05 && b7 < 1.0);
    }

    #[test]
    fn test_lightmap_block_light_propagation() {
        let glowstone = InputBlock::new("minecraft:glowstone");
        let air1 = InputBlock::new("minecraft:air");
        let air2 = InputBlock::new("minecraft:air");
        let air3 = InputBlock::new("minecraft:air");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &glowstone),
            (BlockPosition::new(1, 0, 0), &air1),
            (BlockPosition::new(2, 0, 0), &air2),
            (BlockPosition::new(3, 0, 0), &air3),
        ];

        let config = LightingConfig {
            enable_block_light: true,
            enable_sky_light: false,
            sky_light_level: 15,
            ambient_light: 0.0,
        };

        let light_map = LightMap::compute(&blocks, &config);

        // Glowstone at (0,0,0) = 15, should propagate outward losing 1 per block
        let (bl0, _) = light_map.get_light(BlockPosition::new(0, 0, 0));
        let (bl1, _) = light_map.get_light(BlockPosition::new(1, 0, 0));
        let (bl2, _) = light_map.get_light(BlockPosition::new(2, 0, 0));
        let (bl3, _) = light_map.get_light(BlockPosition::new(3, 0, 0));

        assert_eq!(bl0, 15);
        assert!(bl1 > bl2, "Light should decrease with distance: {} > {}", bl1, bl2);
        assert!(bl2 > bl3, "Light should decrease with distance: {} > {}", bl2, bl3);
    }

    #[test]
    fn test_lightmap_sky_light() {
        // Column of air blocks — sky light should propagate down
        let air1 = InputBlock::new("minecraft:air");
        let air2 = InputBlock::new("minecraft:air");
        let air3 = InputBlock::new("minecraft:air");

        let blocks = vec![
            (BlockPosition::new(0, 2, 0), &air1),
            (BlockPosition::new(0, 1, 0), &air2),
            (BlockPosition::new(0, 0, 0), &air3),
        ];

        let config = LightingConfig {
            enable_block_light: false,
            enable_sky_light: true,
            sky_light_level: 15,
            ambient_light: 0.0,
        };

        let light_map = LightMap::compute(&blocks, &config);

        let (_, sl2) = light_map.get_light(BlockPosition::new(0, 2, 0));
        let (_, sl0) = light_map.get_light(BlockPosition::new(0, 0, 0));

        // Sky light should be high at top, still present at bottom (air column)
        assert!(sl2 > 0, "Top should have sky light");
        assert!(sl0 > 0, "Bottom of air column should have sky light");
    }

    #[test]
    fn test_lightmap_opaque_blocks_light() {
        // Stone should block direct light propagation
        // Note: light can still reach behind stone by going around it in 3D
        let torch = InputBlock::new("minecraft:torch");
        let stone = InputBlock::new("minecraft:stone");
        let air1 = InputBlock::new("minecraft:air");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &torch),
            (BlockPosition::new(1, 0, 0), &stone),
            (BlockPosition::new(2, 0, 0), &air1),
        ];

        let config = LightingConfig {
            enable_block_light: true,
            enable_sky_light: false,
            sky_light_level: 15,
            ambient_light: 0.0,
        };

        let light_map = LightMap::compute(&blocks, &config);

        let (bl_torch, _) = light_map.get_light(BlockPosition::new(0, 0, 0));
        let (bl_behind, _) = light_map.get_light(BlockPosition::new(2, 0, 0));

        assert_eq!(bl_torch, 14);
        // Light behind stone should be lower than the torch
        // (can still reach around stone in 3D but attenuated by extra distance)
        assert!(bl_behind < bl_torch, "Light behind stone ({}) should be less than torch ({})", bl_behind, bl_torch);
    }

    #[test]
    fn test_emissive_detection() {
        let glowstone = InputBlock::new("minecraft:glowstone");
        let stone = InputBlock::new("minecraft:stone");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &glowstone),
            (BlockPosition::new(1, 0, 0), &stone),
        ];

        let config = LightingConfig {
            enable_block_light: true,
            enable_sky_light: false,
            sky_light_level: 15,
            ambient_light: 0.0,
        };

        let light_map = LightMap::compute(&blocks, &config);

        assert!(light_map.is_emissive(BlockPosition::new(0, 0, 0)));
        assert!(!light_map.is_emissive(BlockPosition::new(1, 0, 0)));
    }

    #[test]
    fn test_face_brightness() {
        let glowstone = InputBlock::new("minecraft:glowstone");
        let air = InputBlock::new("minecraft:air");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &glowstone),
            (BlockPosition::new(1, 0, 0), &air),
        ];

        let config = LightingConfig {
            enable_block_light: true,
            enable_sky_light: false,
            sky_light_level: 15,
            ambient_light: 0.0,
        };

        let light_map = LightMap::compute(&blocks, &config);

        // Face facing east (toward the air block) should have brightness
        let brightness = light_map.face_brightness(BlockPosition::new(0, 0, 0), Direction::East);
        assert!(brightness > 0.0, "Face toward lit neighbor should be bright");
    }
}
