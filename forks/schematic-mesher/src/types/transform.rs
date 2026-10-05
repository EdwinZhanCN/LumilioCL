//! Transform types for block and element rotations.

use super::Axis;
use serde::{Deserialize, Serialize};

/// Block-level transform from blockstate variant.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlockTransform {
    /// X rotation in degrees (0, 90, 180, 270).
    pub x: i32,
    /// Y rotation in degrees (0, 90, 180, 270).
    pub y: i32,
    /// If true, UV coordinates don't rotate with the block.
    pub uvlock: bool,
}

impl BlockTransform {
    pub fn new(x: i32, y: i32, uvlock: bool) -> Self {
        Self { x, y, uvlock }
    }

    /// Check if this is an identity transform (no rotation).
    pub fn is_identity(&self) -> bool {
        self.x == 0 && self.y == 0
    }
}

/// Element-level rotation from model element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRotation {
    /// Origin point for rotation (in 0-16 Minecraft coordinates).
    #[serde(default = "default_origin")]
    pub origin: [f32; 3],
    /// Axis to rotate around.
    pub axis: Axis,
    /// Rotation angle in degrees (-45 to 45, in 22.5 increments).
    pub angle: f32,
    /// Whether to rescale the element after rotation.
    #[serde(default)]
    pub rescale: bool,
}

fn default_origin() -> [f32; 3] {
    [8.0, 8.0, 8.0]
}

impl ElementRotation {
    /// Convert origin from Minecraft coordinates (0-16) to normalized (-0.5 to 0.5).
    pub fn normalized_origin(&self) -> [f32; 3] {
        [
            self.origin[0] / 16.0 - 0.5,
            self.origin[1] / 16.0 - 0.5,
            self.origin[2] / 16.0 - 0.5,
        ]
    }

    /// Get the angle in radians.
    pub fn angle_radians(&self) -> f32 {
        self.angle.to_radians()
    }

    /// Get the rescale factor for this rotation.
    /// When rescale is true, the element is scaled to maintain its original size.
    pub fn rescale_factor(&self) -> f32 {
        if self.rescale {
            1.0 / self.angle_radians().cos()
        } else {
            1.0
        }
    }
}
