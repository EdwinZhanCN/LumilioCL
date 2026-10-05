//! Texture loading and handling.

/// Animation metadata from .png.mcmeta files.
#[derive(Debug, Clone)]
pub struct AnimationMeta {
    /// Ticks per frame (default 1).
    pub frametime: u32,
    /// Whether to interpolate between frames.
    pub interpolate: bool,
    /// Explicit frame order. If None, frames play sequentially.
    pub frames: Option<Vec<AnimFrame>>,
    /// Override frame width (defaults to texture width).
    pub frame_width: Option<u32>,
    /// Override frame height (defaults to frame_width or texture width for square frames).
    pub frame_height: Option<u32>,
}

/// A single frame in an animation sequence.
#[derive(Debug, Clone)]
pub struct AnimFrame {
    /// Index of the frame in the sprite sheet.
    pub index: u32,
    /// Override tick duration for this frame.
    pub time: Option<u32>,
}

/// Parse a .png.mcmeta JSON string into animation metadata.
pub fn parse_mcmeta(json: &str) -> Option<AnimationMeta> {
    let parsed: serde_json::Value = serde_json::from_str(json).ok()?;
    let anim = parsed.get("animation")?;

    let frametime = anim.get("frametime")
        .and_then(|v| v.as_u64())
        .unwrap_or(1) as u32;

    let interpolate = anim.get("interpolate")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let frame_width = anim.get("width").and_then(|v| v.as_u64()).map(|v| v as u32);
    let frame_height = anim.get("height").and_then(|v| v.as_u64()).map(|v| v as u32);

    let frames = anim.get("frames").and_then(|v| v.as_array()).map(|arr| {
        arr.iter().filter_map(|entry| {
            if let Some(idx) = entry.as_u64() {
                Some(AnimFrame { index: idx as u32, time: None })
            } else if let Some(obj) = entry.as_object() {
                let index = obj.get("index")?.as_u64()? as u32;
                let time = obj.get("time").and_then(|v| v.as_u64()).map(|v| v as u32);
                Some(AnimFrame { index, time })
            } else {
                None
            }
        }).collect()
    });

    Some(AnimationMeta { frametime, interpolate, frames, frame_width, frame_height })
}

/// Raw texture data loaded from PNG.
#[derive(Debug, Clone)]
pub struct TextureData {
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in pixels.
    pub height: u32,
    /// RGBA8 pixel data (4 bytes per pixel).
    pub pixels: Vec<u8>,
    /// Whether this texture has animation metadata.
    pub is_animated: bool,
    /// Animation frame count (1 if not animated).
    pub frame_count: u32,
    /// Parsed animation metadata from .mcmeta file (if present).
    pub animation: Option<AnimationMeta>,
}

