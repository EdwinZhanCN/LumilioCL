//! Greedy meshing algorithm for merging adjacent coplanar faces.
//!
//! Merges adjacent faces with the same texture and tint into larger quads,
//! dramatically reducing triangle count for large flat surfaces.

use crate::mesher::face_culler::{get_ao_neighbors, vertex_ao, FaceCuller};
use crate::types::{BlockPosition, Direction};
use std::collections::HashMap;

/// Key for determining if two faces can be merged.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FaceMergeKey {
    /// Resolved texture path.
    pub texture: String,
    /// Quantized vertex color (RGBA as u8).
    pub tint: [u8; 4],
    /// Per-vertex ambient occlusion values (0=darkest, 3=brightest).
    /// Faces with different AO patterns won't merge, preserving AO detail.
    pub ao: [u8; 4],
    /// Quantized light level (0-15). Faces with different light levels won't merge.
    pub light: u8,
}

/// A face recorded for greedy merging.
#[derive(Debug, Clone)]
pub struct GreedyFace {
    /// Merge key for this face.
    pub key: FaceMergeKey,
    /// Whether this face's texture is transparent.
    pub is_transparent: bool,
}

/// A merged quad produced by the greedy algorithm.
#[derive(Debug, Clone)]
pub struct MergedQuad {
    /// Face direction.
    pub direction: Direction,
    /// Layer coordinate (the fixed axis value).
    pub layer: i32,
    /// Start of the U range (inclusive).
    pub u_min: i32,
    /// Start of the V range (inclusive).
    pub v_min: i32,
    /// Width along U axis.
    pub width: i32,
    /// Height along V axis.
    pub height: i32,
    /// Texture path.
    pub texture: String,
    /// Vertex color tint (quantized).
    pub tint: [u8; 4],
    /// Per-vertex ambient occlusion values.
    pub ao: [u8; 4],
    /// Whether this quad's texture is transparent.
    pub is_transparent: bool,
}

impl MergedQuad {
    /// Compute the 4 world-space vertex positions for this merged quad.
    /// Winding order matches `generate_face_vertices` in element.rs.
    /// Uses centered convention: block at (x,y,z) occupies [x-0.5, x+0.5].
    pub fn world_positions(&self) -> [[f32; 3]; 4] {
        let (u_min, v_min) = (self.u_min as f32 - 0.5, self.v_min as f32 - 0.5);
        let (u_max, v_max) = (
            (self.u_min + self.width) as f32 - 0.5,
            (self.v_min + self.height) as f32 - 0.5,
        );
        let layer = self.layer as f32 - 0.5;

        // For positive-facing directions, the face is at layer + 1 boundary
        // For negative-facing directions, the face is at layer boundary
        //
        // Coordinate mapping (pos_to_layer_coords):
        //   Up/Down:     layer=y, u=x, v=z
        //   North/South: layer=z, u=x, v=y
        //   East/West:   layer=x, u=z, v=y
        //
        // Winding order matches element.rs generate_face_vertices
        // for a full cube with from=[-0.5,-0.5,-0.5] to=[0.5,0.5,0.5],
        // but scaled to block coordinates (each block = 1 unit).
        match self.direction {
            Direction::Up => {
                // Face at y = layer + 1
                let y = layer + 1.0;
                [
                    [u_min, y, v_min], // from.x, to.y, from.z
                    [u_max, y, v_min], // to.x, to.y, from.z
                    [u_max, y, v_max], // to.x, to.y, to.z
                    [u_min, y, v_max], // from.x, to.y, to.z
                ]
            }
            Direction::Down => {
                // Face at y = layer
                let y = layer as f32;
                [
                    [u_min, y, v_max], // from.x, from.y, to.z
                    [u_max, y, v_max], // to.x, from.y, to.z
                    [u_max, y, v_min], // to.x, from.y, from.z
                    [u_min, y, v_min], // from.x, from.y, from.z
                ]
            }
            Direction::North => {
                // Face at z = layer
                let z = layer as f32;
                [
                    [u_max, v_max, z], // to.x, to.y, from.z
                    [u_min, v_max, z], // from.x, to.y, from.z
                    [u_min, v_min, z], // from.x, from.y, from.z
                    [u_max, v_min, z], // to.x, from.y, from.z
                ]
            }
            Direction::South => {
                // Face at z = layer + 1
                let z = layer + 1.0;
                [
                    [u_min, v_max, z], // from.x, to.y, to.z
                    [u_max, v_max, z], // to.x, to.y, to.z
                    [u_max, v_min, z], // to.x, from.y, to.z
                    [u_min, v_min, z], // from.x, from.y, to.z
                ]
            }
            Direction::West => {
                // Face at x = layer
                let x = layer as f32;
                [
                    [x, v_max, u_min], // from.x, to.y, from.z
                    [x, v_max, u_max], // from.x, to.y, to.z
                    [x, v_min, u_max], // from.x, from.y, to.z
                    [x, v_min, u_min], // from.x, from.y, from.z
                ]
            }
            Direction::East => {
                // Face at x = layer + 1
                let x = layer + 1.0;
                [
                    [x, v_max, u_max], // to.x, to.y, to.z
                    [x, v_max, u_min], // to.x, to.y, from.z
                    [x, v_min, u_min], // to.x, from.y, from.z
                    [x, v_min, u_max], // to.x, from.y, to.z
                ]
            }
        }
    }

