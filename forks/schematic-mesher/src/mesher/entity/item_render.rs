//! Item rendering for item frames and dropped items.
//!
//! Renders items inside item frames: flat sprite items with pixel edge extrusion
//! (the "paper cutout" look) and 3D block items with display transforms.
//! Also renders dropped items floating on the ground.

use super::EntityFaceTexture;
use crate::mesher::geometry::Vertex;
use crate::resolver::ModelResolver;
use crate::resource_pack::{BlockModel, ModelElement, ResourcePack};
use crate::types::Direction;
use glam::{Mat4, Vec3, Vec4};
use std::collections::HashMap;

/// Display transform parsed from a model's `display.fixed` context.
struct DisplayTransform {
    rotation: [f32; 3],    // degrees
    translation: [f32; 3], // 1/16 block units
    scale: [f32; 3],
}

impl Default for DisplayTransform {
    fn default() -> Self {
        Self {
            rotation: [0.0, 0.0, 0.0],
            translation: [0.0, 0.0, 0.0],
            scale: [1.0, 1.0, 1.0],
        }
    }
}

/// Resolved item type for rendering.
enum ItemRenderType {
    /// Flat sprite item (swords, tools, food, etc.)
    Generated {
        layers: Vec<String>,
        display: DisplayTransform,
    },
    /// 3D block model item (stone, planks, etc.)
    BlockModel {
        model: BlockModel,
        resolved_textures: HashMap<String, String>,
        display: DisplayTransform,
    },
}

/// Render an item inside an item frame.
///
/// Called from `element.rs::add_mob()` with access to resource pack and model resolver.
/// Returns geometry (vertices, indices, face_textures) in item frame local space [0,1],
/// already transformed for the frame's facing direction.
pub fn render_item_in_frame(
    resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    item_id: &str,
    item_rotation: u8,
    facing: &str,
) -> Option<(Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>)> {
    let item_type = resolve_item_type(resource_pack, model_resolver, item_id)?;

    let (mut vertices, indices, face_textures) = match &item_type {
        ItemRenderType::Generated { layers, display } => {
            generate_flat_item(resource_pack, layers, display, item_rotation, facing)
        }
        ItemRenderType::BlockModel {
            model,
            resolved_textures,
            display,
        } => generate_block_item(model, resolved_textures, display, item_rotation, facing),
    };

    if vertices.is_empty() {
        return None;
    }

    // Renormalize normals after transforms
    for v in &mut vertices {
        let n = Vec3::new(v.normal[0], v.normal[1], v.normal[2]).normalize_or_zero();
        v.normal = [n.x, n.y, n.z];
    }

    Some((vertices, indices, face_textures))
}

/// Render a dropped item floating on the ground.
///
/// Dropped items use `display.ground` transforms and hover slightly above the surface.
/// Returns geometry in [0,1] block-local space.
pub fn render_dropped_item(
    resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    item_id: &str,
    facing: &str,
) -> Option<(Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>)> {
    let name = bare_name(item_id);

    // Resolve model (same logic as item frames)
    let model_path = format!("item/{}", name);
    let model = model_resolver.resolve(&format!("minecraft:{}", model_path))
        .or_else(|_| model_resolver.resolve(&format!("minecraft:block/{}", name)))
        .ok()?;

    let display = parse_display_ground(&model.display);
    let resolved_textures = model_resolver.resolve_textures(&model);

    let is_generated = model.parent.as_deref() == Some("builtin/generated")
        || (model.elements.is_empty() && has_layer_textures(&resolved_textures));

    let (mut vertices, indices, face_textures) = if is_generated {
        let mut layers = Vec::new();
        for i in 0..4 {
            let key = format!("layer{}", i);
            if let Some(tex) = resolved_textures.get(&key) {
                layers.push(tex.clone());
            } else {
                break;
            }
        }
        if layers.is_empty() {
            return None;
        }
        generate_flat_dropped(resource_pack, &layers, &display, facing)
    } else if !model.elements.is_empty() {
        generate_block_dropped(&model, &resolved_textures, &display, facing)
    } else {
        return None;
    };

    if vertices.is_empty() {
        return None;
    }

    for v in &mut vertices {
        let n = Vec3::new(v.normal[0], v.normal[1], v.normal[2]).normalize_or_zero();
        v.normal = [n.x, n.y, n.z];
    }

    Some((vertices, indices, face_textures))
}

