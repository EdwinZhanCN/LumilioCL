//! Face culling for hidden faces between adjacent blocks.
//!
//! Determines block opacity by analyzing the resolved model rather than hardcoded lists.
//! A block is considered opaque only if its model is a full cube (0-16 on all axes)
//! with all 6 faces defined.
//!
//! Transparent blocks (like glass) are handled specially - they only cull against
//! the same type of transparent block.

use crate::resolver::{ModelResolver, StateResolver};
use crate::resource_pack::ResourcePack;
use crate::types::{BlockPosition, Direction, InputBlock};
use std::collections::{HashMap, HashSet};

/// Classification of a block for culling purposes.
#[derive(Debug, Clone, PartialEq)]
enum BlockCullType {
    /// Air or non-solid blocks - don't cull anything.
    NonSolid,
    /// Fully opaque solid blocks - cull all adjacent faces.
    Opaque,
    /// Transparent blocks that only cull against same type (e.g., glass, ice).
    /// Contains the block's base name for same-type matching.
    Transparent(String),
}

/// Encoded cull type for the flat 3D array.
const CELL_EMPTY: u8 = 0;
const CELL_OPAQUE: u8 = 1;
const CELL_TRANSPARENT: u8 = 2;

/// Minimum block count before block classification is parallelized across rayon
/// threads in [`FaceCuller::new`].
const FACE_CULLER_PARALLEL_THRESHOLD: usize = 50_000;

/// Face culler for determining which faces should be hidden.
pub struct FaceCuller<'a> {
    /// Map of block positions to their cull type (kept for transparent group lookups in should_cull).
    /// FxHashMap: millions of BlockPosition inserts, SipHash was a measurable hotspot.
    block_types: rustc_hash::FxHashMap<BlockPosition, BlockCullType>,
    /// Flat 3D array for fast opacity lookups (indexed by offset from grid_min).
    grid: Vec<u8>,
    /// Minimum corner of the bounding box.
    grid_min: [i32; 3],
    /// Dimensions of the grid (size_x, size_y, size_z).
    grid_size: [usize; 3],
    /// Resource pack reference for model resolution.
    pack: &'a ResourcePack,
    /// Cache of block name -> cull type results.
    cull_cache: rustc_hash::FxHashMap<String, BlockCullType>,
}