    /// Compute AO values for the 4 corners of this merged quad.
    /// Uses the block positions at each corner to sample AO neighbors.
    pub fn calculate_ao(&self, culler: &FaceCuller) -> [u8; 4] {
        // For each vertex corner, find the block position that would generate
        // that vertex in the non-greedy path, and compute AO there.
        let corner_blocks = self.corner_block_positions();
        let mut ao_values = [3u8; 4];

        for (i, corner_pos) in corner_blocks.iter().enumerate() {
            let ao_neighbors = get_ao_neighbors(self.direction);
            let (side1_offset, side2_offset, corner_offset) = &ao_neighbors[i];

            let side1_pos = BlockPosition::new(
                corner_pos.x + side1_offset[0],
                corner_pos.y + side1_offset[1],
                corner_pos.z + side1_offset[2],
            );
            let side2_pos = BlockPosition::new(
                corner_pos.x + side2_offset[0],
                corner_pos.y + side2_offset[1],
                corner_pos.z + side2_offset[2],
            );
            let corner_ao_pos = BlockPosition::new(
                corner_pos.x + corner_offset[0],
                corner_pos.y + corner_offset[1],
                corner_pos.z + corner_offset[2],
            );

            let side1 = culler.is_opaque_at(side1_pos) as u8;
            let side2 = culler.is_opaque_at(side2_pos) as u8;
            let corner = culler.is_opaque_at(corner_ao_pos) as u8;

            ao_values[i] = vertex_ao(side1, side2, corner);
        }

        ao_values
    }