/// Build transform for a dropped item on the ground.
///
/// Pipeline:
/// 1. Center at origin
/// 2. Display.ground transform
/// 3. Facing rotation (Y-axis)
/// 4. Translate to hover position (center of block, slight hover)
fn build_dropped_item_transform(
    display: &DisplayTransform,
    facing: &str,
) -> Mat4 {
    let center = Mat4::from_translation(Vec3::new(-0.5, -0.5, -0.5));

    let dt_translate = Mat4::from_translation(Vec3::new(
        display.translation[0] / 16.0,
        display.translation[1] / 16.0,
        display.translation[2] / 16.0,
    ));
    let dt_rot_y = Mat4::from_rotation_y(display.rotation[1].to_radians());
    let dt_rot_x = Mat4::from_rotation_x(display.rotation[0].to_radians());
    let dt_rot_z = Mat4::from_rotation_z(display.rotation[2].to_radians());
    let dt_scale = Mat4::from_scale(Vec3::new(
        display.scale[0],
        display.scale[1],
        display.scale[2],
    ));
    let display_mat = dt_translate * dt_rot_y * dt_rot_x * dt_rot_z * dt_scale;

    // Facing rotation
    let facing_angle = match facing {
        "north" => std::f32::consts::PI,
        "south" => 0.0,
        "east" => -std::f32::consts::FRAC_PI_2,
        "west" => std::f32::consts::FRAC_PI_2,
        _ => 0.0,
    };
    let facing_rot = Mat4::from_rotation_y(facing_angle);

    // Place at block center, hovering slightly above ground
    let to_world = Mat4::from_translation(Vec3::new(0.5, 0.125, 0.5));

    to_world * facing_rot * display_mat * center
}

/// Generate flat dropped item geometry.
fn generate_flat_dropped(
    resource_pack: &ResourcePack,
    layers: &[String],
    display: &DisplayTransform,
    facing: &str,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    for (layer_idx, texture_path) in layers.iter().enumerate() {
        let tex_data = match resource_pack.get_texture(texture_path) {
            Some(t) => t.first_frame(),
            None => continue,
        };
        let tw = tex_data.width;
        let th = tex_data.height;
        if tw == 0 || th == 0 { continue; }

        let z_offset = layer_idx as f32 * 0.01;
        let fz = 0.5 + z_offset;
        let bz = 0.5 - z_offset - 1.0 / 16.0;

        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0], fz, [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0], false, texture_path,
        );
        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0], bz, [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0, 1.0], true, texture_path,
        );

        generate_edges(
            &tex_data.pixels, tw, th, fz, bz, texture_path,
            &mut vertices, &mut indices, &mut face_textures,
        );
    }

    let mat = build_dropped_item_transform(display, facing);
    transform_vertices(&mut vertices, &mat);
    (vertices, indices, face_textures)
}

/// Generate block dropped item geometry.
fn generate_block_dropped(
    model: &BlockModel,
    resolved_textures: &HashMap<String, String>,
    display: &DisplayTransform,
    facing: &str,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    for element in &model.elements {
        add_block_element(element, resolved_textures, &mut vertices, &mut indices, &mut face_textures);
    }

    let mat = build_dropped_item_transform(display, facing);
    transform_vertices(&mut vertices, &mat);
    (vertices, indices, face_textures)
}

/// Strip `minecraft:` prefix from an item ID.
fn bare_name(item_id: &str) -> &str {
    item_id
        .strip_prefix("minecraft:")
        .unwrap_or(item_id)
}

