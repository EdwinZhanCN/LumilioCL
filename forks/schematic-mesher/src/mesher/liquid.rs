//! Liquid block geometry generation for water and lava.
//!
//! Generates custom mesh geometry for fluid blocks, bypassing the normal
//! model resolution pipeline since water/lava have no JSON block models.

use crate::mesher::geometry::Vertex;
use crate::types::{BlockPosition, Direction, InputBlock};
use std::collections::HashMap;

/// Type of fluid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidType {
    Water,
    Lava,
}

/// Parsed fluid state from a block's properties.
#[derive(Debug, Clone, Copy)]
pub struct FluidState {
    pub fluid_type: FluidType,
    /// Fluid amount 1-8 (8 = source/full).
    pub amount: u8,
    /// Whether this is a source block.
    pub is_source: bool,
    /// Whether fluid is falling (level >= 8).
    pub is_falling: bool,
}

impl FluidState {
    /// Parse a block into a fluid state, if it's a fluid block.
    pub fn from_block(block: &InputBlock) -> Option<Self> {
        let block_id = block.block_id();
        let fluid_type = match block_id {
            "water" => FluidType::Water,
            "lava" => FluidType::Lava,
            _ => return None,
        };

        let level: u8 = block.properties
            .get("level")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Level 0 = source, 1-7 = flowing (7 = thinnest), 8+ = falling
        let is_falling = level >= 8;
        let (is_source, amount) = if level == 0 {
            (true, 8)
        } else if level < 8 {
            (false, 8 - level) // level 1 → amount 7, level 7 → amount 1
        } else {
            // Falling: level 8-15, treated as full height column
            (false, 8)
        };

        Some(FluidState { fluid_type, is_source, amount, is_falling })
    }

    /// Height of this fluid block (0.0 to ~0.89).
    pub fn own_height(&self) -> f32 {
        if self.is_falling {
            1.0
        } else {
            self.amount as f32 / 9.0
        }
    }

    /// Texture path for the still variant.
    pub fn still_texture(&self) -> &'static str {
        match self.fluid_type {
            FluidType::Water => "block/water_still",
            FluidType::Lava => "block/lava_still",
        }
    }

    /// Texture path for the flowing variant.
    pub fn flow_texture(&self) -> &'static str {
        match self.fluid_type {
            FluidType::Water => "block/water_flow",
            FluidType::Lava => "block/lava_flow",
        }
    }
}

/// Check if a block has the `waterlogged=true` property.
pub fn is_waterlogged(block: &InputBlock) -> bool {
    block.properties.get("waterlogged").map(|v| v == "true").unwrap_or(false)
}

/// Get the fluid state at a neighboring position.
fn get_neighbor_fluid(
    pos: BlockPosition,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
) -> Option<FluidState> {
    block_map.get(&pos).and_then(|b| FluidState::from_block(b))
}

/// Check if a neighbor is the same fluid type.
fn is_same_fluid(
    pos: BlockPosition,
    fluid_type: FluidType,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
) -> bool {
    if get_neighbor_fluid(pos, block_map)
        .map(|f| f.fluid_type == fluid_type)
        .unwrap_or(false)
    {
        return true;
    }
    // Waterlogged blocks count as water sources for neighbor matching
    if fluid_type == FluidType::Water {
        if let Some(b) = block_map.get(&pos) {
            return is_waterlogged(b);
        }
    }
    false
}

/// Compute the height at one corner of a fluid block, averaging with neighbors.
/// Returns a height in [0, 1].
fn corner_height(
    pos: BlockPosition,
    state: &FluidState,
    dx: i32,
    dz: i32,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
) -> f32 {
    // If there's the same fluid directly above, height is always 1.0
    let above = BlockPosition::new(pos.x, pos.y + 1, pos.z);
    if is_same_fluid(above, state.fluid_type, block_map) {
        return 1.0;
    }

    // Sample this block + up to 3 neighbors sharing this corner
    let offsets = [(0, 0), (dx, 0), (0, dz), (dx, dz)];
    let mut total_height = 0.0f32;
    let mut total_weight = 0.0f32;

    for (ox, oz) in &offsets {
        let np = BlockPosition::new(pos.x + ox, pos.y, pos.z + oz);

        // Check if same fluid above this neighbor → height 1.0 with high weight
        let np_above = BlockPosition::new(np.x, np.y + 1, np.z);
        if is_same_fluid(np_above, state.fluid_type, block_map) {
            return 1.0;
        }

        if let Some(nf) = get_neighbor_fluid(np, block_map) {
            if nf.fluid_type == state.fluid_type {
                let h = nf.own_height();
                // Source blocks get extra weight to smooth corners
                let w = if h >= 0.8 { 10.0 } else { 1.0 };
                total_height += h * w;
                total_weight += w;
            }
        }
    }

    if total_weight > 0.0 {
        total_height / total_weight
    } else {
        state.own_height()
    }
}

