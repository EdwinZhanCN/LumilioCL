//! Baking: Mojang's resource layout of any game version in, [`BakedPack`](crate::BakedPack) out.

mod blockstate;
mod colormap;
mod model;

mod baker;

pub use baker::{bake, bake_over, canonical_texture};
