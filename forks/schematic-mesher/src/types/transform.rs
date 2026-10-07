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
///
/// Two JSON forms exist: the single-axis `{"axis", "angle"}` form, and the Euler
/// form `{"x", "y", "z"}` that Minecraft 26.x uses for angles beyond ±45°.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementRotation {
    /// Origin point for rotation (in 0-16 Minecraft coordinates).
    #[serde(default = "default_origin")]
    pub origin: [f32; 3],
    /// Axis to rotate around (single-axis form).
    #[serde(default)]
    pub axis: Option<Axis>,
    /// Rotation angle in degrees (single-axis form).
    #[serde(default)]
    pub angle: f32,
    /// Degrees about X (Euler form).
    #[serde(default)]
    pub x: f32,
    /// Degrees about Y (Euler form).
    #[serde(default)]
    pub y: f32,
    /// Degrees about Z (Euler form).
    #[serde(default)]
    pub z: f32,
    /// Whether to rescale the element after rotation.
    #[serde(default)]
    pub rescale: bool,
}

fn default_origin() -> [f32; 3] {
    [8.0, 8.0, 8.0]
}

impl ElementRotation {
    /// Single-axis rotation, the form every Minecraft version accepts.
    pub fn single(origin: [f32; 3], axis: Axis, angle: f32, rescale: bool) -> Self {
        Self {
            origin,
            axis: Some(axis),
            angle,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            rescale,
        }
    }

    /// Convert origin from Minecraft coordinates (0-16) to normalized (-0.5 to 0.5).
    pub fn normalized_origin(&self) -> [f32; 3] {
        [
            self.origin[0] / 16.0 - 0.5,
            self.origin[1] / 16.0 - 0.5,
            self.origin[2] / 16.0 - 0.5,
        ]
    }

    /// The linear part of the transform about the origin: rotation, then (when
    /// `rescale` is set) a per-axis scale applied before it.
    ///
    /// Follows Minecraft 26.3 `CuboidRotation`: the single-axis form is a
    /// right-handed rotation about the positive axis; the Euler form is JOML
    /// `rotationZYX(z, y, x)`, i.e. `Rz * Ry * Rx` (X applied first). Rescale
    /// divides each axis by the largest component of its rotated unit vector, so
    /// a 45° element still spans the full block.
    pub fn matrix(&self) -> glam::Mat3 {
        use glam::Mat3;
        let rotation = match self.axis {
            Some(Axis::X) => Mat3::from_rotation_x(self.angle.to_radians()),
            Some(Axis::Y) => Mat3::from_rotation_y(self.angle.to_radians()),
            Some(Axis::Z) => Mat3::from_rotation_z(self.angle.to_radians()),
            None => {
                Mat3::from_rotation_z(self.z.to_radians())
                    * Mat3::from_rotation_y(self.y.to_radians())
                    * Mat3::from_rotation_x(self.x.to_radians())
            }
        };
        if !self.rescale || rotation.abs_diff_eq(Mat3::IDENTITY, 1e-6) {
            return rotation;
        }
        let factor = |axis: glam::Vec3| 1.0 / (rotation * axis).abs().max_element();
        rotation
            * Mat3::from_diagonal(glam::Vec3::new(
                factor(glam::Vec3::X),
                factor(glam::Vec3::Y),
                factor(glam::Vec3::Z),
            ))
    }
}
