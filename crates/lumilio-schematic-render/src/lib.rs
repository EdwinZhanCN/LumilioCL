//! Offscreen 3D rendering of a Minecraft schematic (ADR 0028).
//!
//! Nucleation parses the schematic and meshes it with a baked pack and the same game version's
//! entity model layers (`lumilio-block-assets`); its wgpu
//! renderer draws one frame at a time and reads it back. This crate knows nothing about the UI:
//! it hands out BGRA pixels, the order GPUI wants.

mod error;
mod icons;
mod pick;
mod scene;
mod view;

pub use error::SceneError;
pub use icons::{icons, item_icons};
pub use lumilio_block_assets::{BakedPack, EntityLayers};
pub use pick::{Grid, Ray, camera_ray, march, project};
pub use scene::{ENTITY_DISTANCE, Focus, Frame, Picked, Scene, Slice, SliceAxis};
pub use view::{Angle, View};

#[cfg(test)]
mod tests;