/// Compute the 4 corner heights [NW, NE, SE, SW] for a fluid block.
/// NW = (-x, -z), NE = (+x, -z), SE = (+x, +z), SW = (-x, +z)
pub fn corner_heights(
    pos: BlockPosition,
    state: &FluidState,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
) -> [f32; 4] {
    [
        corner_height(pos, state, -1, -1, block_map), // NW corner
        corner_height(pos, state,  1, -1, block_map), // NE corner
        corner_height(pos, state,  1,  1, block_map), // SE corner
        corner_height(pos, state, -1,  1, block_map), // SW corner
    ]
}

/// Direction-based shading factor (matches Minecraft's fixed face lighting).
pub fn direction_shade(direction: Direction) -> f32 {
    match direction {
        Direction::Up => 1.0,
        Direction::Down => 0.5,
        Direction::North | Direction::South => 0.8,
        Direction::East | Direction::West => 0.6,
    }
}

/// Determine which faces should be rendered for a fluid block.
pub fn visible_faces(
    pos: BlockPosition,
    state: &FluidState,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
    is_opaque_fn: impl Fn(BlockPosition) -> bool,
) -> [bool; 6] {
    let mut faces = [true; 6]; // [down, up, north, south, west, east]

    // Down: hide if same fluid below or opaque below
    let below = pos.neighbor(Direction::Down);
    if is_same_fluid(below, state.fluid_type, block_map) || is_opaque_fn(below) {
        faces[0] = false;
    }

    // Up: hide if same fluid above
    let above = pos.neighbor(Direction::Up);
    if is_same_fluid(above, state.fluid_type, block_map) {
        faces[1] = false;
    }

    // Sides: hide if same fluid or opaque neighbor
    let side_dirs = [Direction::North, Direction::South, Direction::West, Direction::East];
    for (i, &dir) in side_dirs.iter().enumerate() {
        let neighbor = pos.neighbor(dir);
        if is_same_fluid(neighbor, state.fluid_type, block_map) || is_opaque_fn(neighbor) {
            faces[2 + i] = false;
        }
    }

    faces
}

