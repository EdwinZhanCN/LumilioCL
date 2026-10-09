//! Orthographic map composition, independent of launcher and GPUI types.
mod scene;
pub use scene::{Scene, scale_to};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    NoGpu,
    InvalidInput,
    Render(String),
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f64,
    pub z: f64,
    pub blocks_per_pixel: f64,
}

#[derive(Clone, Debug)]
pub struct Tile {
    pub id: String,
    pub x: f64,
    pub z: f64,
    pub span: f64,
    /// Shared so the host keeps one copy per tile across frames.
    pub rgba: std::sync::Arc<[u8]>,
}

/// An icon drawn over the tiles in screen pixels. `rgba` is straight alpha at
/// its own size; the scene scales it to `size` once and keeps it by `id`.
#[derive(Clone, Debug)]
pub struct Sprite {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<[u8]>,
    /// Screen position of the sprite's centre, in pixels from the top left.
    pub x: f64,
    pub y: f64,
    /// Edge length on screen, in pixels.
    pub size: u32,
    pub opacity: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Grid {
    pub chunks: bool,
    pub regions: bool,
}

/// Where one `Scene::render` call spent its time.
#[derive(Clone, Copy, Debug, Default)]
pub struct Timings {
    /// The whole call.
    pub total: std::time::Duration,
    /// Validation, texture uploads, buffers and bind groups, encoding and
    /// submitting; everything before the GPU is waited on.
    pub encode: std::time::Duration,
    /// From requesting the readback until the mapped result arrived: the GPU
    /// finishing the frame plus the copy into the staging buffer.
    pub wait: std::time::Duration,
    /// Removing the row padding into the returned `Vec`.
    pub copy: std::time::Duration,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
    pub timings: Timings,
}

#[cfg(test)]
mod tests;
