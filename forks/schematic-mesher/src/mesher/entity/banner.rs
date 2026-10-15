//! Banner textures: the base dye and the `patterns` property drawn onto the
//! flag (schematic-mesher, AGPL-3.0-only, Schem-at). The pole, bar and flag
//! are the game's own layers (LumilioCL local change, plan schematic-studio T7).

use crate::resource_pack::{ResourcePack, TextureData};

/// Composite a banner texture from base color and optional patterns.
///
/// The banner texture is 64x64. The base.png and pattern masks define
/// the flag region. We fill the base color, then overlay each pattern
/// with its dye color.
///
/// Returns a TextureData for the composited banner.
pub(crate) fn composite_banner_texture(
    resource_pack: &ResourcePack,
    base_color: &str,
    patterns: &[(String, String)], // [(pattern_name, dye_color), ...]
) -> Option<TextureData> {
    // Start with the base texture (entity/banner/banner_base.png) for the frame/pole
    let base_tex = resource_pack.get_texture("entity/banner/banner_base")?;
    let base_frame = base_tex.first_frame();

    let width = base_frame.width;
    let height = base_frame.height;
    let mut pixels = base_frame.pixels.clone();

    // Load the base pattern mask
    let base_mask = resource_pack.get_texture("entity/banner/base")?;
    let base_mask_frame = base_mask.first_frame();

    // Fill the flag region with the base dye color using the base mask
    let base_rgb = dye_rgb(base_color);
    apply_pattern_mask(&mut pixels, width, height, &base_mask_frame, base_rgb);

    // Apply each pattern layer
    for (pattern_name, dye_color) in patterns {
        let pattern_path = format!("entity/banner/{}", pattern_name);
        if let Some(pattern_tex) = resource_pack.get_texture(&pattern_path) {
            let pattern_frame = pattern_tex.first_frame();
            let rgb = dye_rgb(dye_color);
            apply_pattern_mask(&mut pixels, width, height, &pattern_frame, rgb);
        }
    }

    Some(TextureData {
        width,
        height,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Apply a grayscale+alpha pattern mask to the banner texture.
///
/// For each pixel in the mask that has alpha > 0:
/// - Blend dye_color * mask_gray * mask_alpha over the destination pixel.
fn apply_pattern_mask(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    mask: &TextureData,
    dye_color: [u8; 3],
) {
    let mask_w = mask.width.min(width);
    let mask_h = mask.height.min(height);

    for y in 0..mask_h {
        for x in 0..mask_w {
            let mask_idx = ((y * mask.width + x) * 4) as usize;
            let dst_idx = ((y * width + x) * 4) as usize;

            if mask_idx + 3 >= mask.pixels.len() || dst_idx + 3 >= pixels.len() {
                continue;
            }

            let mask_a = mask.pixels[mask_idx + 3] as f32 / 255.0;
            if mask_a < 0.01 {
                continue;
            }

            // Grayscale intensity from the mask
            let mask_gray = mask.pixels[mask_idx] as f32 / 255.0;

            // Source color = dye_color * gray_intensity
            let src_r = dye_color[0] as f32 * mask_gray;
            let src_g = dye_color[1] as f32 * mask_gray;
            let src_b = dye_color[2] as f32 * mask_gray;

            // Alpha-over compositing
            let dst_r = pixels[dst_idx] as f32;
            let dst_g = pixels[dst_idx + 1] as f32;
            let dst_b = pixels[dst_idx + 2] as f32;
            let dst_a = pixels[dst_idx + 3] as f32 / 255.0;

            let out_a = mask_a + dst_a * (1.0 - mask_a);
            if out_a > 0.0 {
                pixels[dst_idx] =
                    ((src_r * mask_a + dst_r * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 1] =
                    ((src_g * mask_a + dst_g * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 2] =
                    ((src_b * mask_a + dst_b * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 3] = (out_a * 255.0).min(255.0) as u8;
            }
        }
    }
}

/// A dye's diffuse colour, which tints the banner's base and each pattern
/// (LumilioCL local change, plan schematic-studio T35: this was the map
/// colour table, which the banner renderer never used).
fn dye_rgb(color: &str) -> [u8; 3] {
    super::dye_diffuse(color)
}

/// Parse the "patterns" property string into (pattern_name, dye_color) pairs.
///
/// Format: "pattern1:color1,pattern2:color2,..."
/// Example: "stripe_bottom:red,cross:blue"
pub(crate) fn parse_patterns(patterns_str: &str) -> Vec<(String, String)> {
    if patterns_str.is_empty() {
        return Vec::new();
    }
    patterns_str
        .split(',')
        .filter_map(|entry| {
            let parts: Vec<&str> = entry.trim().splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}
