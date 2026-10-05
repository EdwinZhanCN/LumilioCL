//! Shared types used throughout the library.

mod direction;
mod transform;

pub use direction::{Direction, Axis};
pub use transform::{BlockTransform, ElementRotation};

use std::collections::HashMap;

/// A block position in 3D space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl BlockPosition {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Get the neighboring position in the given direction.
    pub fn neighbor(&self, direction: Direction) -> Self {
        let (dx, dy, dz) = direction.offset();
        Self {
            x: self.x + dx,
            y: self.y + dy,
            z: self.z + dz,
        }
    }
}

/// An axis-aligned bounding box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl BoundingBox {
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: impl Iterator<Item = [f32; 3]>) -> Option<Self> {
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        let mut has_points = false;

        for p in points {
            has_points = true;
            for i in 0..3 {
                min[i] = min[i].min(p[i]);
                max[i] = max[i].max(p[i]);
            }
        }

        if has_points {
            Some(Self { min, max })
        } else {
            None
        }
    }

    pub fn dimensions(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }
}

/// Input block for meshing, compatible with Nucleation's BlockState.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputBlock {
    /// Block name, e.g., "minecraft:stone"
    pub name: String,
    /// Block properties, e.g., {"facing": "north"}
    pub properties: HashMap<String, String>,
}

impl InputBlock {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: HashMap::new(),
        }
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Get the namespace (e.g., "minecraft").
    pub fn namespace(&self) -> &str {
        self.name.split(':').next().unwrap_or("minecraft")
    }

    /// Get the block ID without namespace (e.g., "stone").
    pub fn block_id(&self) -> &str {
        self.name.split(':').nth(1).unwrap_or(&self.name)
    }

    /// Check if this is an air block.
    pub fn is_air(&self) -> bool {
        matches!(
            self.name.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "air"
        )
    }
}

/// Trait for block data sources.
///
/// Implement this to connect your block storage (e.g., Nucleation, a custom
/// schematic loader, or an in-memory `HashMap`) to the mesher. Used by
/// [`Mesher::mesh()`](crate::Mesher::mesh) and
/// [`Mesher::mesh_chunks()`](crate::Mesher::mesh_chunks).
///
/// The mesher reads blocks via [`iter_blocks()`](BlockSource::iter_blocks) for
/// single-shot meshing and [`blocks_in_region()`](BlockSource::blocks_in_region)
/// for chunk iteration. Override `blocks_in_region` if your storage can
/// efficiently query spatial subsets (e.g., by chunk file).
pub trait BlockSource {
    /// Get the block at a specific position, or `None` if empty/air.
    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock>;

    /// Iterate over all non-air blocks in the source.
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_>;

    /// Get the axis-aligned bounding box enclosing all blocks.
    fn bounds(&self) -> BoundingBox;

    /// Iterate over blocks within a spatial region.
    ///
    /// The default implementation filters [`iter_blocks()`](BlockSource::iter_blocks),
    /// which scans every block. Override this for efficient spatial queries (e.g.,
    /// only reading the relevant chunk from disk).
    fn blocks_in_region(
        &self,
        bounds: BoundingBox,
    ) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.iter_blocks().filter(move |(pos, _)| {
            let x = pos.x as f32;
            let y = pos.y as f32;
            let z = pos.z as f32;
            x >= bounds.min[0]
                && x < bounds.max[0]
                && y >= bounds.min[1]
                && y < bounds.max[1]
                && z >= bounds.min[2]
                && z < bounds.max[2]
        }))
    }
}