impl<'a> FaceCuller<'a> {
    /// Create a face culler from a list of blocks, using model data to determine opacity.
    pub fn new(pack: &'a ResourcePack, blocks: &[(BlockPosition, &InputBlock)]) -> Self {
        // Compute bounding box (with 1-block padding for AO neighbor lookups)
        let (grid_min, grid_size) = if blocks.is_empty() {
            ([0i32; 3], [0usize; 3])
        } else {
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
            // Pad by 1 on each side so AO neighbor lookups stay in bounds
            min[0] -= 1;
            min[1] -= 1;
            min[2] -= 1;
            max[0] += 1;
            max[1] += 1;
            max[2] += 1;
            let size = [
                (max[0] - min[0] + 1) as usize,
                (max[1] - min[1] + 1) as usize,
                (max[2] - min[2] + 1) as usize,
            ];
            (min, size)
        };

        let mut culler = Self {
            block_types: rustc_hash::FxHashMap::default(),
            grid: vec![CELL_EMPTY; grid_size[0] * grid_size[1] * grid_size[2]],
            grid_min,
            grid_size,
            pack,
            cull_cache: rustc_hash::FxHashMap::default(),
        };

        // Classify every block to fill the flat opacity grid. Classification
        // (model resolution + cache-key hashing) is the expensive, independent
        // part, so it fans out across rayon workers (each with a thread-local
        // classify cache); only the grid write stays serial. wasm / small inputs
        // use the sequential path.
        //
        // `block_types` only needs *transparent* blocks: `should_cull` consults
        // it solely to compare transparent groups (an opaque neighbor culls via
        // the grid before the map is touched, and a missing entry is treated like
        // opaque). Inserting the millions of opaque blocks was the dominant cost
        // here, so we skip them entirely.
        #[allow(unused_mut)]
        let mut filled_parallel = false;

        #[cfg(not(target_arch = "wasm32"))]
        {
            if blocks.len() >= FACE_CULLER_PARALLEL_THRESHOLD {
                use rayon::prelude::*;
                let classified: Vec<(Option<usize>, u8, BlockPosition, BlockCullType)> = blocks
                    .par_iter()
                    .map_init(
                        || {
                            (
                                rustc_hash::FxHashMap::<String, BlockCullType>::default(),
                                0usize,
                                BlockCullType::NonSolid,
                            )
                        },
                        |state, (pos, block)| {
                            // Fast path: same palette entry as the previous block
                            // (terrain runs) → reuse its cull type, skipping the
                            // per-block string cache key. `state` = (cache, last
                            // block ptr, last cull type).
                            let block_ptr = *block as *const InputBlock as usize;
                            let cull_type = if block_ptr == state.1 {
                                state.2.clone()
                            } else {
                                let t = culler.classify_block_cached(block, &mut state.0);
                                state.1 = block_ptr;
                                state.2 = t.clone();
                                t
                            };
                            let cell = match &cull_type {
                                BlockCullType::Opaque => CELL_OPAQUE,
                                BlockCullType::Transparent(_) => CELL_TRANSPARENT,
                                BlockCullType::NonSolid => CELL_EMPTY,
                            };
                            (culler.grid_index(*pos), cell, *pos, cull_type)
                        },
                    )
                    .collect();
                for (idx, cell, pos, cull_type) in classified {
                    if let Some(idx) = idx {
                        culler.grid[idx] = cell;
                    }
                    if matches!(cull_type, BlockCullType::Transparent(_)) {
                        culler.block_types.insert(pos, cull_type);
                    }
                }
                filled_parallel = true;
            }
        }

        if !filled_parallel {
            for (pos, block) in blocks {
                let cull_type = culler.classify_block(block);
                let cell = match &cull_type {
                    BlockCullType::Opaque => CELL_OPAQUE,
                    BlockCullType::Transparent(_) => CELL_TRANSPARENT,
                    BlockCullType::NonSolid => CELL_EMPTY,
                };
                if let Some(idx) = culler.grid_index(*pos) {
                    culler.grid[idx] = cell;
                }
                if matches!(cull_type, BlockCullType::Transparent(_)) {
                    culler.block_types.insert(*pos, cull_type);
                }
            }
        }

        culler
    }

    /// Convert a block position to a flat grid index, or None if out of bounds.
    #[inline]
    fn grid_index(&self, pos: BlockPosition) -> Option<usize> {
        let x = (pos.x - self.grid_min[0]) as usize;
        let y = (pos.y - self.grid_min[1]) as usize;
        let z = (pos.z - self.grid_min[2]) as usize;
        if x < self.grid_size[0] && y < self.grid_size[1] && z < self.grid_size[2] {
            Some(x + y * self.grid_size[0] + z * self.grid_size[0] * self.grid_size[1])
        } else {
            None
        }
    }

    /// Fast grid lookup for a cell value (returns CELL_EMPTY for out-of-bounds).
    #[inline]
    fn grid_cell(&self, pos: BlockPosition) -> u8 {
        // Compute signed offsets
        let x = pos.x - self.grid_min[0];
        let y = pos.y - self.grid_min[1];
        let z = pos.z - self.grid_min[2];
        if x < 0 || y < 0 || z < 0 {
            return CELL_EMPTY;
        }
        let x = x as usize;
        let y = y as usize;
        let z = z as usize;
        if x < self.grid_size[0] && y < self.grid_size[1] && z < self.grid_size[2] {
            self.grid[x + y * self.grid_size[0] + z * self.grid_size[0] * self.grid_size[1]]
        } else {
            CELL_EMPTY
        }
    }

    /// Build a cache key from a block's name and properties.
    fn cache_key(block: &InputBlock) -> String {
        if block.properties.is_empty() {
            block.name.clone()
        } else {
            let mut props: Vec<_> = block.properties.iter().collect();
            props.sort_by_key(|(k, _)| k.as_str());
            let mut key = block.name.clone();
            key.push('|');
            for (i, (k, v)) in props.iter().enumerate() {
                if i > 0 {
                    key.push(',');
                }
                key.push_str(k);
                key.push('=');
                key.push_str(v);
            }
            key
        }
    }

