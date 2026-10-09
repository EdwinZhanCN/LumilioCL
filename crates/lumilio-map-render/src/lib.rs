//! Orthographic map composition, independent of launcher and GPUI types.
mod scene;
pub use scene::Scene;

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

#[derive(Clone, Copy, Debug, Default)]
pub struct Grid {
    pub chunks: bool,
    pub regions: bool,
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

#[cfg(test)]
mod tests;
