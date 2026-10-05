//! Texture atlas builder using simple row packing.

use crate::error::{MesherError, Result};
use crate::resource_pack::TextureData;
use image::ImageEncoder;
use std::collections::HashMap;

/// A region within the texture atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    /// U coordinate of the left edge (0-1).
    pub u_min: f32,
    /// V coordinate of the top edge (0-1).
    pub v_min: f32,
    /// U coordinate of the right edge (0-1).
    pub u_max: f32,
    /// V coordinate of the bottom edge (0-1).
    pub v_max: f32,
}

impl AtlasRegion {
    /// Get the width of this region in UV space.
    pub fn width(&self) -> f32 {
        self.u_max - self.u_min
    }

    /// Get the height of this region in UV space.
    pub fn height(&self) -> f32 {
        self.v_max - self.v_min
    }

    /// Transform a local UV coordinate (0-1) to atlas coordinate.
    pub fn transform_uv(&self, u: f32, v: f32) -> [f32; 2] {
        [
            self.u_min + u * self.width(),
            self.v_min + v * self.height(),
        ]
    }
}

/// A built texture atlas.
#[derive(Debug, Clone)]
pub struct TextureAtlas {
    /// Width of the atlas in pixels.
    pub width: u32,
    /// Height of the atlas in pixels.
    pub height: u32,
    /// RGBA pixel data.
    pub pixels: Vec<u8>,
    /// Mapping from texture path to atlas region.
    pub regions: HashMap<String, AtlasRegion>,
}

impl TextureAtlas {
    /// Get the region for a texture.
    pub fn get_region(&self, texture_path: &str) -> Option<&AtlasRegion> {
        self.regions.get(texture_path)
    }

    /// Check if the atlas contains a texture.
    pub fn contains(&self, texture_path: &str) -> bool {
        self.regions.contains_key(texture_path)
    }

    /// Create an empty atlas.
    pub fn empty() -> Self {
        Self {
            width: 16,
            height: 16,
            pixels: vec![255; 16 * 16 * 4], // White
            regions: HashMap::new(),
        }
    }

    /// Export the atlas as PNG bytes.
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let cursor = std::io::Cursor::new(&mut bytes);
        let encoder = image::codecs::png::PngEncoder::new(cursor);

        encoder
            .write_image(
                &self.pixels,
                self.width,
                self.height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| MesherError::AtlasBuild(format!("Failed to encode PNG: {}", e)))?;

        Ok(bytes)
    }
}

/// Builder for creating texture atlases.
pub struct AtlasBuilder {
    max_size: u32,
    padding: u32,
    textures: HashMap<String, TextureData>,
}

impl AtlasBuilder {
    /// Create a new atlas builder.
    pub fn new(max_size: u32, padding: u32) -> Self {
        Self {
            max_size,
            padding,
            textures: HashMap::new(),
        }
    }

    /// Add a texture to the atlas.
    pub fn add_texture(&mut self, path: String, texture: TextureData) {
        self.textures.insert(path, texture);
    }

    /// Build the texture atlas using simple row packing.
    pub fn build(self) -> Result<TextureAtlas> {
        if self.textures.is_empty() {
            return Ok(TextureAtlas::empty());
        }

        let padding = self.padding;
        let max_size = self.max_size;

        // Sort textures by height (tallest first) for better packing, with the
        // path as a tiebreaker so packing is FULLY deterministic. Without the
        // tiebreaker, equal-height textures kept their (hash-random) HashMap
        // order, making the packed atlas — and the exported GLB — vary run-to-run.
        let mut textures: Vec<_> = self.textures.into_iter().collect();
        textures.sort_by(|a, b| b.1.height.cmp(&a.1.height).then_with(|| a.0.cmp(&b.0)));

        // Calculate required atlas size
        let total_area: u32 = textures
            .iter()
            .map(|(_, t)| (t.width + padding * 2) * (t.height + padding * 2))
            .sum();

        // Start with minimum size that could fit all textures
        let min_size = (total_area as f64).sqrt().ceil() as u32;
        let mut atlas_size = 64u32;
        while atlas_size < min_size && atlas_size < max_size {
            atlas_size *= 2;
        }

        // Try to pack at increasing sizes
        loop {
            if atlas_size > max_size {
                return Err(MesherError::AtlasBuild(format!(
                    "Failed to pack {} textures into {}x{} atlas",
                    textures.len(),
                    max_size,
                    max_size
                )));
            }

            if let Some((pixels, regions)) = try_pack(&textures, atlas_size, padding) {
                return Ok(TextureAtlas {
                    width: atlas_size,
                    height: atlas_size,
                    pixels,
                    regions,
                });
            }

            atlas_size *= 2;
        }
    }
}