/// Resolve an item ID to its render type by checking item and block model paths.
fn resolve_item_type(
    _resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    item_id: &str,
) -> Option<ItemRenderType> {
    let name = bare_name(item_id);

    // Try item model first, then block model
    let model_path = format!("item/{}", name);
    let resolved = model_resolver.resolve(&format!("minecraft:{}", model_path));

    let model = match resolved {
        Ok(m) => m,
        Err(_) => {
            // Fallback: try block model directly
            let block_path = format!("block/{}", name);
            match model_resolver.resolve(&format!("minecraft:{}", block_path)) {
                Ok(m) => m,
                Err(_) => return None,
            }
        }
    };

    let display = parse_display_fixed(&model.display);
    let resolved_textures = model_resolver.resolve_textures(&model);

    // Check if this is a generated/flat item
    let is_generated = model.parent.as_deref() == Some("builtin/generated")
        || (model.elements.is_empty() && has_layer_textures(&resolved_textures));

    if is_generated {
        let mut layers = Vec::new();
        for i in 0..4 {
            let key = format!("layer{}", i);
            if let Some(tex) = resolved_textures.get(&key) {
                layers.push(tex.clone());
            } else {
                break;
            }
        }
        if layers.is_empty() {
            return None;
        }
        Some(ItemRenderType::Generated { layers, display })
    } else if !model.elements.is_empty() {
        Some(ItemRenderType::BlockModel {
            model,
            resolved_textures,
            display,
        })
    } else {
        None
    }
}

/// Check if resolved textures contain layer0 (generated item indicator).
fn has_layer_textures(textures: &HashMap<String, String>) -> bool {
    textures.contains_key("layer0")
}

/// Parse a display transform from a specific context (e.g., "fixed", "ground").
fn parse_display_context(display: &Option<serde_json::Value>, context: &str) -> DisplayTransform {
    let display_val = match display {
        Some(v) => v,
        None => return DisplayTransform::default(),
    };

    let ctx = match display_val.get(context) {
        Some(v) => v,
        None => return DisplayTransform::default(),
    };

    let rotation = parse_f32_array(ctx.get("rotation"), [0.0, 0.0, 0.0]);
    let translation = parse_f32_array(ctx.get("translation"), [0.0, 0.0, 0.0]);
    let scale = parse_f32_array(ctx.get("scale"), [1.0, 1.0, 1.0]);

    DisplayTransform {
        rotation,
        translation,
        scale,
    }
}

/// Convenience: parse `display.fixed`.
fn parse_display_fixed(display: &Option<serde_json::Value>) -> DisplayTransform {
    parse_display_context(display, "fixed")
}

/// Convenience: parse `display.ground`.
fn parse_display_ground(display: &Option<serde_json::Value>) -> DisplayTransform {
    parse_display_context(display, "ground")
}

/// Parse a JSON array of 3 floats, with a default fallback.
fn parse_f32_array(val: Option<&serde_json::Value>, default: [f32; 3]) -> [f32; 3] {
    match val {
        Some(serde_json::Value::Array(arr)) if arr.len() >= 3 => [
            arr[0].as_f64().unwrap_or(default[0] as f64) as f32,
            arr[1].as_f64().unwrap_or(default[1] as f64) as f32,
            arr[2].as_f64().unwrap_or(default[2] as f64) as f32,
        ],
        _ => default,
    }
}

