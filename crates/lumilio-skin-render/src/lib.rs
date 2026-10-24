//! Draws a Minecraft player on the CPU: the skin's two layers on the classic
//! or slim model, and a cape, from an orbit camera, as BGRA pixels (the order
//! GPUI wants). It knows nothing about GPUI or the launcher. The player is a
//! dozen boxes and the textures are pixel art, so a small software rasterizer
//! with nearest sampling draws a frame in a few milliseconds on any machine,
//! with or without a graphics adapter.

mod model;
mod raster;

#[cfg(test)]
mod tests;

mod player;

pub use player::{Arms, BackEquipment, Camera, Frame, Player, Texture, render};
