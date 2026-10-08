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

/// An RGBA picture: a skin (64×64, or a multiple for HD skins) or a cape
/// (64×32, or a multiple).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, row by row, RGBA.
    pub rgba: Vec<u8>,
}

impl Texture {
    /// `None` when the bytes do not fill the size.
    #[must_use]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Option<Self> {
        (width > 0 && height > 0 && rgba.len() == width as usize * height as usize * 4).then_some(
            Self {
                width,
                height,
                rgba,
            },
        )
    }

    fn texel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y.min(self.height - 1) as usize * self.width as usize
            + x.min(self.width - 1) as usize)
            * 4;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

/// The arm width the skin is drawn for: classic (Steve, 4 pixels) or slim
/// (Alex, 3 pixels).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Arms {
    #[default]
    Classic,
    Slim,
}

/// What to draw. With no skin the player is a plain grey figure, for when
/// no picture is available.
#[derive(Clone, Copy, Debug)]
pub struct Player<'a> {
    pub skin: Option<&'a Texture>,
    pub arms: Arms,
    pub cape: Option<&'a Texture>,
    /// The skin's second layer (hat, jacket, sleeves, trousers).
    pub outer_layer: bool,
}

/// An orbit camera around the player's middle. `yaw` 0 looks at the face and
/// grows to the player's left; `pitch` lifts the camera; `zoom` 1 fits the
/// whole player in the picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
}

impl Camera {
    pub const MIN_PITCH: f32 = -1.2;
    pub const MAX_PITCH: f32 = 1.2;
    pub const MIN_ZOOM: f32 = 0.6;
    pub const MAX_ZOOM: f32 = 2.5;

    /// A three-quarter view, as a character select shows a player.
    pub const HOME: Self = Self {
        yaw: 0.45,
        pitch: 0.12,
        zoom: 1.0,
    };

    /// The same camera with pitch and zoom kept in range.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            yaw: self.yaw.rem_euclid(std::f32::consts::TAU),
            pitch: self.pitch.clamp(Self::MIN_PITCH, Self::MAX_PITCH),
            zoom: self.zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM),
        }
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::HOME
    }
}

/// One picture: `width * height` pixels, BGRA, with alpha 0 where nothing
/// was drawn so the panel behind shows through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

/// Draws the player. A zero-sized picture is empty.
#[must_use]
pub fn render(player: &Player<'_>, camera: Camera, width: u32, height: u32) -> Frame {
    let triangles = model::triangles(player);
    raster::draw(&triangles, player, camera.clamped(), width, height)
}
