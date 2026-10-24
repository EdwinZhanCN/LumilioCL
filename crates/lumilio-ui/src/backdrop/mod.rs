//! A deterministic, animated ASCII environment for the Home canvas.
//!
//! The scene deliberately stays renderer-independent for now. GPUI 0.2.2 does
//! not expose a stable public glyph-atlas shader API, so the same small scene
//! contract is expressed with normal text elements and can move to a shader
//! later without changing the Home or navigation layers.

#[cfg(test)]
mod tests;

mod glyphs;

use glyphs::{
    cave_glyph, lake_water_glyph, moon_glyph, oak_shape, spruce_shape, star_glyph, star_twinkle,
    tree_glyph, watcher_glyph,
};
pub use scene::{BackdropColors, render};
use scene::{
    BiomeZone, GLYPH_RAMP, SceneFrame, SceneTint, SurfaceKind, SurfaceRows, TAU, TerrainLayer,
    TerrainSample, biome_zone, generate_scene, hash01, lake_rows_at, line_tint, surface_rows_at,
    terrain_glyph,
};

mod scene;
