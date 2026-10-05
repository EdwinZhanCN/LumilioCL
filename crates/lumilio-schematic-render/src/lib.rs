//! Offscreen 3D rendering of a Minecraft schematic (ADR 0028).
//!
//! Nucleation parses the schematic and meshes it with the textures of a resource pack; its wgpu
//! renderer draws one frame at a time and reads it back. This crate knows nothing about the UI:
//! it hands out BGRA pixels, the order GPUI wants.

mod error;
mod scene;
mod view;

pub use error::SceneError;
pub use scene::{Frame, Scene};
pub use view::View;

#[cfg(test)]
mod tests;