    /// Classify a block for culling purposes.
    fn classify_block(&mut self, block: &InputBlock) -> BlockCullType {
        // Air is never solid
        if block.is_air() {
            return BlockCullType::NonSolid;
        }

        // Check cache first (keyed by name + properties)
        let key = Self::cache_key(block);
        if let Some(cached) = self.cull_cache.get(&key) {
            return cached.clone();
        }

        // Classify the block
        let cull_type = self.resolve_and_classify(block);
        self.cull_cache.insert(key, cull_type.clone());
        cull_type
    }

    /// Like [`classify_block`] but `&self`, using a caller-provided cache instead
    /// of the shared `self.cull_cache`. Lets the construction loop run in parallel
    /// (each rayon worker passes its own thread-local cache).
    fn classify_block_cached(
        &self,
        block: &InputBlock,
        cache: &mut rustc_hash::FxHashMap<String, BlockCullType>,
    ) -> BlockCullType {
        if block.is_air() {
            return BlockCullType::NonSolid;
        }
        let key = Self::cache_key(block);
        if let Some(cached) = cache.get(&key) {
            return cached.clone();
        }
        let cull_type = self.resolve_and_classify(block);
        cache.insert(key, cull_type.clone());
        cull_type
    }

    /// Resolve a block's model and classify it for culling.
    fn resolve_and_classify(&self, block: &InputBlock) -> BlockCullType {
        // Check if this is a transparent block type first
        if let Some(transparent_group) = self.get_transparent_group(&block.name) {
            return BlockCullType::Transparent(transparent_group);
        }

        // Try to resolve the block's variants
        let state_resolver = StateResolver::new(self.pack);
        let variants = match state_resolver.resolve(block) {
            Ok(v) => v,
            Err(_) => return BlockCullType::NonSolid, // Can't resolve = non-solid
        };

        // Check each variant's model
        let model_resolver = ModelResolver::new(self.pack);
        for variant in &variants {
            let model = match model_resolver.resolve(&variant.model_location()) {
                Ok(m) => m,
                Err(_) => return BlockCullType::NonSolid, // Can't resolve model = non-solid
            };

            // Check if this model is a full opaque cube
            if !self.is_full_opaque_cube(&model) {
                return BlockCullType::NonSolid;
            }
        }

        // All variants are full opaque cubes
        if variants.is_empty() {
            BlockCullType::NonSolid
        } else {
            BlockCullType::Opaque
        }
    }

    /// Get the transparent group for a block, if it's a transparent block.
    /// Transparent blocks only cull against the same type.
    fn get_transparent_group(&self, name: &str) -> Option<String> {
        // Extract the base block name without namespace
        let block_id = name.split(':').nth(1).unwrap_or(name);

        // Fluid blocks — transparent for face culling (not full-height cubes)
        if block_id == "water" {
            return Some("water".to_string());
        }
        if block_id == "lava" {
            return Some("lava".to_string());
        }

        // Glass blocks - all glass variants cull against each other
        if block_id == "glass" || block_id.ends_with("_glass") {
            // Group all stained glass together, but separate from regular glass
            if block_id.contains("stained") {
                return Some("stained_glass".to_string());
            }
            if block_id == "tinted_glass" {
                return Some("tinted_glass".to_string());
            }
            return Some("glass".to_string());
        }

        // Glass panes - separate from glass blocks
        if block_id == "glass_pane" || block_id.ends_with("_glass_pane") {
            if block_id.contains("stained") {
                return Some("stained_glass_pane".to_string());
            }
            return Some("glass_pane".to_string());
        }

        // Ice blocks
        if block_id == "ice" || block_id == "packed_ice" || block_id == "blue_ice" {
            return Some(block_id.to_string());
        }
        if block_id == "frosted_ice" {
            return Some("frosted_ice".to_string());
        }

        // Leaves - each type culls against itself
        if block_id.ends_with("_leaves") {
            return Some("leaves".to_string());
        }

        // Slime and honey blocks
        if block_id == "slime_block" {
            return Some("slime_block".to_string());
        }
        if block_id == "honey_block" {
            return Some("honey_block".to_string());
        }

        None
    }