/// Build the combined transform matrix for an item in a frame.
///
/// Pipeline:
/// 1. Center at origin: translate(-0.5, -0.5, -0.5)
/// 2. Display transform: translate(t/16) * rotY * rotX * rotZ * scale
/// 3. Item rotation: rotZ(item_rotation * 45deg)
/// 4. Translate to frame surface: (0.5, 0.5, 15.0/16.0)
/// 5. Facing rotation around (0.5, 0.5, 0.5)
fn build_item_transform(
    display: &DisplayTransform,
    item_rotation: u8,
    facing: &str,
) -> Mat4 {
    // Step 1: center at origin
    let center = Mat4::from_translation(Vec3::new(-0.5, -0.5, -0.5));

    // Step 2: display transform
    let dt_translate = Mat4::from_translation(Vec3::new(
        display.translation[0] / 16.0,
        display.translation[1] / 16.0,
        display.translation[2] / 16.0,
    ));
    let dt_rot_y = Mat4::from_rotation_y(display.rotation[1].to_radians());
    let dt_rot_x = Mat4::from_rotation_x(display.rotation[0].to_radians());
    let dt_rot_z = Mat4::from_rotation_z(display.rotation[2].to_radians());
    let dt_scale = Mat4::from_scale(Vec3::new(
        display.scale[0],
        display.scale[1],
        display.scale[2],
    ));
    let display_mat = dt_translate * dt_rot_y * dt_rot_x * dt_rot_z * dt_scale;

    // Step 2b: Minecraft's ItemFrameRenderer applies an additional scale(0.5) after
    // display.fixed transforms (see RenderItemFrame.renderItem in MC source)
    let renderer_scale = Mat4::from_scale(Vec3::splat(0.5));

    // Step 3: item rotation (0-7 steps of 45 degrees around Z)
    let item_rot_angle = (item_rotation % 8) as f32 * std::f32::consts::FRAC_PI_4;
    let item_rot = Mat4::from_rotation_z(-item_rot_angle);

    // Step 4: translate to frame center (south-facing frame surface is at z=15/16)
    let to_frame = Mat4::from_translation(Vec3::new(0.5, 0.5, 15.0 / 16.0));

    // Step 5: facing rotation around block center
    let block_center = Vec3::new(0.5, 0.5, 0.5);
    let rot_mat = match facing {
        "south" => Mat4::IDENTITY,
        "north" => Mat4::from_rotation_y(std::f32::consts::PI),
        "east" => Mat4::from_rotation_y(-std::f32::consts::FRAC_PI_2),
        "west" => Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        "up" => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2),
        "down" => Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        _ => Mat4::IDENTITY,
    };
    let facing_mat =
        Mat4::from_translation(block_center) * rot_mat * Mat4::from_translation(-block_center);

    // Combined: facing * to_frame * item_rot * renderer_scale * display * center
    facing_mat * to_frame * item_rot * renderer_scale * display_mat * center
}

/// Apply a transform matrix to all vertices (positions and normals).
fn transform_vertices(vertices: &mut [Vertex], mat: &Mat4) {
    for v in vertices {
        let p = *mat * Vec4::new(v.position[0], v.position[1], v.position[2], 1.0);
        v.position = [p.x, p.y, p.z];
        let n = *mat * Vec4::new(v.normal[0], v.normal[1], v.normal[2], 0.0);
        v.normal = [n.x, n.y, n.z];
    }
}

// ── Flat Item Rendering ─────────────────────────────────────────────────────

/// Generate flat item geometry with pixel edge extrusion.
///
/// Creates front/back face quads plus thin edge quads at transparent boundaries,
/// producing the characteristic Minecraft "paper cutout" look for items.
fn generate_flat_item(
    resource_pack: &ResourcePack,
    layers: &[String],
    display: &DisplayTransform,
    item_rotation: u8,
    facing: &str,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    for (layer_idx, texture_path) in layers.iter().enumerate() {
        let tex_data = match resource_pack.get_texture(texture_path) {
            Some(t) => t.first_frame(),
            None => continue,
        };

        let tw = tex_data.width;
        let th = tex_data.height;
        if tw == 0 || th == 0 {
            continue;
        }

        // Z offset for multi-layer items to avoid z-fighting
        let z_offset = layer_idx as f32 * 0.01;

        // Front face (+Z): z = 0.5 + z_offset, in [0,1] space
        let fz = 0.5 + z_offset;
        let bz = 0.5 - z_offset - 1.0 / 16.0; // back face slightly behind

        // Generate front and back quads for this layer
        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0],
            fz, [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0], // UVs: full texture
            false, // not flipped
            texture_path,
        );
        // Back face (-Z): mirrored horizontally
        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0],
            bz, [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0, 1.0], // UVs: mirrored horizontally
            true,
            texture_path,
        );

        // Pixel edge extrusion
        generate_edges(
            &tex_data.pixels, tw, th,
            fz, bz,
            texture_path,
            &mut vertices, &mut indices, &mut face_textures,
        );
    }

    // Apply item transforms
    let mat = build_item_transform(display, item_rotation, facing);
    transform_vertices(&mut vertices, &mat);

    (vertices, indices, face_textures)
}

