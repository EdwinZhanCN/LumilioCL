//! glTF/GLB export with KHR_mesh_quantization.
//!
//! Vertex data is quantized for significant size reduction:
//! - Positions: f32 → i16 (normalized via node transform)
//! - Normals: f32 → i8 (normalized, axis-aligned → exact)
//! - UVs: kept as f32 (greedy UVs can exceed 0-1 range)
//! - Colors: f32 → u8 (normalized, 256 levels sufficient)
//! - Indices: u32 → u16 when vertex_count < 65536

use crate::error::{MesherError, Result};
use crate::mesher::geometry::Mesh;
use crate::mesher::MesherOutput;
use gltf_json as json;
use json::validation::Checked::Valid;
use json::validation::USize64;
use std::mem;

fn quantize_position(pos: [f32; 3], center: &[f32; 3], half_ext: &[f32; 3]) -> [i16; 3] {
    let mut q = [0i16; 3];
    for i in 0..3 {
        let normalized = (pos[i] - center[i]) / half_ext[i] * 32767.0;
        q[i] = normalized.round().clamp(-32767.0, 32767.0) as i16;
    }
    q
}

fn quantize_normal(n: [f32; 3]) -> [i8; 3] {
    [
        (n[0] * 127.0).round().clamp(-127.0, 127.0) as i8,
        (n[1] * 127.0).round().clamp(-127.0, 127.0) as i8,
        (n[2] * 127.0).round().clamp(-127.0, 127.0) as i8,
    ]
}