impl TextureData {
    /// Create a new texture from RGBA data.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        Self {
            width,
            height,
            pixels,
            is_animated: false,
            frame_count: 1,
            animation: None,
        }
    }

    /// Create a placeholder texture (magenta/black checkerboard).
    pub fn placeholder() -> Self {
        let size = 16;
        let mut pixels = vec![0u8; (size * size * 4) as usize];

        for y in 0..size {
            for x in 0..size {
                let idx = ((y * size + x) * 4) as usize;
                let is_magenta = ((x / 2) + (y / 2)) % 2 == 0;

                if is_magenta {
                    pixels[idx] = 255; // R
                    pixels[idx + 1] = 0; // G
                    pixels[idx + 2] = 255; // B
                    pixels[idx + 3] = 255; // A
                } else {
                    pixels[idx] = 0; // R
                    pixels[idx + 1] = 0; // G
                    pixels[idx + 2] = 0; // B
                    pixels[idx + 3] = 255; // A
                }
            }
        }

        Self {
            width: size,
            height: size,
            pixels,
            is_animated: false,
            frame_count: 1,
            animation: None,
        }
    }

    /// Check if this texture has transparency.
    pub fn has_transparency(&self) -> bool {
        self.pixels.chunks(4).any(|pixel| pixel[3] < 255)
    }

    /// Check if this texture is TRANSLUCENT — has any pixel with intermediate
    /// alpha (`0 < a < 255`). This distinguishes alpha-blended blocks (slime,
    /// honey, stained glass, ice) from binary alpha-tested cutouts (leaves,
    /// glass panes), whose pixels are only ever fully opaque or fully clear.
    pub fn has_translucency(&self) -> bool {
        self.pixels
            .chunks(4)
            .any(|pixel| pixel[3] > 0 && pixel[3] < 255)
    }

    /// Get a pixel at (x, y).
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let idx = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[idx],
            self.pixels[idx + 1],
            self.pixels[idx + 2],
            self.pixels[idx + 3],
        ]
    }

    /// Encode this texture as PNG bytes.
    pub fn to_png(&self) -> Result<Vec<u8>, image::ImageError> {
        use image::{ImageBuffer, Rgba};

        let img: ImageBuffer<Rgba<u8>, _> =
            ImageBuffer::from_raw(self.width, self.height, self.pixels.clone())
                .expect("Failed to create image buffer from TextureData");

        let mut bytes = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut bytes);
        img.write_to(&mut cursor, image::ImageFormat::Png)?;
        Ok(bytes)
    }

    /// Apply parsed .mcmeta animation metadata to this texture.
    pub fn apply_mcmeta(&mut self, meta: AnimationMeta) {
        // Determine frame dimensions from mcmeta or default to width×width (square frames)
        let fw = meta.frame_width.unwrap_or(self.width);
        let fh = meta.frame_height.unwrap_or(fw);

        if fw > 0 && fh > 0 && self.height >= fh {
            self.frame_count = self.height / fh;
            self.is_animated = self.frame_count > 1;
        }
        self.animation = Some(meta);
    }

    /// Get the first frame of an animated texture (or the whole texture if not animated).
    pub fn first_frame(&self) -> TextureData {
        if !self.is_animated || self.frame_count <= 1 {
            return self.clone();
        }

        // Determine frame dimensions
        let frame_height = if let Some(ref meta) = self.animation {
            meta.frame_height.unwrap_or(meta.frame_width.unwrap_or(self.width))
        } else {
            self.height / self.frame_count
        };

        // Determine which frame index to use (mcmeta may specify explicit frame order)
        let first_frame_index = if let Some(ref meta) = self.animation {
            if let Some(ref frames) = meta.frames {
                frames.first().map(|f| f.index).unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        let row_bytes = (self.width * 4) as usize;
        let frame_start = (first_frame_index * frame_height) as usize * row_bytes;
        let frame_size = (self.width * frame_height * 4) as usize;

        // Bounds check
        if frame_start + frame_size > self.pixels.len() {
            // Fall back to first sequential frame
            return Self {
                width: self.width,
                height: frame_height,
                pixels: self.pixels[..frame_size.min(self.pixels.len())].to_vec(),
                is_animated: false,
                frame_count: 1,
                animation: None,
            };
        }

        Self {
            width: self.width,
            height: frame_height,
            pixels: self.pixels[frame_start..frame_start + frame_size].to_vec(),
            is_animated: false,
            frame_count: 1,
            animation: None,
        }
    }
}

/// Load a texture from PNG bytes.
pub fn load_texture_from_bytes(data: &[u8]) -> Result<TextureData, image::ImageError> {
    let img = image::load_from_memory(data)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();

    // Check for animation (texture is taller than wide, height is multiple of width)
    let is_animated = height > width && height % width == 0;
    let frame_count = if is_animated { height / width } else { 1 };

    Ok(TextureData {
        width,
        height,
        pixels: rgba.into_raw(),
        is_animated,
        frame_count,
        animation: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_texture() {
        let tex = TextureData::placeholder();
        assert_eq!(tex.width, 16);
        assert_eq!(tex.height, 16);
        assert_eq!(tex.pixels.len(), 16 * 16 * 4);
        assert!(!tex.has_transparency());
    }

    #[test]
    fn test_get_pixel() {
        let tex = TextureData::new(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]);

        assert_eq!(tex.get_pixel(0, 0), [255, 0, 0, 255]); // Red
        assert_eq!(tex.get_pixel(1, 0), [0, 255, 0, 255]); // Green
        assert_eq!(tex.get_pixel(0, 1), [0, 0, 255, 255]); // Blue
        assert_eq!(tex.get_pixel(1, 1), [255, 255, 255, 255]); // White
    }

    #[test]
    fn test_has_transparency() {
        let opaque = TextureData::new(1, 1, vec![255, 0, 0, 255]);
        assert!(!opaque.has_transparency());

        let transparent = TextureData::new(1, 1, vec![255, 0, 0, 128]);
        assert!(transparent.has_transparency());
    }

    #[test]
    fn test_parse_mcmeta_basic() {
        let json = r#"{"animation":{"frametime":2,"interpolate":true}}"#;
        let meta = parse_mcmeta(json).unwrap();
        assert_eq!(meta.frametime, 2);
        assert!(meta.interpolate);
        assert!(meta.frames.is_none());
        assert!(meta.frame_width.is_none());
    }

    #[test]
    fn test_parse_mcmeta_with_frames() {
        let json = r#"{"animation":{"frametime":1,"frames":[0,2,1,{"index":3,"time":5}]}}"#;
        let meta = parse_mcmeta(json).unwrap();
        let frames = meta.frames.unwrap();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0].index, 0);
        assert!(frames[0].time.is_none());
        assert_eq!(frames[1].index, 2);
        assert_eq!(frames[3].index, 3);
        assert_eq!(frames[3].time, Some(5));
    }

    #[test]
    fn test_parse_mcmeta_no_animation_key() {
        let json = r#"{"pack":{"description":"test"}}"#;
        assert!(parse_mcmeta(json).is_none());
    }

    #[test]
    fn test_parse_mcmeta_defaults() {
        let json = r#"{"animation":{}}"#;
        let meta = parse_mcmeta(json).unwrap();
        assert_eq!(meta.frametime, 1);
        assert!(!meta.interpolate);
        assert!(meta.frames.is_none());
    }

    #[test]
    fn test_apply_mcmeta() {
        // 16x64 texture = 4 frames of 16x16
        let pixels = vec![0u8; 16 * 64 * 4];
        let mut tex = TextureData::new(16, 64, pixels);
        assert!(!tex.is_animated); // heuristic not applied in new()

        let meta = AnimationMeta {
            frametime: 2,
            interpolate: false,
            frames: None,
            frame_width: None,
            frame_height: None,
        };
        tex.apply_mcmeta(meta);
        assert!(tex.is_animated);
        assert_eq!(tex.frame_count, 4);
    }

    #[test]
    fn test_first_frame_with_mcmeta_explicit_order() {
        // 16x48 texture = 3 frames, mcmeta says first frame is index 2
        let mut pixels = vec![0u8; 16 * 48 * 4];
        // Mark frame 2 (rows 32-47) with distinctive pixel
        pixels[32 * 16 * 4] = 42;

        let tex = TextureData {
            width: 16,
            height: 48,
            pixels,
            is_animated: true,
            frame_count: 3,
            animation: Some(AnimationMeta {
                frametime: 1,
                interpolate: false,
                frames: Some(vec![
                    AnimFrame { index: 2, time: None },
                    AnimFrame { index: 0, time: None },
                    AnimFrame { index: 1, time: None },
                ]),
                frame_width: None,
                frame_height: None,
            }),
        };

        let frame = tex.first_frame();
        assert_eq!(frame.width, 16);
        assert_eq!(frame.height, 16);
        assert_eq!(frame.pixels[0], 42); // Should be frame index 2
    }
}