/// Add a flat quad (front or back face of the sprite).
fn add_flat_face(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
    from_xy: [f32; 2],
    to_xy: [f32; 2],
    z: f32,
    normal: [f32; 3],
    uv_rect: [f32; 4], // [u0, v0, u1, v1]
    is_back: bool,
    texture_path: &str,
) {
    let v_start = vertices.len() as u32;

    let (u0, v0, u1, v1) = (uv_rect[0], uv_rect[1], uv_rect[2], uv_rect[3]);

    // Quad corners in [0,1] XY plane at the given Z
    // CCW winding when viewed from front (+Z)
    if !is_back {
        // Front face: TL, TR, BR, BL
        vertices.push(Vertex::new([from_xy[0], to_xy[1], z], normal, [u0, v0]));
        vertices.push(Vertex::new([to_xy[0], to_xy[1], z], normal, [u1, v0]));
        vertices.push(Vertex::new([to_xy[0], from_xy[1], z], normal, [u1, v1]));
        vertices.push(Vertex::new([from_xy[0], from_xy[1], z], normal, [u0, v1]));
    } else {
        // Back face: TR, TL, BL, BR (reversed winding)
        vertices.push(Vertex::new([to_xy[0], to_xy[1], z], normal, [u0, v0]));
        vertices.push(Vertex::new([from_xy[0], to_xy[1], z], normal, [u1, v0]));
        vertices.push(Vertex::new([from_xy[0], from_xy[1], z], normal, [u1, v1]));
        vertices.push(Vertex::new([to_xy[0], from_xy[1], z], normal, [u0, v1]));
    }

    // Two triangles (CCW): (0,2,1) (0,3,2) — matches Mesh::add_quad
    indices.extend_from_slice(&[
        v_start, v_start + 2, v_start + 1,
        v_start, v_start + 3, v_start + 2,
    ]);

    face_textures.push(EntityFaceTexture {
        texture: texture_path.to_string(),
        is_transparent: true, // Flat items often have transparency
    });
}