    /// Check if a resolved model is a full opaque cube.
    fn is_full_opaque_cube(&self, model: &crate::resource_pack::BlockModel) -> bool {
        // Must have exactly one element
        if model.elements.len() != 1 {
            return false;
        }

        let element = &model.elements[0];

        // Element must span the full block (0-16 on each axis)
        const EPSILON: f32 = 0.001;
        if element.from[0].abs() > EPSILON
            || element.from[1].abs() > EPSILON
            || element.from[2].abs() > EPSILON
        {
            return false;
        }
        if (element.to[0] - 16.0).abs() > EPSILON
            || (element.to[1] - 16.0).abs() > EPSILON
            || (element.to[2] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // Must have all 6 faces defined
        if element.faces.len() != 6 {
            return false;
        }

        // All faces must exist
        for direction in Direction::ALL.iter() {
            if !element.faces.contains_key(direction) {
                return false;
            }
        }

        true
    }

    /// Check if a face should be culled.
    pub fn should_cull(&self, pos: BlockPosition, cullface: Direction) -> bool {
        let neighbor_pos = pos.neighbor(cullface);
        let neighbor_cell = self.grid_cell(neighbor_pos);

        // If there's no block in that direction, don't cull
        if neighbor_cell == CELL_EMPTY {
            return false;
        }

        // If neighbor is opaque, always cull
        if neighbor_cell == CELL_OPAQUE {
            return true;
        }

        // Neighbor is transparent — only cull if current block is same transparent group
        let current_type = self.block_types.get(&pos);
        let neighbor_type = self.block_types.get(&neighbor_pos);

        match (current_type, neighbor_type) {
            (Some(BlockCullType::Transparent(current_group)), Some(BlockCullType::Transparent(neighbor_group))) => {
                current_group == neighbor_group
            }
            _ => false,
        }
    }

    /// Check if a position has an opaque block (for AO calculations).
    /// Note: Both opaque and transparent blocks contribute to AO.
    #[inline]
    pub fn is_opaque_at(&self, pos: BlockPosition) -> bool {
        self.grid_cell(pos) != CELL_EMPTY
    }

    /// Check if a position has a fully opaque (non-transparent) block.
    #[inline]
    pub fn is_fully_opaque_at(&self, pos: BlockPosition) -> bool {
        self.grid_cell(pos) == CELL_OPAQUE
    }

    /// Check if a block is fully occluded (all 6 neighbors are fully opaque).
    /// A fully occluded block can be skipped entirely during meshing.
    pub fn is_fully_occluded(&self, pos: BlockPosition) -> bool {
        Direction::ALL.iter().all(|&dir| self.is_fully_opaque_at(pos.neighbor(dir)))
    }

    /// Check if a position is occupied.
    #[inline]
    pub fn is_occupied(&self, pos: BlockPosition) -> bool {
        self.grid_cell(pos) != CELL_EMPTY
    }

    /// Calculate ambient occlusion values for the 4 vertices of a face.
    /// Returns [v0_ao, v1_ao, v2_ao, v3_ao] where values are 0-3 (0=darkest, 3=brightest).
    pub fn calculate_ao(&self, pos: BlockPosition, direction: Direction) -> [u8; 4] {
        // Get the neighbor offsets for the 4 corners of this face
        let corner_neighbors = get_ao_neighbors(direction);

        let mut ao_values = [3u8; 4];
        for (i, (side1_offset, side2_offset, corner_offset)) in corner_neighbors.iter().enumerate() {
            let side1_pos = BlockPosition::new(
                pos.x + side1_offset[0],
                pos.y + side1_offset[1],
                pos.z + side1_offset[2],
            );
            let side2_pos = BlockPosition::new(
                pos.x + side2_offset[0],
                pos.y + side2_offset[1],
                pos.z + side2_offset[2],
            );
            let corner_pos = BlockPosition::new(
                pos.x + corner_offset[0],
                pos.y + corner_offset[1],
                pos.z + corner_offset[2],
            );

            let side1 = self.is_opaque_at(side1_pos) as u8;
            let side2 = self.is_opaque_at(side2_pos) as u8;
            let corner = self.is_opaque_at(corner_pos) as u8;

            ao_values[i] = vertex_ao(side1, side2, corner);
        }

        ao_values
    }
}

/// For backwards compatibility - create from blocks without pack reference.
/// This uses a simplified opacity check (occupied = opaque).
impl<'a> FaceCuller<'a> {
    /// Create a face culler without model resolution (legacy mode).
    /// Uses simple occupancy for opacity - all blocks are considered opaque.
    pub fn from_blocks(blocks: &[(BlockPosition, &InputBlock)]) -> FaceCullerSimple {
        FaceCullerSimple::from_blocks(blocks)
    }
}

/// Simplified cull type for heuristic-based culling.
#[derive(Debug, Clone, PartialEq)]
enum SimpleCullType {
    NonSolid,
    Opaque,
    Transparent(String),
}

/// Simplified face culler without model resolution.
/// Uses basic heuristics to determine opacity.
pub struct FaceCullerSimple {
    /// Map of block positions to their cull type.
    block_types: HashMap<BlockPosition, SimpleCullType>,
    /// Set of occupied positions.
    occupied: HashSet<BlockPosition>,
}

impl FaceCullerSimple {
    /// Create from blocks using heuristic opacity detection.
    pub fn from_blocks(blocks: &[(BlockPosition, &InputBlock)]) -> Self {
        let mut block_types = HashMap::new();
        let mut occupied = HashSet::new();

        for (pos, block) in blocks {
            occupied.insert(*pos);
            let cull_type = Self::classify_block_heuristic(&block.name);
            block_types.insert(*pos, cull_type);
        }

        Self {
            block_types,
            occupied,
        }
    }