fn quantize_color(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0] * 255.0).round().clamp(0.0, 255.0) as u8,
        (c[1] * 255.0).round().clamp(0.0, 255.0) as u8,
        (c[2] * 255.0).round().clamp(0.0, 255.0) as u8,
        (c[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

/// Pad buffer to the given alignment boundary.
fn align_buffer(buffer: &mut Vec<u8>, alignment: usize) {
    let padding = (alignment - (buffer.len() % alignment)) % alignment;
    buffer.extend(std::iter::repeat(0u8).take(padding));
}

/// Export a mesh to GLB format (binary glTF) with embedded texture.
/// Uses KHR_mesh_quantization for ~56% vertex data reduction.
/// Separates opaque and transparent geometry into different primitives for correct rendering.
/// Greedy-merged materials get their own textures with REPEAT wrapping for proper tiling.
pub fn export_glb(output: &MesherOutput) -> Result<Vec<u8>> {
    // Main layers are SoA (MeshLayer) and the writer reads SoA directly — no AoS
    // conversion. Greedy materials are still AoS Mesh (converted at their call).
    let opaque_mesh = &output.opaque_mesh;
    let cutout_mesh = &output.cutout_mesh;
    let transparent_mesh = &output.transparent_mesh;
    let atlas = &output.atlas;
    // Check if all meshes are empty
    let has_greedy = output.greedy_materials.iter().any(|gm| {
        !gm.opaque_mesh.is_empty() || !gm.transparent_mesh.is_empty()
    });
    if opaque_mesh.is_empty() && cutout_mesh.is_empty() && transparent_mesh.is_empty() && !has_greedy {
        return Err(MesherError::Export("Cannot export empty mesh".to_string()));
    }

    // Get texture PNG data
    let texture_png = atlas.to_png()?;

    // Calculate combined bounding box for position quantization
    let (bounds_min, bounds_max) = calculate_bounds_all(output);

    // Compute quantization parameters: center and half_extent per axis
    let mut center = [0.0f32; 3];
    let mut half_ext = [0.0f32; 3];
    for i in 0..3 {
        center[i] = (bounds_max[i] + bounds_min[i]) / 2.0;
        half_ext[i] = (bounds_max[i] - bounds_min[i]) / 2.0;
        if half_ext[i] < 1e-6 {
            half_ext[i] = 1e-6; // avoid div-by-zero
        }
    }

    // Build the binary buffer incrementally
    let mut buffer_data: Vec<u8> = Vec::new();

    // Track mesh data offsets
    struct MeshOffsets {
        pos_offset: usize,
        pos_bytes: usize,
        norm_offset: usize,
        norm_bytes: usize,
        uv_offset: usize,
        uv_bytes: usize,
        color_offset: usize,
        color_bytes: usize,
        idx_offset: usize,
        idx_bytes: usize,
        vertex_count: usize,
        index_count: usize,
        pos_min: [i16; 3],
        pos_max: [i16; 3],
        use_u16_indices: bool,
    }

    fn write_mesh(
        buffer: &mut Vec<u8>,
        layer: &crate::mesh_output::MeshLayer,
        center: &[f32; 3],
        half_ext: &[f32; 3],
    ) -> Option<MeshOffsets> {
        if layer.is_empty() {
            return None;
        }

        let vertex_count = layer.vertex_count();
        let use_u16_indices = vertex_count <= 65535;

        // Positions: i16 × 3 (2 bytes each = 6 bytes/vertex)
        // Align to 4 bytes (glTF requires 4-byte aligned vertex attributes)
        align_buffer(buffer, 4);
        let pos_offset = buffer.len();
        let mut pos_min = [i16::MAX; 3];
        let mut pos_max = [i16::MIN; 3];
        for position in &layer.positions {
            let q = quantize_position(*position, center, half_ext);
            for i in 0..3 {
                pos_min[i] = pos_min[i].min(q[i]);
                pos_max[i] = pos_max[i].max(q[i]);
            }
            buffer.extend_from_slice(bytemuck_cast_slice(&q));
        }

        // Normals: i8 × 3 (1 byte each = 3 bytes/vertex)
        // Align to 4 bytes (glTF requires 4-byte aligned vertex attributes)
        align_buffer(buffer, 4);
        let norm_offset = buffer.len();
        for normal in &layer.normals {
            let q = quantize_normal(*normal);
            buffer.extend_from_slice(bytemuck_cast_slice(&q));
        }

        // UVs: f32 × 2 (4 bytes each = 8 bytes/vertex)
        // Align to 4 bytes (f32 alignment)
        align_buffer(buffer, 4);
        let uv_offset = buffer.len();
        for uv in &layer.uvs {
            buffer.extend_from_slice(bytemuck_cast_slice(uv));
        }

        // Colors: u8 × 4 (1 byte each = 4 bytes/vertex)
        // Align to 4 bytes (glTF requires 4-byte aligned vertex attributes)
        align_buffer(buffer, 4);
        let color_offset = buffer.len();
        for color in &layer.colors {
            let q = quantize_color(*color);
            buffer.extend_from_slice(&q);
        }

        // Indices: u16 or u32
        if use_u16_indices {
            // Align to 2 bytes (u16 alignment)
            align_buffer(buffer, 2);
        } else {
            // Align to 4 bytes (u32 alignment)
            align_buffer(buffer, 4);
        }
        let idx_offset = buffer.len();
        if use_u16_indices {
            for &idx in &layer.indices {
                buffer.extend_from_slice(&(idx as u16).to_le_bytes());
            }
        } else {
            buffer.extend_from_slice(bytemuck_cast_slice(&layer.indices));
        }
        let end = buffer.len();

        Some(MeshOffsets {
            pos_offset,
            pos_bytes: norm_offset - pos_offset,
            norm_offset,
            norm_bytes: uv_offset - norm_offset,
            uv_offset,
            uv_bytes: color_offset - uv_offset,
            color_offset,
            color_bytes: idx_offset - color_offset,
            idx_offset,
            idx_bytes: end - idx_offset,
            vertex_count,
            index_count: layer.indices.len(),
            pos_min,
            pos_max,
            use_u16_indices,
        })
    }

    let opaque_offsets = write_mesh(&mut buffer_data, opaque_mesh, &center, &half_ext);
    let cutout_offsets = write_mesh(&mut buffer_data, cutout_mesh, &center, &half_ext);
    let transparent_offsets = write_mesh(&mut buffer_data, transparent_mesh, &center, &half_ext);

    // Write greedy material mesh data
    let mut greedy_mesh_offsets: Vec<(Option<MeshOffsets>, Option<MeshOffsets>)> = Vec::new();
    for gm in &output.greedy_materials {
        // Greedy materials are AoS Mesh; convert to SoA for the writer (rare/small).
        let opaque = write_mesh(
            &mut buffer_data,
            &crate::mesh_output::mesh_to_layer(&gm.opaque_mesh),
            &center,
            &half_ext,
        );
        let transparent = write_mesh(
            &mut buffer_data,
            &crate::mesh_output::mesh_to_layer(&gm.transparent_mesh),
            &center,
            &half_ext,
        );
        greedy_mesh_offsets.push((opaque, transparent));
    }

    // Append atlas texture PNG (aligned to 4 bytes)
    align_buffer(&mut buffer_data, 4);
    let atlas_texture_offset = buffer_data.len();
    buffer_data.extend_from_slice(&texture_png);

    // Append greedy texture PNGs (aligned to 4 bytes)
    let mut greedy_texture_offsets: Vec<(usize, usize)> = Vec::new();
    for gm in &output.greedy_materials {
        if gm.texture_png.is_empty() {
            greedy_texture_offsets.push((0, 0));
            continue;
        }
        align_buffer(&mut buffer_data, 4);
        let offset = buffer_data.len();
        buffer_data.extend_from_slice(&gm.texture_png);
        greedy_texture_offsets.push((offset, gm.texture_png.len()));
    }

    // Append animated texture sprite sheet PNGs (aligned to 4 bytes)
    let mut anim_texture_offsets: Vec<(usize, usize)> = Vec::new();
    for at in &output.animated_textures {
        align_buffer(&mut buffer_data, 4);
        let offset = buffer_data.len();
        buffer_data.extend_from_slice(&at.sprite_sheet_png);
        anim_texture_offsets.push((offset, at.sprite_sheet_png.len()));
    }

    let total_buffer_size = buffer_data.len();

    // Build glTF arrays
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut primitives = Vec::new();
    let mut images = Vec::new();
    let mut textures = Vec::new();
    let mut materials = Vec::new();

    let mut buffer_view_idx = 0u32;

    // Helper: add buffer views, accessors, and primitive for a mesh
    fn add_mesh_primitive(
        offsets: &MeshOffsets,
        material_idx: u32,
        buffer_views: &mut Vec<json::buffer::View>,
        accessors: &mut Vec<json::Accessor>,
        primitives: &mut Vec<json::mesh::Primitive>,
        buffer_view_idx: &mut u32,
    ) {
        let accessor_start = accessors.len() as u32;

        // 5 buffer views: positions, normals, uvs, colors, indices
        buffer_views.push(create_buffer_view(offsets.pos_offset, offsets.pos_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let pos_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.norm_offset, offsets.norm_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let norm_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.uv_offset, offsets.uv_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let uv_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.color_offset, offsets.color_bytes, Some(json::buffer::Target::ArrayBuffer)));
        let color_view = *buffer_view_idx; *buffer_view_idx += 1;

        buffer_views.push(create_buffer_view(offsets.idx_offset, offsets.idx_bytes, Some(json::buffer::Target::ElementArrayBuffer)));
        let idx_view = *buffer_view_idx; *buffer_view_idx += 1;

        // Position accessor: i16, not normalized, with integer min/max
        accessors.push(create_accessor(
            pos_view,
            offsets.vertex_count,
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::I16,
            false,
            Some(json::Value::from(vec![
                offsets.pos_min[0] as i64,
                offsets.pos_min[1] as i64,
                offsets.pos_min[2] as i64,
            ])),
            Some(json::Value::from(vec![
                offsets.pos_max[0] as i64,
                offsets.pos_max[1] as i64,
                offsets.pos_max[2] as i64,
            ])),
        ));

        // Normal accessor: i8, normalized
        accessors.push(create_accessor(
            norm_view,
            offsets.vertex_count,
            json::accessor::Type::Vec3,
            json::accessor::ComponentType::I8,
            true,
            None,
            None,
        ));

        // UV accessor: f32, not normalized
        accessors.push(create_accessor(
            uv_view,
            offsets.vertex_count,
            json::accessor::Type::Vec2,
            json::accessor::ComponentType::F32,
            false,
            None,
            None,
        ));

        // Color accessor: u8, normalized
        accessors.push(create_accessor(
            color_view,
            offsets.vertex_count,
            json::accessor::Type::Vec4,
            json::accessor::ComponentType::U8,
            true,
            None,
            None,
        ));

        // Index accessor
        let idx_component_type = if offsets.use_u16_indices {
            json::accessor::ComponentType::U16
        } else {
            json::accessor::ComponentType::U32
        };
        accessors.push(create_accessor(
            idx_view,
            offsets.index_count,
            json::accessor::Type::Scalar,
            idx_component_type,
            false,
            None,
            None,
        ));

        primitives.push(create_primitive(accessor_start, accessor_start + 4, material_idx));
    }

    // Material 0: Atlas opaque
    materials.push(create_material_with_texture(json::material::AlphaMode::Opaque, 0));
    // Material 1: Atlas cutout (alpha-tested, writes depth — fire, flowers, leaves)
    materials.push(create_material_with_alpha_cutoff(0, 0.5));
    // Material 2: Atlas transparent (alpha-blended — water, ice, stained glass)
    materials.push(create_material_with_texture(json::material::AlphaMode::Blend, 0));

    // Atlas texture image and glTF texture
    buffer_views.push(json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(texture_png.len() as u64),
        byte_offset: Some(USize64(atlas_texture_offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: None,
    });
    let atlas_image_view = buffer_view_idx;
    buffer_view_idx += 1;

    images.push(json::Image {
        buffer_view: Some(json::Index::new(atlas_image_view)),
        mime_type: Some(json::image::MimeType("image/png".to_string())),
        uri: None,
        extensions: Default::default(),
        extras: Default::default(),
    });
    textures.push(json::Texture {
        sampler: Some(json::Index::new(0)),
        source: json::Index::new(0),
        extensions: Default::default(),
        extras: Default::default(),
    });

    // Add atlas-based primitives
    if let Some(ref offsets) = opaque_offsets {
        add_mesh_primitive(offsets, 0, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
    }
    if let Some(ref offsets) = cutout_offsets {
        add_mesh_primitive(offsets, 1, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
    }
    if let Some(ref offsets) = transparent_offsets {
        add_mesh_primitive(offsets, 2, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
    }

    // Add greedy material images, textures, materials, and primitives
    for (i, _gm) in output.greedy_materials.iter().enumerate() {
        let (tex_offset, tex_len) = greedy_texture_offsets[i];
        if tex_len == 0 {
            continue;
        }

        // Add image buffer view
        buffer_views.push(json::buffer::View {
            buffer: json::Index::new(0),
            byte_length: USize64(tex_len as u64),
            byte_offset: Some(USize64(tex_offset as u64)),
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            target: None,
        });
        let img_view = buffer_view_idx;
        buffer_view_idx += 1;

        let image_idx = images.len() as u32;
        images.push(json::Image {
            buffer_view: Some(json::Index::new(img_view)),
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        });

        let texture_idx = textures.len() as u32;
        textures.push(json::Texture {
            sampler: Some(json::Index::new(0)), // Reuse sampler 0 (REPEAT)
            source: json::Index::new(image_idx),
            extensions: Default::default(),
            extras: Default::default(),
        });

        // Opaque material for this greedy texture
        let opaque_mat_idx = materials.len() as u32;
        materials.push(create_material_with_texture(json::material::AlphaMode::Opaque, texture_idx));

        // Transparent material for this greedy texture
        let transparent_mat_idx = materials.len() as u32;
        materials.push(create_material_with_texture(json::material::AlphaMode::Blend, texture_idx));

        let (ref opaque_off, ref transparent_off) = greedy_mesh_offsets[i];

        if let Some(ref offsets) = opaque_off {
            add_mesh_primitive(offsets, opaque_mat_idx, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
        }
        if let Some(ref offsets) = transparent_off {
            add_mesh_primitive(offsets, transparent_mat_idx, &mut buffer_views, &mut accessors, &mut primitives, &mut buffer_view_idx);
        }
    }

    // Add animated texture sprite sheet images and build scene extras
    let mut anim_texture_indices: Vec<u32> = Vec::new();
    for (i, _at) in output.animated_textures.iter().enumerate() {
        let (offset, len) = anim_texture_offsets[i];
        if len == 0 {
            continue;
        }

        buffer_views.push(json::buffer::View {
            buffer: json::Index::new(0),
            byte_length: USize64(len as u64),
            byte_offset: Some(USize64(offset as u64)),
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            target: None,
        });
        let img_view = buffer_view_idx;
        buffer_view_idx += 1;

        let image_idx = images.len() as u32;
        images.push(json::Image {
            buffer_view: Some(json::Index::new(img_view)),
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        });

        anim_texture_indices.push(image_idx);
    }

    // Build scene extras with animation metadata
    let scene_extras: json::Extras = if !output.animated_textures.is_empty() {
        let anim_entries: Vec<serde_json::Value> = output.animated_textures.iter().enumerate().map(|(i, at)| {
            let mut entry = serde_json::Map::new();
            entry.insert("imageIndex".into(), serde_json::Value::from(anim_texture_indices[i] as u64));
            entry.insert("frameCount".into(), serde_json::Value::from(at.frame_count as u64));
            entry.insert("frametime".into(), serde_json::Value::from(at.frametime as u64));
            entry.insert("interpolate".into(), serde_json::Value::from(at.interpolate));
            entry.insert("frameWidth".into(), serde_json::Value::from(at.frame_width as u64));
            entry.insert("frameHeight".into(), serde_json::Value::from(at.frame_height as u64));
            entry.insert("atlasX".into(), serde_json::Value::from(at.atlas_x as u64));
            entry.insert("atlasY".into(), serde_json::Value::from(at.atlas_y as u64));
            if let Some(ref frames) = at.frames {
                let frame_values: Vec<serde_json::Value> = frames.iter().map(|&f| serde_json::Value::from(f as u64)).collect();
                entry.insert("frames".into(), serde_json::Value::from(frame_values));
            }
            serde_json::Value::from(entry)
        }).collect();
        let mut extras_obj = serde_json::Map::new();
        extras_obj.insert("animatedTextures".into(), serde_json::Value::from(anim_entries));
        let json_str = serde_json::to_string(&extras_obj).unwrap_or_default();
        serde_json::value::RawValue::from_string(json_str).ok()
    } else {
        None
    };

    // Build glTF JSON with KHR_mesh_quantization extension
    let root = json::Root {
        accessors,
        buffers: vec![json::Buffer {
            byte_length: USize64(total_buffer_size as u64),
            extensions: Default::default(),
            extras: Default::default(),
            uri: None,
        }],
        buffer_views,
        images,
        samplers: vec![json::texture::Sampler {
            mag_filter: Some(Valid(json::texture::MagFilter::Nearest)),
            min_filter: Some(Valid(json::texture::MinFilter::Nearest)),
            wrap_s: Valid(json::texture::WrappingMode::Repeat),
            wrap_t: Valid(json::texture::WrappingMode::Repeat),
            extensions: Default::default(),
            extras: Default::default(),
        }],
        textures,
        materials,
        meshes: vec![json::Mesh {
            extensions: Default::default(),
            extras: Default::default(),
            primitives,
            weights: None,
        }],
        nodes: vec![json::Node {
            camera: None,
            children: None,
            extensions: Default::default(),
            extras: Default::default(),
            matrix: None,
            mesh: Some(json::Index::new(0)),
            rotation: None,
            scale: Some([
                half_ext[0] / 32767.0,
                half_ext[1] / 32767.0,
                half_ext[2] / 32767.0,
            ]),
            translation: Some(center),
            skin: None,
            weights: None,
        }],
        scenes: vec![json::Scene {
            extensions: Default::default(),
            extras: scene_extras,
            nodes: vec![json::Index::new(0)],
        }],
        scene: Some(json::Index::new(0)),
        extensions_used: vec!["KHR_mesh_quantization".to_string()],
        extensions_required: vec!["KHR_mesh_quantization".to_string()],
        ..Default::default()
    };

    // Serialize JSON
    let json_string = json::serialize::to_string(&root)
        .map_err(|e| MesherError::Export(format!("Failed to serialize glTF JSON: {}", e)))?;
    let json_bytes = json_string.as_bytes();

    // Pad JSON to 4-byte alignment
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let padded_json_len = json_bytes.len() + json_padding;

    // Pad buffer to 4-byte alignment
    let buffer_padding = (4 - (buffer_data.len() % 4)) % 4;
    let padded_buffer_len = buffer_data.len() + buffer_padding;

    // Calculate total size
    let total_size = 12 + // GLB header
        8 + padded_json_len + // JSON chunk
        8 + padded_buffer_len; // BIN chunk

    let mut glb = Vec::with_capacity(total_size);

    // GLB Header
    glb.extend_from_slice(b"glTF"); // magic
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
    glb.extend_from_slice(&(total_size as u32).to_le_bytes()); // length

    // JSON Chunk
    glb.extend_from_slice(&(padded_json_len as u32).to_le_bytes()); // chunk length
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // chunk type: JSON
    glb.extend_from_slice(json_bytes);
    glb.extend_from_slice(&vec![0x20u8; json_padding]); // padding (spaces)

    // BIN Chunk
    glb.extend_from_slice(&(padded_buffer_len as u32).to_le_bytes()); // chunk length
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // chunk type: BIN
    glb.extend_from_slice(&buffer_data);
    glb.extend_from_slice(&vec![0u8; buffer_padding]); // padding (zeros)

    Ok(glb)
}

/// Calculate bounding box from all meshes in the output.
fn calculate_bounds_all(output: &MesherOutput) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];

    // Main layers are SoA (positions array); greedy materials are still AoS Mesh.
    let all_positions = output
        .opaque_mesh
        .positions
        .iter()
        .chain(output.cutout_mesh.positions.iter())
        .chain(output.transparent_mesh.positions.iter())
        .chain(output.greedy_materials.iter().flat_map(|gm| {
            gm.opaque_mesh
                .vertices
                .iter()
                .map(|v| &v.position)
                .chain(gm.transparent_mesh.vertices.iter().map(|v| &v.position))
        }));

    for position in all_positions {
        for i in 0..3 {
            min[i] = min[i].min(position[i]);
            max[i] = max[i].max(position[i]);
        }
    }

    // Handle empty case
    if min[0] == f32::MAX {
        min = [0.0; 3];
        max = [0.0; 3];
    }

    (min, max)
}

/// Create a buffer view.
fn create_buffer_view(
    offset: usize,
    size: usize,
    target: Option<json::buffer::Target>,
) -> json::buffer::View {
    json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(size as u64),
        byte_offset: Some(USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: target.map(Valid),
    }
}

/// Create an accessor with quantization support.
fn create_accessor(
    buffer_view: u32,
    count: usize,
    type_: json::accessor::Type,
    component_type: json::accessor::ComponentType,
    normalized: bool,
    min: Option<json::Value>,
    max: Option<json::Value>,
) -> json::Accessor {
    json::Accessor {
        buffer_view: Some(json::Index::new(buffer_view)),
        byte_offset: Some(USize64(0)),
        count: USize64(count as u64),
        component_type: Valid(json::accessor::GenericComponentType(component_type)),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(type_),
        min,
        max,
        normalized,
        sparse: None,
    }
}

/// Create a primitive.
fn create_primitive(
    positions_accessor: u32,
    indices_accessor: u32,
    material: u32,
) -> json::mesh::Primitive {
    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(
        Valid(json::mesh::Semantic::Positions),
        json::Index::new(positions_accessor),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Normals),
        json::Index::new(positions_accessor + 1),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::TexCoords(0)),
        json::Index::new(positions_accessor + 2),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Colors(0)),
        json::Index::new(positions_accessor + 3),
    );

    json::mesh::Primitive {
        attributes,
        extensions: Default::default(),
        extras: Default::default(),
        indices: Some(json::Index::new(indices_accessor)),
        material: Some(json::Index::new(material)),
        mode: Valid(json::mesh::Mode::Triangles),
        targets: None,
    }
}

/// Create a material with MASK alpha mode and a specific cutoff for binary-alpha textures.
fn create_material_with_alpha_cutoff(texture_idx: u32, cutoff: f32) -> json::Material {
    json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: json::Index::new(texture_idx),
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            }),
            base_color_factor: json::material::PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
            metallic_factor: json::material::StrengthFactor(0.0),
            roughness_factor: json::material::StrengthFactor(1.0),
            metallic_roughness_texture: None,
            extensions: Default::default(),
            extras: Default::default(),
        },
        alpha_mode: Valid(json::material::AlphaMode::Mask),
        alpha_cutoff: Some(json::material::AlphaCutoff(cutoff)),
        double_sided: true,
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: json::material::EmissiveFactor([0.0, 0.0, 0.0]),
        extensions: Default::default(),
        extras: Default::default(),
    }
}