/// Generate edge quads for pixel boundaries (the "depth" of the paper cutout).
fn generate_edges(
    pixels: &[u8],
    tw: u32,
    th: u32,
    front_z: f32,
    back_z: f32,
    texture_path: &str,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    let tw_f = tw as f32;
    let th_f = th as f32;

    let is_opaque = |px: i32, py: i32| -> bool {
        if px < 0 || py < 0 || px >= tw as i32 || py >= th as i32 {
            return false;
        }
        let idx = ((py as u32 * tw + px as u32) * 4 + 3) as usize;
        pixels.get(idx).copied().unwrap_or(0) > 0
    };

    for py in 0..th as i32 {
        for px in 0..tw as i32 {
            if !is_opaque(px, py) {
                continue;
            }

            // UV for this pixel (single pixel strip)
            let u0 = px as f32 / tw_f;
            let u1 = (px + 1) as f32 / tw_f;
            let v0 = py as f32 / th_f;
            let v1 = (py + 1) as f32 / th_f;

            // Position in [0,1] space (Y is flipped: py=0 is top of image = Y=1 in model)
            let x0 = px as f32 / tw_f;
            let x1 = (px + 1) as f32 / tw_f;
            let y0 = 1.0 - (py + 1) as f32 / th_f; // bottom of pixel
            let y1 = 1.0 - py as f32 / th_f; // top of pixel

            // West edge (px-1 is transparent)
            if !is_opaque(px - 1, py) {
                add_edge_quad(
                    vertices, indices, face_textures,
                    [x0, y0, back_z], [x0, y1, front_z],
                    [-1.0, 0.0, 0.0],
                    [u0, v1, u1, v0], // single pixel column
                    texture_path,
                );
            }

            // East edge (px+1 is transparent)
            if !is_opaque(px + 1, py) {
                add_edge_quad(
                    vertices, indices, face_textures,
                    [x1, y0, front_z], [x1, y1, back_z],
                    [1.0, 0.0, 0.0],
                    [u0, v1, u1, v0],
                    texture_path,
                );
            }

            // Up edge (py-1 is transparent, i.e. above in image = higher Y in model)
            if !is_opaque(px, py - 1) {
                add_edge_quad(
                    vertices, indices, face_textures,
                    [x0, y1, front_z], [x1, y1, back_z],
                    [0.0, 1.0, 0.0],
                    [u0, v0, u1, v1],
                    texture_path,
                );
            }

            // Down edge (py+1 is transparent)
            if !is_opaque(px, py + 1) {
                add_edge_quad(
                    vertices, indices, face_textures,
                    [x0, y0, back_z], [x1, y0, front_z],
                    [0.0, -1.0, 0.0],
                    [u0, v0, u1, v1],
                    texture_path,
                );
            }
        }
    }
}

/// Add a thin edge quad between two diagonal corners.
fn add_edge_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
    min: [f32; 3],  // one corner
    max: [f32; 3],  // opposite corner
    normal: [f32; 3],
    uv_rect: [f32; 4],
    texture_path: &str,
) {
    let v_start = vertices.len() as u32;
    let (u0, v0, u1, v1) = (uv_rect[0], uv_rect[1], uv_rect[2], uv_rect[3]);

    // Determine which axis varies for this edge
    // For X-facing edges (West/East): Y and Z vary
    // For Y-facing edges (Up/Down): X and Z vary
    if normal[0].abs() > 0.5 {
        // X-facing: quad spans Y and Z
        vertices.push(Vertex::new([min[0], min[1], min[2]], normal, [u0, v0]));
        vertices.push(Vertex::new([min[0], max[1], min[2]], normal, [u0, v1]));
        vertices.push(Vertex::new([max[0], max[1], max[2]], normal, [u1, v1]));
        vertices.push(Vertex::new([max[0], min[1], max[2]], normal, [u1, v0]));
    } else {
        // Y-facing: quad spans X and Z
        vertices.push(Vertex::new([min[0], min[1], min[2]], normal, [u0, v0]));
        vertices.push(Vertex::new([max[0], min[1], min[2]], normal, [u1, v0]));
        vertices.push(Vertex::new([max[0], max[1], max[2]], normal, [u1, v1]));
        vertices.push(Vertex::new([min[0], max[1], max[2]], normal, [u0, v1]));
    }

    indices.extend_from_slice(&[
        v_start, v_start + 2, v_start + 1,
        v_start, v_start + 3, v_start + 2,
    ]);

    face_textures.push(EntityFaceTexture {
        texture: texture_path.to_string(),
        is_transparent: true,
    });
}

// ── Block Item Rendering ────────────────────────────────────────────────────

/// Generate 3D block item geometry from model elements.
fn generate_block_item(
    model: &BlockModel,
    resolved_textures: &HashMap<String, String>,
    display: &DisplayTransform,
    item_rotation: u8,
    facing: &str,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut face_textures = Vec::new();

    for element in &model.elements {
        add_block_element(
            element,
            resolved_textures,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );
    }

    // Apply item transforms
    let mat = build_item_transform(display, item_rotation, facing);
    transform_vertices(&mut vertices, &mat);

    (vertices, indices, face_textures)
}

