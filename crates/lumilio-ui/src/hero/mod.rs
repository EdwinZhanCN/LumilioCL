//! Home hero: procedural pixel-art scenes that reflect Home's state
//! (ADR 0004, design language §5).
//!
//! Each frame the current scene is rasterised on the CPU into a small texel
//! grid (pure Rust, see [`raster`]), composited with an outgoing scene or the
//! unloaded void during chunk loading, and painted with one quad per rectangle
//! of identical texels. [`director`] decides what is shown; showcase copy, the
//! live HUD chip, and slide indicators are ordinary GPUI elements on top.

mod director;
pub(crate) mod dissolve;
pub(crate) mod noise;
pub(crate) mod raster;
mod scenes;
mod timeline;
mod transition;
mod wallpaper;

pub use director::HeroMode;
use hero_carousel::{FrameSpec, MAX_COLUMNS, MIN_COLUMNS, TARGET_TEXEL};
pub use hero_carousel::{HeroCarousel, accent};
#[cfg(test)]
use hero_carousel::{grid_layout, rasterize};
pub use scenes::{Landmark, Scene};

#[cfg(test)]
mod tests;

mod hero_carousel;
