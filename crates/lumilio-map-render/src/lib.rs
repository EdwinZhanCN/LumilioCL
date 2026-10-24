//! Orthographic map composition, independent of launcher and GPUI types.
mod scene;

pub use frame::{Camera, Error, Frame, Grid, Sprite, Tile, Timings};
pub use scene::{Scene, scale_to};

#[cfg(test)]
mod tests;

mod frame;