    /// Get the block positions that correspond to each of the 4 vertex corners.
    /// These are the blocks whose AO sampling positions are used for each vertex.
    fn corner_block_positions(&self) -> [BlockPosition; 4] {
        // For each vertex of the merged quad, we need the block that "owns" that corner.
        // The vertex indices match the winding order in world_positions().
        //
        // For a merged quad spanning u_min..u_min+width, v_min..v_min+height:
        // The corner blocks are at the extremes of the range.
        //
        // Coordinate mapping: layer=fixed axis, u/v=variable axes
        let (u_min, v_min) = (self.u_min, self.v_min);
        let (u_max, v_max) = (self.u_min + self.width - 1, self.v_min + self.height - 1);
        let layer = self.layer;

        // Map (layer, u, v) back to (x, y, z) per direction
        match self.direction {
            Direction::Up => [
                // v0: (u_min, layer, v_min)
                BlockPosition::new(u_min, layer, v_min),
                // v1: (u_max, layer, v_min)
                BlockPosition::new(u_max, layer, v_min),
                // v2: (u_max, layer, v_max)
                BlockPosition::new(u_max, layer, v_max),
                // v3: (u_min, layer, v_max)
                BlockPosition::new(u_min, layer, v_max),
            ],
            Direction::Down => [
                // v0: (u_min, layer, v_max)
                BlockPosition::new(u_min, layer, v_max),
                // v1: (u_max, layer, v_max)
                BlockPosition::new(u_max, layer, v_max),
                // v2: (u_max, layer, v_min)
                BlockPosition::new(u_max, layer, v_min),
                // v3: (u_min, layer, v_min)
                BlockPosition::new(u_min, layer, v_min),
            ],
            Direction::North => [
                // v0: (u_max, v_max, layer)
                BlockPosition::new(u_max, v_max, layer),
                // v1: (u_min, v_max, layer)
                BlockPosition::new(u_min, v_max, layer),
                // v2: (u_min, v_min, layer)
                BlockPosition::new(u_min, v_min, layer),
                // v3: (u_max, v_min, layer)
                BlockPosition::new(u_max, v_min, layer),
            ],
            Direction::South => [
                // v0: (u_min, v_max, layer)
                BlockPosition::new(u_min, v_max, layer),
                // v1: (u_max, v_max, layer)
                BlockPosition::new(u_max, v_max, layer),
                // v2: (u_max, v_min, layer)
                BlockPosition::new(u_max, v_min, layer),
                // v3: (u_min, v_min, layer)
                BlockPosition::new(u_min, v_min, layer),
            ],
            Direction::West => [
                // v0: (layer, v_max, u_min)
                BlockPosition::new(layer, v_max, u_min),
                // v1: (layer, v_max, u_max)
                BlockPosition::new(layer, v_max, u_max),
                // v2: (layer, v_min, u_max)
                BlockPosition::new(layer, v_min, u_max),
                // v3: (layer, v_min, u_min)
                BlockPosition::new(layer, v_min, u_min),
            ],
            Direction::East => [
                // v0: (layer, v_max, u_max)
                BlockPosition::new(layer, v_max, u_max),
                // v1: (layer, v_max, u_min)
                BlockPosition::new(layer, v_max, u_min),
                // v2: (layer, v_min, u_min)
                BlockPosition::new(layer, v_min, u_min),
                // v3: (layer, v_min, u_max)
                BlockPosition::new(layer, v_min, u_max),
            ],
        }
    }
}

/// Convert a block position and face direction to (layer, u, v) coordinates.
pub fn pos_to_layer_coords(pos: BlockPosition, direction: Direction) -> (i32, i32, i32) {
    match direction {
        Direction::Up | Direction::Down => (pos.y, pos.x, pos.z),
        Direction::North | Direction::South => (pos.z, pos.x, pos.y),
        Direction::East | Direction::West => (pos.x, pos.z, pos.y),
    }
}

/// Quantize an f32 color to u8 for hashing.
pub fn quantize_color(color: [f32; 4]) -> [u8; 4] {
    [
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        (color[3] * 255.0).round() as u8,
    ]
}

/// The greedy mesher collects eligible faces and merges them.
pub struct GreedyMesher {
    /// Faces indexed by direction -> layer -> (u, v) -> face data.
    layers: HashMap<Direction, HashMap<i32, HashMap<(i32, i32), GreedyFace>>>,
}

impl GreedyMesher {
    pub fn new() -> Self {
        Self {
            layers: HashMap::new(),
        }
    }

    /// Add a face to be considered for greedy merging.
    pub fn add_face(
        &mut self,
        pos: BlockPosition,
        direction: Direction,
        key: FaceMergeKey,
        is_transparent: bool,
    ) {
        let (layer, u, v) = pos_to_layer_coords(pos, direction);
        self.layers
            .entry(direction)
            .or_default()
            .entry(layer)
            .or_default()
            .insert((u, v), GreedyFace { key, is_transparent });
    }

    /// Run the greedy merge algorithm and return merged quads.
    pub fn merge(self) -> Vec<MergedQuad> {
        let mut result = Vec::new();

        for (direction, layers) in &self.layers {
            for (&layer, grid) in layers {
                let quads = merge_layer(*direction, layer, grid);
                result.extend(quads);
            }
        }

        result
    }
}