    /// Classify a block using heuristics (no model resolution).
    fn classify_block_heuristic(name: &str) -> SimpleCullType {
        let block_id = name.split(':').nth(1).unwrap_or(name);

        // Air is never solid
        if name.contains("air") {
            return SimpleCullType::NonSolid;
        }

        // Check for transparent blocks
        if let Some(group) = Self::get_transparent_group_heuristic(block_id) {
            return SimpleCullType::Transparent(group);
        }

        // Check if likely a full cube
        if is_likely_full_cube(name) {
            SimpleCullType::Opaque
        } else {
            SimpleCullType::NonSolid
        }
    }

    /// Get transparent group using heuristics.
    fn get_transparent_group_heuristic(block_id: &str) -> Option<String> {
        // Fluid blocks
        if block_id == "water" {
            return Some("water".to_string());
        }
        if block_id == "lava" {
            return Some("lava".to_string());
        }

        // Glass blocks
        if block_id == "glass" || block_id.ends_with("_glass") {
            if block_id.contains("stained") {
                return Some("stained_glass".to_string());
            }
            if block_id == "tinted_glass" {
                return Some("tinted_glass".to_string());
            }
            return Some("glass".to_string());
        }

        // Glass panes
        if block_id == "glass_pane" || block_id.ends_with("_glass_pane") {
            if block_id.contains("stained") {
                return Some("stained_glass_pane".to_string());
            }
            return Some("glass_pane".to_string());
        }

        // Ice
        if block_id == "ice" || block_id == "packed_ice" || block_id == "blue_ice" || block_id == "frosted_ice" {
            return Some(block_id.to_string());
        }

        // Leaves
        if block_id.ends_with("_leaves") {
            return Some("leaves".to_string());
        }

        // Slime and honey
        if block_id == "slime_block" || block_id == "honey_block" {
            return Some(block_id.to_string());
        }

        None
    }

    /// Check if a face should be culled.
    pub fn should_cull(&self, pos: BlockPosition, cullface: Direction) -> bool {
        let neighbor_pos = pos.neighbor(cullface);

        if !self.occupied.contains(&neighbor_pos) {
            return false;
        }

        let neighbor_type = self.block_types.get(&neighbor_pos);
        let current_type = self.block_types.get(&pos);

        match (current_type, neighbor_type) {
            (_, Some(SimpleCullType::Opaque)) => true,
            (Some(SimpleCullType::Transparent(current)), Some(SimpleCullType::Transparent(neighbor))) => {
                current == neighbor
            }
            _ => false,
        }
    }

    /// Check if a position has an opaque block (for AO).
    pub fn is_opaque_at(&self, pos: BlockPosition) -> bool {
        match self.block_types.get(&pos) {
            Some(SimpleCullType::Opaque) => true,
            Some(SimpleCullType::Transparent(_)) => true,
            _ => false,
        }
    }