/// Try to pack textures into an atlas of the given size.
fn try_pack(
    textures: &[(String, TextureData)],
    atlas_size: u32,
    padding: u32,
) -> Option<(Vec<u8>, HashMap<String, AtlasRegion>)> {
    let mut pixels = vec![0u8; (atlas_size * atlas_size * 4) as usize];
    let mut regions = HashMap::new();

    // Simple row-based packing
    let mut current_x = 0u32;
    let mut current_y = 0u32;
    let mut row_height = 0u32;

    for (path, texture) in textures {
        let tex_width = texture.width + padding * 2;
        let tex_height = texture.height + padding * 2;

        // Check if we need to start a new row
        if current_x + tex_width > atlas_size {
            current_x = 0;
            current_y += row_height;
            row_height = 0;
        }

        // Check if we've run out of space
        if current_y + tex_height > atlas_size {
            return None;
        }

        // Place the texture
        let x = current_x + padding;
        let y = current_y + padding;

        // Copy texture pixels to atlas with edge-clamped padding.
        // Padding pixels get the nearest edge pixel color to prevent
        // bilinear filtering from bleeding black at texel boundaries.
        for py in 0..tex_height {
            for px in 0..tex_width {
                let sx = (px as i32 - padding as i32).clamp(0, texture.width as i32 - 1) as u32;
                let sy = (py as i32 - padding as i32).clamp(0, texture.height as i32 - 1) as u32;

                let src_idx = ((sy * texture.width + sx) * 4) as usize;
                let dst_x = current_x + px;
                let dst_y = current_y + py;
                let dst_idx = ((dst_y * atlas_size + dst_x) * 4) as usize;

                if src_idx + 4 <= texture.pixels.len() && dst_idx + 4 <= pixels.len() {
                    pixels[dst_idx..dst_idx + 4]
                        .copy_from_slice(&texture.pixels[src_idx..src_idx + 4]);
                }
            }
        }

        // Record the region
        let region = AtlasRegion {
            u_min: x as f32 / atlas_size as f32,
            v_min: y as f32 / atlas_size as f32,
            u_max: (x + texture.width) as f32 / atlas_size as f32,
            v_max: (y + texture.height) as f32 / atlas_size as f32,
        };
        regions.insert(path.clone(), region);

        // Update position
        current_x += tex_width;
        row_height = row_height.max(tex_height);
    }

    Some((pixels, regions))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_texture(width: u32, height: u32, color: [u8; 4]) -> TextureData {
        let pixels: Vec<u8> = (0..width * height)
            .flat_map(|_| color.iter().copied())
            .collect();
        TextureData::new(width, height, pixels)
    }

    #[test]
    fn test_empty_atlas() {
        let builder = AtlasBuilder::new(256, 0);
        let atlas = builder.build().unwrap();
        assert_eq!(atlas.width, 16);
        assert_eq!(atlas.height, 16);
        assert!(atlas.regions.is_empty());
    }

    #[test]
    fn test_single_texture_atlas() {
        let mut builder = AtlasBuilder::new(256, 0);
        builder.add_texture(
            "test".to_string(),
            create_test_texture(16, 16, [255, 0, 0, 255]),
        );

        let atlas = builder.build().unwrap();
        assert!(atlas.regions.contains_key("test"));

        let region = atlas.get_region("test").unwrap();
        assert!(region.u_min >= 0.0);
        assert!(region.u_max <= 1.0);
        assert!(region.v_min >= 0.0);
        assert!(region.v_max <= 1.0);
    }

    #[test]
    fn test_multiple_textures() {
        let mut builder = AtlasBuilder::new(256, 1);
        builder.add_texture(
            "red".to_string(),
            create_test_texture(16, 16, [255, 0, 0, 255]),
        );
        builder.add_texture(
            "green".to_string(),
            create_test_texture(16, 16, [0, 255, 0, 255]),
        );
        builder.add_texture(
            "blue".to_string(),
            create_test_texture(16, 16, [0, 0, 255, 255]),
        );

        let atlas = builder.build().unwrap();
        assert_eq!(atlas.regions.len(), 3);
        assert!(atlas.contains("red"));
        assert!(atlas.contains("green"));
        assert!(atlas.contains("blue"));
    }

    #[test]
    fn test_atlas_region_transform() {
        let region = AtlasRegion {
            u_min: 0.25,
            v_min: 0.5,
            u_max: 0.5,
            v_max: 0.75,
        };

        let [u, v] = region.transform_uv(0.0, 0.0);
        assert!((u - 0.25).abs() < 0.001);
        assert!((v - 0.5).abs() < 0.001);

        let [u, v] = region.transform_uv(1.0, 1.0);
        assert!((u - 0.5).abs() < 0.001);
        assert!((v - 0.75).abs() < 0.001);
    }
}