/// Generate fluid geometry (vertices + indices) for a single fluid block.
///
/// Returns (vertices, indices, texture paths used).
/// The texture paths are: (still_texture, flow_texture).
pub fn generate_fluid_geometry(
    pos: BlockPosition,
    state: &FluidState,
    block_map: &rustc_hash::FxHashMap<BlockPosition, &InputBlock>,
    is_opaque_fn: impl Fn(BlockPosition) -> bool,
    base_color: [f32; 4],
) -> (Vec<Vertex>, Vec<u32>, Vec<FaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    let faces = visible_faces(pos, state, block_map, &is_opaque_fn);
    let heights = corner_heights(pos, state, block_map);

    // Block positions are block-centered: a block at pos occupies [pos-0.5, pos+0.5].
    // This matches regular block geometry which uses normalized_from/to in [-0.5, 0.5] range.
    let x = pos.x as f32 - 0.5;
    let y = pos.y as f32 - 0.5;
    let z = pos.z as f32 - 0.5;

    // Small epsilon to prevent z-fighting
    let eps = 0.001;

    // heights: [NW, NE, SE, SW]
    // NW = (x, z), NE = (x+1, z), SE = (x+1, z+1), SW = (x, z+1)
    let h_nw = heights[0];
    let h_ne = heights[1];
    let h_se = heights[2];
    let h_sw = heights[3];

    // === Top face ===
    if faces[1] {
        let shade = direction_shade(Direction::Up);
        let color = [base_color[0] * shade, base_color[1] * shade, base_color[2] * shade, base_color[3]];
        let normal = [0.0, 1.0, 0.0];
        let tex = state.still_texture();

        let v_start = vertices.len() as u32;
        // Winding order: NW, NE, SE, SW — CCW triangles 0,3,2 + 0,2,1 for upward normal
        vertices.push(Vertex::new([x, y + h_nw, z], normal, [0.0, 0.0]).with_color(color));
        vertices.push(Vertex::new([x + 1.0, y + h_ne, z], normal, [1.0, 0.0]).with_color(color));
        vertices.push(Vertex::new([x + 1.0, y + h_se, z + 1.0], normal, [1.0, 1.0]).with_color(color));
        vertices.push(Vertex::new([x, y + h_sw, z + 1.0], normal, [0.0, 1.0]).with_color(color));

        indices.extend_from_slice(&[v_start, v_start + 3, v_start + 2, v_start, v_start + 2, v_start + 1]);
        face_textures.push(FaceTexture { texture: tex, is_transparent: state.fluid_type == FluidType::Water });
    }

    // === Bottom face ===
    if faces[0] {
        let shade = direction_shade(Direction::Down);
        let color = [base_color[0] * shade, base_color[1] * shade, base_color[2] * shade, base_color[3]];
        let normal = [0.0, -1.0, 0.0];
        let tex = state.still_texture();

        let v_start = vertices.len() as u32;
        // Winding order for Down face: SW, SE, NE, NW — CCW triangles 0,3,2 + 0,2,1 for downward normal
        vertices.push(Vertex::new([x, y + eps, z + 1.0], normal, [0.0, 1.0]).with_color(color));
        vertices.push(Vertex::new([x + 1.0, y + eps, z + 1.0], normal, [1.0, 1.0]).with_color(color));
        vertices.push(Vertex::new([x + 1.0, y + eps, z], normal, [1.0, 0.0]).with_color(color));
        vertices.push(Vertex::new([x, y + eps, z], normal, [0.0, 0.0]).with_color(color));

        indices.extend_from_slice(&[v_start, v_start + 3, v_start + 2, v_start, v_start + 2, v_start + 1]);
        face_textures.push(FaceTexture { texture: tex, is_transparent: state.fluid_type == FluidType::Water });
    }

    // === Side faces ===
    // North face (z- side)
    if faces[2] {
        emit_side_face(
            &mut vertices, &mut indices, &mut face_textures,
            state, base_color, Direction::North,
            [x + 1.0, y, z + eps], [x, y, z + eps],
            h_ne, h_nw,
        );
    }

    // South face (z+ side)
    if faces[3] {
        emit_side_face(
            &mut vertices, &mut indices, &mut face_textures,
            state, base_color, Direction::South,
            [x, y, z + 1.0 - eps], [x + 1.0, y, z + 1.0 - eps],
            h_sw, h_se,
        );
    }

    // West face (x- side)
    if faces[4] {
        emit_side_face(
            &mut vertices, &mut indices, &mut face_textures,
            state, base_color, Direction::West,
            [x + eps, y, z], [x + eps, y, z + 1.0],
            h_nw, h_sw,
        );
    }

    // East face (x+ side)
    if faces[5] {
        emit_side_face(
            &mut vertices, &mut indices, &mut face_textures,
            state, base_color, Direction::East,
            [x + 1.0 - eps, y, z + 1.0], [x + 1.0 - eps, y, z],
            h_se, h_ne,
        );
    }

    (vertices, indices, face_textures)
}

/// Texture assignment for a generated face.
pub struct FaceTexture {
    pub texture: &'static str,
    pub is_transparent: bool,
}

