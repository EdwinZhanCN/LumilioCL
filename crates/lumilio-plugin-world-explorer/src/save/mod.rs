//! The save base map: a single-player world's own region files drawn from
//! above. Only level 0 is drawn here; the host builds the coarser levels from
//! it and keeps them until a region file changes (plan W4, W6).
pub(crate) mod colors;
pub(crate) mod positions;
mod render;

#[cfg(test)]
mod tests;

mod saved_world;

#[cfg(test)]
use saved_world::HostRegion;
pub(crate) use saved_world::{BASE, info, sources, tile};
