//! Skins and capes as pixels for a preview: decoded, checked and brought to
//! the current layout. A 64×32 skin (before Minecraft 1.8) has one layer and
//! shares one arm and one leg between both sides; it is upgraded the way the
//! game upgrades it, by mirroring the right arm and leg into the left ones'
//! places. The layout facts are the game's skin format
//! (<https://minecraft.wiki/w/Skin>).

use image::{ImageReader, RgbaImage};
use std::io;

use super::{MAX_SIDE, PICTURE_LIMIT, SkinError};

/// What an account looks like, as pixels for a preview. No skin is the
/// game's default look.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AccountLook {
    pub model: super::SkinModel,
    pub skin: Option<Pixels>,
    pub cape: Option<Pixels>,
}

impl AccountLook {
    /// The pictures a skin choice loaded, decoded for the preview.
    pub fn from_loaded(loaded: &super::LoadedSkin) -> Result<Self, SkinError> {
        Ok(Self {
            model: loaded.model,
            skin: loaded
                .skin
                .as_ref()
                .map(|texture| skin_pixels(&texture.png))
                .transpose()?,
            cape: loaded
                .cape
                .as_ref()
                .map(|texture| cape_pixels(&texture.png))
                .transpose()?,
        })
    }
}

/// A decoded picture, RGBA, row by row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn decode(bytes: &[u8]) -> Result<RgbaImage, SkinError> {
    if bytes.len() as u64 > PICTURE_LIMIT {
        return Err(SkinError::Picture("the file is too large".to_owned()));
    }
    let mut reader = ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| SkinError::Picture(error.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader
        .decode()
        .map_err(|error| SkinError::Picture(error.to_string()))?
        .to_rgba8())
}

fn pixels(image: RgbaImage) -> Pixels {
    Pixels {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// A skin in the 64×64 layout (or a multiple of it). 64×32 skins are
/// upgraded; any other shape is refused.
pub fn skin_pixels(bytes: &[u8]) -> Result<Pixels, SkinError> {
    let image = decode(bytes)?;
    let (width, height) = image.dimensions();
    if width == 0 || width % 64 != 0 {
        return Err(SkinError::Picture(format!(
            "a skin is 64 pixels wide or a multiple of it, not {width}"
        )));
    }
    if height == width {
        Ok(pixels(image))
    } else if height * 2 == width {
        Ok(pixels(upgrade_legacy(&image)))
    } else {
        Err(SkinError::Picture(format!(
            "a skin is 64×64 or 64×32, not {width}×{height}"
        )))
    }
}

/// A cape in the 64×32 layout (or a multiple of it). The early 22×17
/// pictures are placed on a 64×32 canvas; any other shape is refused.
pub fn cape_pixels(bytes: &[u8]) -> Result<Pixels, SkinError> {
    let image = decode(bytes)?;
    let (width, height) = image.dimensions();
    if (width, height) == (22, 17) {
        let mut canvas = RgbaImage::new(64, 32);
        image::imageops::replace(&mut canvas, &image, 0, 0);
        return Ok(pixels(canvas));
    }
    if width == 0 || width % 64 != 0 || height * 2 != width {
        return Err(SkinError::Picture(format!(
            "a cape is 64×32 or a multiple of it, not {width}×{height}"
        )));
    }
    Ok(pixels(image))
}

/// Whether a skin is drawn for slim arms: the two texel columns a classic
/// arm uses beyond a slim one's unwrap are empty.
#[must_use]
pub fn looks_slim(skin: &Pixels) -> bool {
    let scale = skin.width / 64;
    if scale == 0 || skin.height != skin.width {
        return false;
    }
    (54 * scale..56 * scale).all(|x| {
        (20 * scale..32 * scale).all(|y| {
            let at = ((y * skin.width + x) * 4 + 3) as usize;
            skin.rgba.get(at).copied() == Some(0)
        })
    })
}

/// The game's own upgrade of a 64×32 skin: each face of the right leg and
/// right arm is copied, mirrored, to the left limb's place, and a hat area
/// with no transparency at all is cleared (old skins filled it).
fn upgrade_legacy(old: &RgbaImage) -> RgbaImage {
    let scale = old.width() / 64;
    let mut new = RgbaImage::new(old.width(), old.width());
    image::imageops::replace(&mut new, old, 0, 0);
    // (x, y, w, h) of a face of the right limb, and where its mirror goes.
    let faces: [(u32, u32, u32, u32, u32, u32); 12] = [
        // Leg: top, bottom, outer side, front, inner side, back.
        (4, 16, 4, 4, 20, 48),
        (8, 16, 4, 4, 24, 48),
        (0, 20, 4, 12, 24, 52),
        (4, 20, 4, 12, 20, 52),
        (8, 20, 4, 12, 16, 52),
        (12, 20, 4, 12, 28, 52),
        // Arm: top, bottom, outer side, front, inner side, back.
        (44, 16, 4, 4, 36, 48),
        (48, 16, 4, 4, 40, 48),
        (40, 20, 4, 12, 40, 52),
        (44, 20, 4, 12, 36, 52),
        (48, 20, 4, 12, 32, 52),
        (52, 20, 4, 12, 44, 52),
    ];
    for (x, y, w, h, to_x, to_y) in faces {
        for dy in 0..h * scale {
            for dx in 0..w * scale {
                let pixel = *old.get_pixel(x * scale + dx, y * scale + dy);
                new.put_pixel(
                    to_x * scale + (w * scale - 1 - dx),
                    to_y * scale + dy,
                    pixel,
                );
            }
        }
    }
    let hat = (32 * scale..64 * scale).flat_map(|x| (0..16 * scale).map(move |y| (x, y)));
    if hat.clone().all(|(x, y)| new.get_pixel(x, y).0[3] == 255) {
        for (x, y) in hat {
            new.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
        }
    }
    new
}