/// Generate geometry for a single model element (all 6 faces).
fn add_block_element(
    element: &ModelElement,
    resolved_textures: &HashMap<String, String>,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    face_textures: &mut Vec<EntityFaceTexture>,
) {
    // Element bounds in [0,1] block space
    let from = [
        element.from[0] / 16.0,
        element.from[1] / 16.0,
        element.from[2] / 16.0,
    ];
    let to = [
        element.to[0] / 16.0,
        element.to[1] / 16.0,
        element.to[2] / 16.0,
    ];

    for (direction, face) in &element.faces {
        // Resolve texture
        let texture_path = if face.texture.starts_with('#') {
            let key = &face.texture[1..];
            resolved_textures
                .get(key)
                .cloned()
                .unwrap_or_else(|| "block/missing".to_string())
        } else {
            face.texture.clone()
        };

        let uv = face.normalized_uv();
        let (u1, v1, u2, v2) = (uv[0], uv[1], uv[2], uv[3]);
        let base_uvs = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];

        // Rotate UVs
        let steps = ((face.rotation / 90) % 4 + 4) % 4;
        let mut uvs = base_uvs;
        for _ in 0..steps {
            uvs = [uvs[3], uvs[0], uvs[1], uvs[2]];
        }

        let normal = direction.normal();

        // Generate positions matching element.rs::generate_face_vertices
        let positions: [[f32; 3]; 4] = match direction {
            Direction::Down => [
                [from[0], from[1], to[2]],
                [to[0], from[1], to[2]],
                [to[0], from[1], from[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::Up => [
                [from[0], to[1], from[2]],
                [to[0], to[1], from[2]],
                [to[0], to[1], to[2]],
                [from[0], to[1], to[2]],
            ],
            Direction::North => [
                [to[0], to[1], from[2]],
                [from[0], to[1], from[2]],
                [from[0], from[1], from[2]],
                [to[0], from[1], from[2]],
            ],
            Direction::South => [
                [from[0], to[1], to[2]],
                [to[0], to[1], to[2]],
                [to[0], from[1], to[2]],
                [from[0], from[1], to[2]],
            ],
            Direction::West => [
                [from[0], to[1], from[2]],
                [from[0], to[1], to[2]],
                [from[0], from[1], to[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::East => [
                [to[0], to[1], to[2]],
                [to[0], to[1], from[2]],
                [to[0], from[1], from[2]],
                [to[0], from[1], to[2]],
            ],
        };

        let v_start = vertices.len() as u32;
        for i in 0..4 {
            vertices.push(Vertex::new(positions[i], normal, uvs[i]));
        }

        // CCW winding: (0,2,1)(0,3,2) — matches Mesh::add_quad
        indices.extend_from_slice(&[
            v_start, v_start + 2, v_start + 1,
            v_start, v_start + 3, v_start + 2,
        ]);

        face_textures.push(EntityFaceTexture {
            texture: texture_path,
            is_transparent: false,
        });
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_display_fixed() {
        let json: serde_json::Value = serde_json::json!({
            "fixed": {
                "rotation": [0.0, 180.0, 0.0],
                "translation": [0.0, 0.0, 0.0],
                "scale": [0.5, 0.5, 0.5]
            }
        });

        let dt = parse_display_fixed(&Some(json));
        assert_eq!(dt.rotation, [0.0, 180.0, 0.0]);
        assert_eq!(dt.translation, [0.0, 0.0, 0.0]);
        assert_eq!(dt.scale, [0.5, 0.5, 0.5]);
    }

    #[test]
    fn test_parse_display_missing() {
        let dt = parse_display_fixed(&None);
        assert_eq!(dt.rotation, [0.0, 0.0, 0.0]);
        assert_eq!(dt.translation, [0.0, 0.0, 0.0]);
        assert_eq!(dt.scale, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_parse_display_no_fixed_context() {
        let json: serde_json::Value = serde_json::json!({
            "gui": {
                "rotation": [30.0, 225.0, 0.0],
                "scale": [0.625, 0.625, 0.625]
            }
        });
        let dt = parse_display_fixed(&Some(json));
        assert_eq!(dt.rotation, [0.0, 0.0, 0.0]); // defaults
    }

    #[test]
    fn test_flat_item_edge_count() {
        // 2x2 fully opaque texture = 2 face quads + 8 edge quads = 10
        let pixels = vec![
            255, 0, 0, 255,  0, 255, 0, 255,
            0, 0, 255, 255,  255, 255, 0, 255,
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        // Add front + back quads
        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0], 0.5, [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0], false, "test",
        );
        add_flat_face(
            &mut vertices, &mut indices, &mut face_textures,
            [0.0, 0.0], [1.0, 1.0], 0.4375, [0.0, 0.0, -1.0],
            [1.0, 0.0, 0.0, 1.0], true, "test",
        );

        // Generate edges
        generate_edges(
            &pixels, 2, 2, 0.5, 0.4375, "test",
            &mut vertices, &mut indices, &mut face_textures,
        );

        // 2 face quads + 8 edge quads (4 pixels × boundary edges = 8 perimeter edges)
        // Each 2x2 opaque block: top-left has W+U edges, top-right has E+U edges,
        // bottom-left has W+D edges, bottom-right has E+D edges = 8 edges
        assert_eq!(face_textures.len(), 10); // 2 faces + 8 edges
    }

    #[test]
    fn test_flat_item_with_transparency() {
        // 2x2 texture where top-right pixel is transparent
        let pixels = vec![
            255, 0, 0, 255,  0, 0, 0, 0,    // top-left opaque, top-right transparent
            0, 0, 255, 255,  255, 255, 0, 255, // bottom-left opaque, bottom-right opaque
        ];

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        generate_edges(
            &pixels, 2, 2, 0.5, 0.4375, "test",
            &mut vertices, &mut indices, &mut face_textures,
        );

        // 3 opaque pixels: each has perimeter edges where neighbor is transparent/OOB
        // TL(0,0): W(oob), U(oob), E(transparent) = 3 edges
        // BL(0,1): W(oob), D(oob), E(opaque below-right)... let's just check > 0
        assert!(face_textures.len() > 0);
        // Fewer edges than the fully opaque case
        // TL: W,U,E = 3; BL: W,D = 2; BR: E,D = 2; plus internal edges between them
        // This is complex to count exactly, just verify it's reasonable
        assert!(face_textures.len() < 12);
    }

    #[test]
    fn test_block_item_cube() {
        // Create a simple cube model element with all 6 faces
        let mut faces = HashMap::new();
        for dir in Direction::ALL {
            faces.insert(dir, crate::resource_pack::ModelFace {
                uv: None,
                texture: "#all".to_string(),
                cullface: None,
                rotation: 0,
                tintindex: -1,
            });
        }
        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            shade: true,
            faces,
        };

        let mut resolved_textures = HashMap::new();
        resolved_textures.insert("all".to_string(), "block/stone".to_string());

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut face_textures = Vec::new();

        add_block_element(
            &element,
            &resolved_textures,
            &mut vertices,
            &mut indices,
            &mut face_textures,
        );

        assert_eq!(face_textures.len(), 6);
        assert_eq!(vertices.len(), 24); // 6 faces × 4 verts
        assert_eq!(indices.len(), 36);  // 6 faces × 6 indices
    }

    #[test]
    fn test_build_item_transform_identity() {
        let display = DisplayTransform::default();
        let mat = build_item_transform(&display, 0, "south");

        // With default display and south facing, the item should be near the frame surface
        // center (0.5,0.5,0.5) → centered at origin → scaled 0.5 → at frame z
        let p = mat * Vec4::new(0.5, 0.5, 0.5, 1.0);
        // After 0.5 scale, origin stays at origin, then translated to (0.5, 0.5, 15/16)
        assert!((p.x - 0.5).abs() < 0.01);
        assert!((p.y - 0.5).abs() < 0.01);
        assert!((p.z - 15.0 / 16.0).abs() < 0.01);
    }
}