    /// Check if a position has a fully opaque (non-transparent) block.
    pub fn is_fully_opaque_at(&self, pos: BlockPosition) -> bool {
        matches!(self.block_types.get(&pos), Some(SimpleCullType::Opaque))
    }

    /// Check if a block is fully occluded (all 6 neighbors are fully opaque).
    pub fn is_fully_occluded(&self, pos: BlockPosition) -> bool {
        Direction::ALL.iter().all(|&dir| self.is_fully_opaque_at(pos.neighbor(dir)))
    }

    /// Check if a position is occupied.
    pub fn is_occupied(&self, pos: BlockPosition) -> bool {
        self.occupied.contains(&pos)
    }

    /// Calculate ambient occlusion values for the 4 vertices of a face.
    pub fn calculate_ao(&self, pos: BlockPosition, direction: Direction) -> [u8; 4] {
        let corner_neighbors = get_ao_neighbors(direction);

        let mut ao_values = [3u8; 4];
        for (i, (side1_offset, side2_offset, corner_offset)) in corner_neighbors.iter().enumerate() {
            let side1_pos = BlockPosition::new(
                pos.x + side1_offset[0],
                pos.y + side1_offset[1],
                pos.z + side1_offset[2],
            );
            let side2_pos = BlockPosition::new(
                pos.x + side2_offset[0],
                pos.y + side2_offset[1],
                pos.z + side2_offset[2],
            );
            let corner_pos = BlockPosition::new(
                pos.x + corner_offset[0],
                pos.y + corner_offset[1],
                pos.z + corner_offset[2],
            );

            let side1 = self.is_opaque_at(side1_pos) as u8;
            let side2 = self.is_opaque_at(side2_pos) as u8;
            let corner = self.is_opaque_at(corner_pos) as u8;

            ao_values[i] = vertex_ao(side1, side2, corner);
        }

        ao_values
    }
}

/// Heuristic check if a block name likely represents a full cube.
/// This is a fallback when we can't resolve the model.
fn is_likely_full_cube(name: &str) -> bool {
    // Air is never a full cube
    if name.contains("air") {
        return false;
    }

    // Fluids are never full cubes (variable height)
    let block_id = name.split(':').nth(1).unwrap_or(name);
    if block_id == "water" || block_id == "lava" {
        return false;
    }

    // Entity namespace blocks (mobs) are never full cubes
    if name.starts_with("entity:") {
        return false;
    }

    // Common patterns for non-full blocks
    let non_full_patterns = [
        "slab", "stairs", "fence", "wall", "door", "trapdoor",
        "sign", "banner", "button", "lever", "torch", "lantern",
        "pressure_plate", "carpet", "rail", "flower", "sapling",
        "glass_pane", "iron_bars", "chain", "rod", "candle",
        "head", "skull", "pot", "campfire", "anvil", "bell", "shulker",
        "brewing_stand", "cauldron", "hopper", "lectern",
        "grindstone", "stonecutter", "enchanting_table",
        "repeater", "comparator", "daylight_detector",
        "piston", "tripwire", "string", "cobweb", "vine",
        "ladder", "scaffolding", "coral_fan", "pickle",
        "egg", "frogspawn", "dripleaf", "azalea", "roots",
        "sprouts", "fungus", "mushroom", "grass", "fern",
        "bush", "berry", "wart", "stem", "crop", "wheat",
        "carrots", "potatoes", "beetroots", "cocoa", "cactus",
        "sugar_cane", "bamboo", "kelp", "seagrass", "lichen",
        "vein", "fire", "snow", "layer",
        // Block entities with non-full geometry
        "_bed", "chest", "armor_stand", "minecart", "item_frame",
        // Specific flowers that don't have "flower" in name
        "poppy", "dandelion", "orchid", "allium", "tulip",
        "oxeye_daisy", "cornflower", "lily_of_the_valley",
        "wither_rose", "sunflower", "lilac", "rose_bush",
        "peony", "pitcher_plant", "torchflower", "pink_petals",
    ];

    for pattern in &non_full_patterns {
        if name.contains(pattern) {
            // Special case: some blocks contain these patterns but ARE full cubes
            // e.g., "mushroom_block", "grass_block", "hay_block"
            if name.ends_with("_block") && !name.contains("piston") {
                continue;
            }
            return false;
        }
    }

    // Default: assume it's a full cube
    true
}

/// Calculate vertex AO value from neighbor occupancy.
pub fn vertex_ao(side1: u8, side2: u8, corner: u8) -> u8 {
    if side1 == 1 && side2 == 1 {
        0
    } else {
        3 - (side1 + side2 + corner)
    }
}

/// Get the neighbor offsets for AO calculation for each vertex of a face.
pub fn get_ao_neighbors(direction: Direction) -> [([i32; 3], [i32; 3], [i32; 3]); 4] {
    match direction {
        Direction::Up => [
            ([0, 1, -1], [-1, 1, 0], [-1, 1, -1]),
            ([0, 1, -1], [1, 1, 0], [1, 1, -1]),
            ([0, 1, 1], [1, 1, 0], [1, 1, 1]),
            ([0, 1, 1], [-1, 1, 0], [-1, 1, 1]),
        ],
        Direction::Down => [
            ([0, -1, 1], [-1, -1, 0], [-1, -1, 1]),
            ([0, -1, 1], [1, -1, 0], [1, -1, 1]),
            ([0, -1, -1], [1, -1, 0], [1, -1, -1]),
            ([0, -1, -1], [-1, -1, 0], [-1, -1, -1]),
        ],
        Direction::North => [
            ([1, 0, -1], [0, 1, -1], [1, 1, -1]),
            ([-1, 0, -1], [0, 1, -1], [-1, 1, -1]),
            ([-1, 0, -1], [0, -1, -1], [-1, -1, -1]),
            ([1, 0, -1], [0, -1, -1], [1, -1, -1]),
        ],
        Direction::South => [
            ([-1, 0, 1], [0, 1, 1], [-1, 1, 1]),
            ([1, 0, 1], [0, 1, 1], [1, 1, 1]),
            ([1, 0, 1], [0, -1, 1], [1, -1, 1]),
            ([-1, 0, 1], [0, -1, 1], [-1, -1, 1]),
        ],
        Direction::West => [
            ([-1, 0, -1], [-1, 1, 0], [-1, 1, -1]),
            ([-1, 0, 1], [-1, 1, 0], [-1, 1, 1]),
            ([-1, 0, 1], [-1, -1, 0], [-1, -1, 1]),
            ([-1, 0, -1], [-1, -1, 0], [-1, -1, -1]),
        ],
        Direction::East => [
            ([1, 0, 1], [1, 1, 0], [1, 1, 1]),
            ([1, 0, -1], [1, 1, 0], [1, 1, -1]),
            ([1, 0, -1], [1, -1, 0], [1, -1, -1]),
            ([1, 0, 1], [1, -1, 0], [1, -1, 1]),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_likely_full_cube() {
        // Full cubes
        assert!(is_likely_full_cube("minecraft:stone"));
        assert!(is_likely_full_cube("minecraft:dirt"));
        assert!(is_likely_full_cube("minecraft:oak_log"));
        assert!(is_likely_full_cube("minecraft:diamond_block"));
        assert!(is_likely_full_cube("minecraft:grass_block"));
        assert!(is_likely_full_cube("minecraft:mushroom_block"));

        // Non-full blocks
        assert!(!is_likely_full_cube("minecraft:air"));
        assert!(!is_likely_full_cube("minecraft:oak_slab"));
        assert!(!is_likely_full_cube("minecraft:oak_stairs"));
        assert!(!is_likely_full_cube("minecraft:oak_fence"));
        assert!(!is_likely_full_cube("minecraft:glass_pane"));
        assert!(!is_likely_full_cube("minecraft:torch"));
        assert!(!is_likely_full_cube("minecraft:poppy"));
        assert!(!is_likely_full_cube("minecraft:oak_door"));
    }

    #[test]
    fn test_simple_culler() {
        let stone1 = InputBlock::new("minecraft:stone");
        let stone2 = InputBlock::new("minecraft:stone");
        let flower = InputBlock::new("minecraft:poppy");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &stone1),
            (BlockPosition::new(1, 0, 0), &stone2),
            (BlockPosition::new(0, 1, 0), &flower),
        ];

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Face between two stones should be culled
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::East));
        assert!(culler.should_cull(BlockPosition::new(1, 0, 0), Direction::West));