/// Create a material with the specified alpha mode and texture index.
fn create_material_with_texture(alpha_mode: json::material::AlphaMode, texture_idx: u32) -> json::Material {
    // Opaque materials use backface culling; transparent/blended materials are double-sided
    // (cross-model plants, etc. need both sides visible)
    let double_sided = matches!(alpha_mode, json::material::AlphaMode::Blend);
    json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: json::Index::new(texture_idx),
                tex_coord: 0,
                extensions: Default::default(),
                extras: Default::default(),
            }),
            base_color_factor: json::material::PbrBaseColorFactor([1.0, 1.0, 1.0, 1.0]),
            metallic_factor: json::material::StrengthFactor(0.0),
            roughness_factor: json::material::StrengthFactor(1.0),
            metallic_roughness_texture: None,
            extensions: Default::default(),
            extras: Default::default(),
        },
        alpha_mode: Valid(alpha_mode),
        alpha_cutoff: None,
        double_sided,
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: json::material::EmissiveFactor([0.0, 0.0, 0.0]),
        extensions: Default::default(),
        extras: Default::default(),
    }
}

/// Cast a slice of T to a slice of bytes.
fn bytemuck_cast_slice<T: Copy>(slice: &[T]) -> &[u8] {
    let ptr = slice.as_ptr() as *const u8;
    let len = slice.len() * mem::size_of::<T>();
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    #[test]
    fn test_export_simple_mesh() {
        let mut mesh = Mesh::new();

        // Create a simple triangle
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let output = MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();

        // Check GLB header
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes([glb[4], glb[5], glb[6], glb[7]]), 2); // version
    }

    #[test]
    fn test_export_empty_mesh_fails() {
        let output = MesherOutput {
            opaque_mesh: Mesh::new(),
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let result = export_glb(&output);
        assert!(result.is_err());
    }

    #[test]
    fn test_export_transparent_only() {
        let mut mesh = Mesh::new();

        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let output = MesherOutput {
            opaque_mesh: Mesh::new(),
            cutout_mesh: Mesh::new(),
            transparent_mesh: mesh,
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
    }

    #[test]
    fn test_quantize_position() {
        let center = [5.0, 5.0, 5.0];
        let half_ext = [5.0, 5.0, 5.0];

        // Center should map to 0
        let q = super::quantize_position([5.0, 5.0, 5.0], &center, &half_ext);
        assert_eq!(q, [0, 0, 0]);

        // Max should map to 32767
        let q = super::quantize_position([10.0, 10.0, 10.0], &center, &half_ext);
        assert_eq!(q, [32767, 32767, 32767]);

        // Min should map to -32767
        let q = super::quantize_position([0.0, 0.0, 0.0], &center, &half_ext);
        assert_eq!(q, [-32767, -32767, -32767]);
    }

    #[test]
    fn test_quantize_normal() {
        // Axis-aligned normals should quantize exactly
        assert_eq!(super::quantize_normal([1.0, 0.0, 0.0]), [127, 0, 0]);
        assert_eq!(super::quantize_normal([0.0, 1.0, 0.0]), [0, 127, 0]);
        assert_eq!(super::quantize_normal([0.0, 0.0, -1.0]), [0, 0, -127]);
    }

    #[test]
    fn test_quantize_color() {
        assert_eq!(super::quantize_color([1.0, 1.0, 1.0, 1.0]), [255, 255, 255, 255]);
        assert_eq!(super::quantize_color([0.0, 0.0, 0.0, 0.0]), [0, 0, 0, 0]);
        assert_eq!(super::quantize_color([0.5, 0.5, 0.5, 1.0]), [128, 128, 128, 255]);
    }

    #[test]
    fn test_u16_indices_for_small_meshes() {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let output = MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();
        // Parse the JSON to verify u16 indices
        let json_chunk_len = u32::from_le_bytes([glb[12], glb[13], glb[14], glb[15]]) as usize;
        let json_str = std::str::from_utf8(&glb[20..20 + json_chunk_len]).unwrap().trim();
        // Should contain UNSIGNED_SHORT (5123) for indices, not UNSIGNED_INT (5125)
        assert!(json_str.contains("5123"), "Expected u16 indices (5123) in JSON");
    }

    #[test]
    fn test_quantized_glb_smaller_than_f32() {
        // Create a mesh with enough vertices to see a meaningful difference
        let mut mesh = Mesh::new();
        for i in 0..100 {
            let x = i as f32;
            let v0 = mesh.add_vertex(Vertex::new([x, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
            let v1 = mesh.add_vertex(Vertex::new([x + 1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
            let v2 = mesh.add_vertex(Vertex::new([x, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
            mesh.add_triangle(v0, v1, v2);
        }

        let output = MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [100.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let glb = export_glb(&output).unwrap();
        // 300 vertices × 48 bytes old = 14400 bytes for vertex data alone
        // 300 vertices × 21 bytes new = 6300 bytes + alignment padding
        // Total GLB includes JSON + textures, so just verify it's reasonably sized
        assert!(glb.len() < 14400, "Quantized GLB ({}) should be smaller than old vertex data alone (14400)", glb.len());
    }
}
