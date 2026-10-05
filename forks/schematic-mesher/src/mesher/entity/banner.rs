use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};
use crate::resource_pack::{ResourcePack, TextureData};

/// Banner model — 64x64 texture.
/// From BannerModel.java + BannerFlagModel.java (MC 1.21.4).
///
/// Standing banner (is_standing=true):
///   - pole: [-1,-42,-1] 2x42x2 tex(44,0)
///   - bar: [-10,-44,-1] 20x2x2 tex(0,42)
///   - flag: [-10,0,-2] 20x40x1 tex(0,0) offset(0,-44,0)
///
/// Wall banner (is_standing=false):
///   - no pole
///   - bar: [-10,-20.5,9.5] 20x2x2 tex(0,42)
///   - flag: [-10,0,-2] 20x40x1 tex(0,0) offset(0,-20.5,10.5)
pub(crate) fn banner_model(is_standing: bool, texture_path: &str) -> EntityModelDef {
    let mut parts = Vec::new();

    // Pole (standing only)
    if is_standing {
        parts.push(EntityPart {
            cubes: vec![EntityCube {
                origin: [-1.0, -42.0, -1.0],
                dimensions: [2.0, 42.0, 2.0],
                tex_offset: [44, 0],
                inflate: 0.0,
                mirror: false,
                skip_faces: vec![],
            }],
            pose: Default::default(),
            children: vec![],
        });
    }

    // Bar (crossbar at top)
    let bar_y = if is_standing { -44.0 } else { -20.5 };
    let bar_z = if is_standing { -1.0 } else { 9.5 };
    parts.push(EntityPart {
        cubes: vec![EntityCube {
            origin: [-10.0, bar_y, bar_z],
            dimensions: [20.0, 2.0, 2.0],
            tex_offset: [0, 42],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    });

    // Flag
    let flag_offset_y = if is_standing { -44.0 } else { -20.5 };
    let flag_offset_z = if is_standing { 0.0 } else { 10.5 };
    parts.push(EntityPart {
        cubes: vec![EntityCube {
            origin: [-10.0, 0.0, -2.0],
            dimensions: [20.0, 40.0, 1.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, flag_offset_y, flag_offset_z],
            ..Default::default()
        },
        children: vec![],
    });

    // Root wrapper matching MC BannerRenderer's transform:
    //   translate(0.5, 0, 0.5) · rotateY(yaw) · scale(2/3, -2/3, -2/3)
    // The negative Y/Z in MC's scale is equivalent to RotX(π) combined with a
    // uniform 2/3 downscale. Position Y=0 (not 24) because MC anchors the
    // banner at the block's bottom, not its top.
    let scale = 2.0 / 3.0;
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 0.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            scale: [scale, scale, scale],
        },
        children: parts,
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: false, // Banner texture has transparent regions
    }
}

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
    // Start with the base texture (entity/banner_base.png) for the frame/pole
    let base_tex = resource_pack.get_texture("entity/banner_base")?;
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
                pixels[dst_idx] = ((src_r * mask_a + dst_r * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 1] = ((src_g * mask_a + dst_g * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 2] = ((src_b * mask_a + dst_b * dst_a * (1.0 - mask_a)) / out_a).min(255.0) as u8;
                pixels[dst_idx + 3] = (out_a * 255.0).min(255.0) as u8;
            }
        }
    }
}

/// Standard Minecraft dye color RGB values (0-255).
fn dye_rgb(color: &str) -> [u8; 3] {
    match color {
        "white" => [255, 255, 255],
        "orange" => [216, 127, 51],
        "magenta" => [178, 76, 216],
        "light_blue" => [102, 153, 216],
        "yellow" => [229, 229, 51],
        "lime" => [127, 204, 25],
        "pink" => [242, 127, 165],
        "gray" => [76, 76, 76],
        "light_gray" => [153, 153, 153],
        "cyan" => [76, 127, 153],
        "purple" => [127, 63, 178],
        "blue" => [51, 76, 178],
        "brown" => [102, 76, 51],
        "green" => [102, 127, 51],
        "red" => [153, 51, 51],
        "black" => [25, 25, 25],
        _ => [255, 255, 255],
    }
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