        // Flower on top of stone should NOT cull stone's top face
        assert!(!culler.should_cull(BlockPosition::new(0, 0, 0), Direction::Up));

        // Outer faces should not be culled
        assert!(!culler.should_cull(BlockPosition::new(0, 0, 0), Direction::West));
    }

    #[test]
    fn test_glass_culling() {
        let glass1 = InputBlock::new("minecraft:glass");
        let glass2 = InputBlock::new("minecraft:glass");
        let stained_glass = InputBlock::new("minecraft:red_stained_glass");
        let stone = InputBlock::new("minecraft:stone");

        let blocks = vec![
            (BlockPosition::new(0, 0, 0), &glass1),
            (BlockPosition::new(1, 0, 0), &glass2),
            (BlockPosition::new(2, 0, 0), &stained_glass),
            (BlockPosition::new(0, 1, 0), &stone),
        ];

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Glass should cull against same glass type
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::East));
        assert!(culler.should_cull(BlockPosition::new(1, 0, 0), Direction::West));

        // Glass should NOT cull against stained glass (different group)
        assert!(!culler.should_cull(BlockPosition::new(1, 0, 0), Direction::East));
        assert!(!culler.should_cull(BlockPosition::new(2, 0, 0), Direction::West));

        // Glass should cull when neighbor is opaque stone
        assert!(culler.should_cull(BlockPosition::new(0, 0, 0), Direction::Up));
    }

    #[test]
    fn test_transparent_groups() {
        // Test that transparent group detection works correctly
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("glass"),
            Some("glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("red_stained_glass"),
            Some("stained_glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("tinted_glass"),
            Some("tinted_glass".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("oak_leaves"),
            Some("leaves".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("ice"),
            Some("ice".to_string())
        );
        assert_eq!(
            FaceCullerSimple::get_transparent_group_heuristic("stone"),
            None
        );
    }

    #[test]
    fn test_is_fully_occluded() {
        // Create a 3x3x3 cube of stone — the center block should be fully occluded
        let stones: Vec<InputBlock> = (0..27).map(|_| InputBlock::new("minecraft:stone")).collect();
        let mut blocks = Vec::new();
        let mut i = 0;
        for x in 0..3 {
            for y in 0..3 {
                for z in 0..3 {
                    blocks.push((BlockPosition::new(x, y, z), &stones[i]));
                    i += 1;
                }
            }
        }

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Center block (1,1,1) has opaque neighbors on all 6 sides
        assert!(culler.is_fully_occluded(BlockPosition::new(1, 1, 1)));

        // Corner block (0,0,0) has 3 open faces
        assert!(!culler.is_fully_occluded(BlockPosition::new(0, 0, 0)));

        // Edge block (1,0,0) has bottom face exposed
        assert!(!culler.is_fully_occluded(BlockPosition::new(1, 0, 0)));

        // Face block (1,1,0) has north face exposed
        assert!(!culler.is_fully_occluded(BlockPosition::new(1, 1, 0)));
    }

    #[test]
    fn test_is_fully_occluded_with_transparent() {
        // Glass does not occlude — a block surrounded by glass is NOT occluded
        let stone = InputBlock::new("minecraft:stone");
        let glass: Vec<InputBlock> = (0..6).map(|_| InputBlock::new("minecraft:glass")).collect();

        let blocks = vec![
            (BlockPosition::new(1, 1, 1), &stone),
            (BlockPosition::new(0, 1, 1), &glass[0]),
            (BlockPosition::new(2, 1, 1), &glass[1]),
            (BlockPosition::new(1, 0, 1), &glass[2]),
            (BlockPosition::new(1, 2, 1), &glass[3]),
            (BlockPosition::new(1, 1, 0), &glass[4]),
            (BlockPosition::new(1, 1, 2), &glass[5]),
        ];

        let culler = FaceCullerSimple::from_blocks(&blocks);

        // Glass is transparent, not fully opaque — so center is NOT occluded
        assert!(!culler.is_fully_occluded(BlockPosition::new(1, 1, 1)));
    }
}