/// Merge a single 2D layer of faces using greedy rectangle expansion.
fn merge_layer(
    direction: Direction,
    layer: i32,
    grid: &HashMap<(i32, i32), GreedyFace>,
) -> Vec<MergedQuad> {
    if grid.is_empty() {
        return Vec::new();
    }

    // Find bounds
    let mut u_min = i32::MAX;
    let mut u_max = i32::MIN;
    let mut v_min = i32::MAX;
    let mut v_max = i32::MIN;
    for &(u, v) in grid.keys() {
        u_min = u_min.min(u);
        u_max = u_max.max(u);
        v_min = v_min.min(v);
        v_max = v_max.max(v);
    }

    let grid_width = (u_max - u_min + 1) as usize;
    let grid_height = (v_max - v_min + 1) as usize;
    let mut visited = vec![false; grid_width * grid_height];
    let mut result = Vec::new();

    // Inline helper to index into visited array
    let idx = |u: i32, v: i32| -> usize {
        (u - u_min) as usize + (v - v_min) as usize * grid_width
    };

    // Scan in v-major order (top to bottom, left to right)
    for v in v_min..=v_max {
        for u in u_min..=u_max {
            if visited[idx(u, v)] {
                continue;
            }

            let face = match grid.get(&(u, v)) {
                Some(f) => f,
                None => continue,
            };

            let key = &face.key;
            let is_transparent = face.is_transparent;

            // Expand right (along u)
            let mut width = 1;
            while u + width <= u_max {
                if visited[idx(u + width, v)] {
                    break;
                }
                match grid.get(&(u + width, v)) {
                    Some(f) if f.key == *key => width += 1,
                    _ => break,
                }
            }

            // Expand down (along v)
            let mut height = 1;
            'outer: while v + height <= v_max {
                // Check entire row
                for du in 0..width {
                    if visited[idx(u + du, v + height)] {
                        break 'outer;
                    }
                    match grid.get(&(u + du, v + height)) {
                        Some(f) if f.key == *key => {}
                        _ => break 'outer,
                    }
                }
                height += 1;
            }

            // Mark visited
            for dv in 0..height {
                for du in 0..width {
                    visited[idx(u + du, v + dv)] = true;
                }
            }

            result.push(MergedQuad {
                direction,
                layer,
                u_min: u,
                v_min: v,
                width,
                height,
                texture: key.texture.clone(),
                tint: key.tint,
                ao: key.ao,
                is_transparent,
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stone_key() -> FaceMergeKey {
        FaceMergeKey {
            texture: "block/stone".to_string(),
            tint: [255, 255, 255, 255],
            ao: [3, 3, 3, 3],
            light: 15,
        }
    }

    fn dirt_key() -> FaceMergeKey {
        FaceMergeKey {
            texture: "block/dirt".to_string(),
            tint: [255, 255, 255, 255],
            ao: [3, 3, 3, 3],
            light: 15,
        }
    }

    #[test]
    fn test_pos_to_layer_coords() {
        let pos = BlockPosition::new(3, 5, 7);

        assert_eq!(pos_to_layer_coords(pos, Direction::Up), (5, 3, 7));
        assert_eq!(pos_to_layer_coords(pos, Direction::Down), (5, 3, 7));
        assert_eq!(pos_to_layer_coords(pos, Direction::North), (7, 3, 5));
        assert_eq!(pos_to_layer_coords(pos, Direction::South), (7, 3, 5));
        assert_eq!(pos_to_layer_coords(pos, Direction::East), (3, 7, 5));
        assert_eq!(pos_to_layer_coords(pos, Direction::West), (3, 7, 5));
    }

    #[test]
    fn test_quantize_color() {
        assert_eq!(quantize_color([1.0, 0.5, 0.0, 1.0]), [255, 128, 0, 255]);
        assert_eq!(quantize_color([0.0, 0.0, 0.0, 0.0]), [0, 0, 0, 0]);
    }

    #[test]
    fn test_single_face_no_merge() {
        let mut mesher = GreedyMesher::new();
        mesher.add_face(BlockPosition::new(0, 0, 0), Direction::Up, stone_key(), false);

        let quads = mesher.merge();
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].width, 1);
        assert_eq!(quads[0].height, 1);
    }

    #[test]
    fn test_merge_row() {
        let mut mesher = GreedyMesher::new();
        for x in 0..4 {
            mesher.add_face(BlockPosition::new(x, 0, 0), Direction::Up, stone_key(), false);
        }

        let quads = mesher.merge();
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].width, 4);
        assert_eq!(quads[0].height, 1);
    }

    #[test]
    fn test_merge_rectangle() {
        let mut mesher = GreedyMesher::new();
        for x in 0..3 {
            for z in 0..2 {
                mesher.add_face(BlockPosition::new(x, 0, z), Direction::Up, stone_key(), false);
            }
        }

        let quads = mesher.merge();
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].width, 3);
        assert_eq!(quads[0].height, 2);
    }

    #[test]
    fn test_no_merge_different_textures() {
        let mut mesher = GreedyMesher::new();
        mesher.add_face(BlockPosition::new(0, 0, 0), Direction::Up, stone_key(), false);
        mesher.add_face(BlockPosition::new(1, 0, 0), Direction::Up, dirt_key(), false);

        let quads = mesher.merge();
        assert_eq!(quads.len(), 2);
        assert!(quads.iter().all(|q| q.width == 1 && q.height == 1));
    }

    #[test]
    fn test_merge_separate_layers() {
        let mut mesher = GreedyMesher::new();
        // Two rows on different Y layers
        for x in 0..3 {
            mesher.add_face(BlockPosition::new(x, 0, 0), Direction::Up, stone_key(), false);
            mesher.add_face(BlockPosition::new(x, 1, 0), Direction::Up, stone_key(), false);
        }

        let quads = mesher.merge();
        // Should be 2 quads (one per layer), not merged across layers
        assert_eq!(quads.len(), 2);
        assert!(quads.iter().all(|q| q.width == 3 && q.height == 1));
    }

    #[test]
    fn test_world_positions_up() {
        let quad = MergedQuad {
            direction: Direction::Up,
            layer: 2,
            u_min: 1,
            v_min: 3,
            width: 4,
            height: 2,
            texture: "block/stone".to_string(),
            tint: [255, 255, 255, 255],
            ao: [3, 3, 3, 3],
            is_transparent: false,
        };

        let positions = quad.world_positions();
        // Face at y=2.5 (layer+0.5 centered), spanning x=0.5..4.5, z=2.5..4.5
        assert_eq!(positions[0], [0.5, 2.5, 2.5]); // u_min, y, v_min
        assert_eq!(positions[1], [4.5, 2.5, 2.5]); // u_max, y, v_min
        assert_eq!(positions[2], [4.5, 2.5, 4.5]); // u_max, y, v_max
        assert_eq!(positions[3], [0.5, 2.5, 4.5]); // u_min, y, v_max
    }

    #[test]
    fn test_world_positions_north() {
        let quad = MergedQuad {
            direction: Direction::North,
            layer: 0,
            u_min: 0,
            v_min: 0,
            width: 3,
            height: 2,
            texture: "block/stone".to_string(),
            tint: [255, 255, 255, 255],
            ao: [3, 3, 3, 3],
            is_transparent: false,
        };

        let positions = quad.world_positions();
        // Face at z=-0.5 (layer-0.5 centered), spanning x=-0.5..2.5, y=-0.5..1.5
        assert_eq!(positions[0], [2.5, 1.5, -0.5]); // u_max, v_max, z
        assert_eq!(positions[1], [-0.5, 1.5, -0.5]); // u_min, v_max, z
        assert_eq!(positions[2], [-0.5, -0.5, -0.5]); // u_min, v_min, z
        assert_eq!(positions[3], [2.5, -0.5, -0.5]); // u_max, v_min, z
    }

    #[test]
    fn test_l_shaped_merge() {
        // L-shape: 3 blocks in a row + 1 below first
        // xxx
        // x
        let mut mesher = GreedyMesher::new();
        mesher.add_face(BlockPosition::new(0, 0, 0), Direction::Up, stone_key(), false);
        mesher.add_face(BlockPosition::new(1, 0, 0), Direction::Up, stone_key(), false);
        mesher.add_face(BlockPosition::new(2, 0, 0), Direction::Up, stone_key(), false);
        mesher.add_face(BlockPosition::new(0, 0, 1), Direction::Up, stone_key(), false);

        let quads = mesher.merge();
        // Greedy should produce 2 quads: top row (3x1) + bottom-left (1x1)
        assert_eq!(quads.len(), 2);
        let total_area: i32 = quads.iter().map(|q| q.width * q.height).sum();
        assert_eq!(total_area, 4);
    }

    #[test]
    fn test_cube_top_face() {
        // 4x4 top face (simulating top of a 4x4x4 cube)
        let mut mesher = GreedyMesher::new();
        for x in 0..4 {
            for z in 0..4 {
                mesher.add_face(BlockPosition::new(x, 3, z), Direction::Up, stone_key(), false);
            }
        }

        let quads = mesher.merge();
        assert_eq!(quads.len(), 1);
        assert_eq!(quads[0].width, 4);
        assert_eq!(quads[0].height, 4);
    }
}
