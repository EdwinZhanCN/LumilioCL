//! The hero scenes. The four showcase scenes each turn one game mechanic into
//! motion; Hearth is Continue's own scene.

pub mod caves;
pub mod dawn;
pub mod hearth;
pub mod portal;
pub mod redstone;

pub use hearth::Landmark;
pub use scene::Scene;

#[cfg(test)]
mod tests;

mod scene;