/// Emit a side face quad (double-sided for correct viewing from both sides).
fn emit_side_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<FaceTexture>,
    state: &FluidState,
    base_color: [f32; 4],
    direction: Direction,
    // Bottom-left and bottom-right corners (at y=pos.y)
    bl: [f32; 3],
    br: [f32; 3],
    // Heights at left and right edges
    h_left: f32,
    h_right: f32,
) {
    let shade = direction_shade(direction);
    let color = [base_color[0] * shade, base_color[1] * shade, base_color[2] * shade, base_color[3]];
    let normal = direction.normal();
    let tex = state.flow_texture();
    let is_transparent = state.fluid_type == FluidType::Water;

    let y_base = bl[1]; // Both bl and br share the same y

    // Front face (outward-facing normal) — CCW winding 0,3,2 + 0,2,1
    let v_start = vertices.len() as u32;
    let v_bl = [bl[0], y_base, bl[2]];
    let v_br = [br[0], y_base, br[2]];
    let v_tr = [br[0], y_base + h_right, br[2]];
    let v_tl = [bl[0], y_base + h_left, bl[2]];

    // UV: map vertical extent to fluid height
    vertices.push(Vertex::new(v_tl, normal, [0.0, 1.0 - h_left]).with_color(color));
    vertices.push(Vertex::new(v_tr, normal, [1.0, 1.0 - h_right]).with_color(color));
    vertices.push(Vertex::new(v_br, normal, [1.0, 1.0]).with_color(color));
    vertices.push(Vertex::new(v_bl, normal, [0.0, 1.0]).with_color(color));

    indices.extend_from_slice(&[v_start, v_start + 3, v_start + 2, v_start, v_start + 2, v_start + 1]);
    face_textures.push(FaceTexture { texture: tex, is_transparent });

    // Back face (inward-facing for double-sided rendering) — CCW winding 0,3,2 + 0,2,1
    let neg_normal = [-normal[0], -normal[1], -normal[2]];
    let v_start2 = vertices.len() as u32;

    vertices.push(Vertex::new(v_tr, neg_normal, [1.0, 1.0 - h_right]).with_color(color));
    vertices.push(Vertex::new(v_tl, neg_normal, [0.0, 1.0 - h_left]).with_color(color));
    vertices.push(Vertex::new(v_bl, neg_normal, [0.0, 1.0]).with_color(color));
    vertices.push(Vertex::new(v_br, neg_normal, [1.0, 1.0]).with_color(color));

    indices.extend_from_slice(&[v_start2, v_start2 + 3, v_start2 + 2, v_start2, v_start2 + 2, v_start2 + 1]);
    face_textures.push(FaceTexture { texture: tex, is_transparent });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water_source() -> InputBlock {
        InputBlock::new("minecraft:water").with_property("level", "0")
    }

    fn water_flowing(level: u8) -> InputBlock {
        InputBlock::new("minecraft:water").with_property("level", &level.to_string())
    }

    fn lava_source() -> InputBlock {
        InputBlock::new("minecraft:lava").with_property("level", "0")
    }

    #[test]
    fn test_fluid_state_source() {
        let block = water_source();
        let state = FluidState::from_block(&block).unwrap();
        assert_eq!(state.fluid_type, FluidType::Water);
        assert!(state.is_source);
        assert_eq!(state.amount, 8);
        assert!(!state.is_falling);
    }

    #[test]
    fn test_fluid_state_flowing() {
        let block = water_flowing(3);
        let state = FluidState::from_block(&block).unwrap();
        assert!(!state.is_source);
        assert_eq!(state.amount, 5); // 8 - 3
        assert!(!state.is_falling);
    }

    #[test]
    fn test_fluid_state_falling() {
        let block = water_flowing(8);
        let state = FluidState::from_block(&block).unwrap();
        assert!(state.is_falling);
        assert_eq!(state.amount, 8);
    }

    #[test]
    fn test_fluid_state_lava() {
        let block = lava_source();
        let state = FluidState::from_block(&block).unwrap();
        assert_eq!(state.fluid_type, FluidType::Lava);
        assert!(state.is_source);
    }

    #[test]
    fn test_fluid_state_not_fluid() {
        let block = InputBlock::new("minecraft:stone");
        assert!(FluidState::from_block(&block).is_none());
    }

    #[test]
    fn test_own_height() {
        let source = FluidState { fluid_type: FluidType::Water, amount: 8, is_source: true, is_falling: false };
        assert!((source.own_height() - 8.0 / 9.0).abs() < 0.01);

        let flowing = FluidState { fluid_type: FluidType::Water, amount: 4, is_source: false, is_falling: false };
        assert!((flowing.own_height() - 4.0 / 9.0).abs() < 0.01);

        let falling = FluidState { fluid_type: FluidType::Water, amount: 8, is_source: false, is_falling: true };
        assert!((falling.own_height() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_corner_heights_isolated_source() {
        let block = water_source();
        let pos = BlockPosition::new(0, 0, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &block);

        let state = FluidState::from_block(&block).unwrap();
        let heights = corner_heights(pos, &state, &map);

        // All corners should be approximately the same (source height)
        for h in &heights {
            assert!(*h > 0.5, "Corner height {} should be > 0.5 for source", h);
        }
    }

    #[test]
    fn test_corner_heights_fluid_above_forces_1() {
        let block = water_source();
        let above = water_source();
        let pos = BlockPosition::new(0, 0, 0);
        let above_pos = BlockPosition::new(0, 1, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &block);
        map.insert(above_pos, &above);

        let state = FluidState::from_block(&block).unwrap();
        let heights = corner_heights(pos, &state, &map);

        for h in &heights {
            assert!((*h - 1.0).abs() < 0.01, "Height should be 1.0 with fluid above, got {}", h);
        }
    }

    #[test]
    fn test_visible_faces_isolated() {
        let block = water_source();
        let pos = BlockPosition::new(0, 0, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &block);

        let state = FluidState::from_block(&block).unwrap();
        let faces = visible_faces(pos, &state, &map, |_| false);

        // All faces visible for isolated fluid
        assert!(faces.iter().all(|&f| f));
    }

    #[test]
    fn test_visible_faces_same_fluid_above_hides_top() {
        let block = water_source();
        let above = water_source();
        let pos = BlockPosition::new(0, 0, 0);
        let above_pos = BlockPosition::new(0, 1, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &block);
        map.insert(above_pos, &above);

        let state = FluidState::from_block(&block).unwrap();
        let faces = visible_faces(pos, &state, &map, |_| false);

        assert!(faces[0]); // bottom visible
        assert!(!faces[1]); // top hidden (same fluid above)
    }

    #[test]
    fn test_generate_geometry_produces_vertices() {
        let block = water_source();
        let pos = BlockPosition::new(0, 0, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &block);

        let state = FluidState::from_block(&block).unwrap();
        let color = [0.247, 0.463, 0.894, 0.8];

        let (verts, idxs, faces) = generate_fluid_geometry(pos, &state, &map, |_| false, color);

        assert!(!verts.is_empty());
        assert!(!idxs.is_empty());
        assert!(!faces.is_empty());
        // Each face has 6 indices (2 triangles), sides are double-sided (12 indices each)
        assert_eq!(idxs.len() % 6, 0);
    }

    #[test]
    fn test_texture_paths() {
        let water = FluidState { fluid_type: FluidType::Water, amount: 8, is_source: true, is_falling: false };
        assert_eq!(water.still_texture(), "block/water_still");
        assert_eq!(water.flow_texture(), "block/water_flow");

        let lava = FluidState { fluid_type: FluidType::Lava, amount: 8, is_source: true, is_falling: false };
        assert_eq!(lava.still_texture(), "block/lava_still");
        assert_eq!(lava.flow_texture(), "block/lava_flow");
    }

    #[test]
    fn test_is_waterlogged_true() {
        let block = InputBlock::new("minecraft:oak_stairs")
            .with_property("waterlogged", "true");
        assert!(is_waterlogged(&block));
    }

    #[test]
    fn test_is_waterlogged_false() {
        let block = InputBlock::new("minecraft:oak_stairs")
            .with_property("waterlogged", "false");
        assert!(!is_waterlogged(&block));

        let block = InputBlock::new("minecraft:oak_stairs");
        assert!(!is_waterlogged(&block));
    }

    #[test]
    fn test_waterlogged_neighbor_is_same_fluid() {
        let waterlogged = InputBlock::new("minecraft:oak_slab")
            .with_property("waterlogged", "true");
        let water = water_source();
        let pos_water = BlockPosition::new(0, 0, 0);
        let pos_wl = BlockPosition::new(1, 0, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos_water, &water);
        map.insert(pos_wl, &waterlogged);

        // Water at pos_water should see waterlogged neighbor as same fluid
        assert!(is_same_fluid(pos_wl, FluidType::Water, &map));
    }

    #[test]
    fn test_non_waterlogged_not_same_fluid() {
        let slab = InputBlock::new("minecraft:oak_slab");
        let pos = BlockPosition::new(0, 0, 0);
        let mut map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> = rustc_hash::FxHashMap::default();
        map.insert(pos, &slab);

        assert!(!is_same_fluid(pos, FluidType::Water, &map));
    }
}
