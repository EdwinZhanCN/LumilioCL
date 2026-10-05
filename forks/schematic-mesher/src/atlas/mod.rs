//! Texture atlas building.
//!
//! This module combines multiple textures into a single atlas
//! and remaps UV coordinates accordingly.

mod builder;

pub use builder::{AtlasBuilder, TextureAtlas, AtlasRegion};
