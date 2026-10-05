//! Animated GLB export — a multi-node scene with glTF keyframe animation.
//!
//! `export_glb` (in `gltf.rs`) bakes one static mesh under one node. This
//! module instead emits one node *per animated piece* and a glTF `Animation`
//! with keyframe channels, so a scenario replay can show block-state changes
//! (redstone turning on) and block motion (a piston push) play out over time.
//!
//! Design choices for this first cut, kept deliberately simple:
//! - Vertices are plain `f32` (no `KHR_mesh_quantization`). Replay scenes are
//!   small contraptions, so the size cost is irrelevant and it avoids the
//!   quantization-transform-vs-animation-transform conflict.
//! - Every piece's geometry is flattened into a single mesh and drawn with one
//!   shared `MASK` (alpha-tested) material. Opaque and cutout textures both
//!   render correctly under alpha-test; truly transparent blocks (water/glass)
//!   would be slightly off, which is fine for redstone replays.
//! - A `scale` track with STEP interpolation toggles a node between [0,0,0]
//!   (invisible) and [1,1,1] (visible) — STEP means the collapsed-to-origin
//!   intermediate is never seen, so it reads as a clean on/off.
//! - A `translation` track with LINEAR interpolation slides a node — it is an
//!   offset delta layered on the mesh's baked model-local position.

use crate::atlas::TextureAtlas;
use crate::error::{MesherError, Result};
use crate::mesher::geometry::Mesh;
use gltf_json as json;
use json::validation::Checked::Valid;
use json::validation::USize64;

/// One animated node in the scene: a baked mesh plus optional keyframe tracks.
/// The mesh is in model-local coordinates (world − region origin). A piece with
/// no tracks is static.
pub struct AnimatedPiece {
    /// Merged geometry for this node (opaque + cutout + transparent flattened).
    pub mesh: Mesh,
    /// Optional scale track — STEP interpolation. Use [0,0,0]↔[1,1,1] to toggle
    /// visibility (a block-state variant blinking on/off).
    pub scale_keys: Option<Vec<(f32, [f32; 3])>>,
    /// Optional translation track — LINEAR interpolation. An offset delta
    /// layered on top of the mesh's baked model-local position.
    pub translation_keys: Option<Vec<(f32, [f32; 3])>>,
}

/// Cast a slice of `Copy` values to bytes (little-endian on all our targets).
pub(super) fn cast_bytes<T: Copy>(slice: &[T]) -> &[u8] {
    let ptr = slice.as_ptr() as *const u8;
    let len = std::mem::size_of_val(slice);
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

pub(super) fn align(buf: &mut Vec<u8>, alignment: usize) {
    let pad = (alignment - (buf.len() % alignment)) % alignment;
    buf.extend(std::iter::repeat(0u8).take(pad));
}

pub(super) fn buffer_view(
    offset: usize,
    len: usize,
    target: Option<json::buffer::Target>,
) -> json::buffer::View {
    json::buffer::View {
        buffer: json::Index::new(0),
        byte_length: USize64(len as u64),
        byte_offset: Some(USize64(offset as u64)),
        byte_stride: None,
        extensions: Default::default(),
        extras: Default::default(),
        target: target.map(Valid),
    }
}

pub(super) fn accessor(
    view: u32,
    count: usize,
    type_: json::accessor::Type,
    component: json::accessor::ComponentType,
    min: Option<json::Value>,
    max: Option<json::Value>,
) -> json::Accessor {
    json::Accessor {
        buffer_view: Some(json::Index::new(view)),
        byte_offset: Some(USize64(0)),
        count: USize64(count as u64),
        component_type: Valid(json::accessor::GenericComponentType(component)),
        extensions: Default::default(),
        extras: Default::default(),
        type_: Valid(type_),
        min,
        max,
        normalized: false,
        sparse: None,
    }
}

/// Assemble a multi-node animated GLB from pre-meshed pieces sharing one atlas.
pub fn export_animated_glb(atlas: &TextureAtlas, pieces: &[AnimatedPiece]) -> Result<Vec<u8>> {
    if pieces.iter().all(|p| p.mesh.is_empty()) {
        return Err(MesherError::Export(
            "Cannot export empty animated mesh".to_string(),
        ));
    }

    let texture_png = atlas.to_png()?;

    let mut buf: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<json::buffer::View> = Vec::new();
    let mut accessors: Vec<json::Accessor> = Vec::new();
    let mut meshes: Vec<json::Mesh> = Vec::new();
    let mut nodes: Vec<json::Node> = Vec::new();
    let mut anim_channels: Vec<json::animation::Channel> = Vec::new();
    let mut anim_samplers: Vec<json::animation::Sampler> = Vec::new();

    // --- per-piece geometry → mesh + node ---
    for (piece_idx, piece) in pieces.iter().enumerate() {
        let mesh = &piece.mesh;
        if mesh.is_empty() {
            // Keep node indices aligned with piece indices — emit an empty node.
            nodes.push(empty_node());
            continue;
        }

        let mesh_idx = push_mesh_geometry(
            &mut buf,
            &mut buffer_views,
            &mut accessors,
            &mut meshes,
            mesh,
            0,
        );

        nodes.push(json::Node {
            camera: None,
            children: None,
            extensions: Default::default(),
            extras: Default::default(),
            matrix: None,
            mesh: Some(json::Index::new(mesh_idx)),
            rotation: None,
            // A scale-toggled node starts hidden iff its first key is [0,0,0];
            // bake that as the node's resting scale so frame 0 looks right even
            // before the animation player kicks in.
            scale: piece
                .scale_keys
                .as_ref()
                .map(|k| k.first().map(|(_, s)| *s).unwrap_or([1.0; 3])),
            translation: None,
            skin: None,
            weights: None,
        });

        // node index == piece index by construction
        let node_idx = piece_idx as u32;

        if let Some(keys) = &piece.scale_keys {
            push_vec3_channel(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut anim_samplers,
                &mut anim_channels,
                node_idx,
                keys,
                json::animation::Property::Scale,
                json::animation::Interpolation::Step,
            );
        }
        if let Some(keys) = &piece.translation_keys {
            push_vec3_channel(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut anim_samplers,
                &mut anim_channels,
                node_idx,
                keys,
                json::animation::Property::Translation,
                json::animation::Interpolation::Linear,
            );
        }
    }

    // --- atlas image ---
    align(&mut buf, 4);
    let tex_off = buf.len();
    buf.extend_from_slice(&texture_png);
    let tex_view = buffer_views.len() as u32;
    buffer_views.push(buffer_view(tex_off, texture_png.len(), None));

    let total_buffer = buf.len();

    let images = vec![json::Image {
        buffer_view: Some(json::Index::new(tex_view)),
        mime_type: Some(json::image::MimeType("image/png".to_string())),
        uri: None,
        extensions: Default::default(),
        extras: Default::default(),
    }];
    let textures = vec![json::Texture {
        sampler: Some(json::Index::new(0)),
        source: json::Index::new(0),
        extensions: Default::default(),
        extras: Default::default(),
    }];
    // One shared alpha-tested material (handles opaque + cutout textures).
    let materials = vec![json::Material {
        pbr_metallic_roughness: json::material::PbrMetallicRoughness {
            base_color_texture: Some(json::texture::Info {
                index: json::Index::new(0),
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
        alpha_cutoff: Some(json::material::AlphaCutoff(0.5)),
        double_sided: true,
        normal_texture: None,
        occlusion_texture: None,
        emissive_texture: None,
        emissive_factor: json::material::EmissiveFactor([0.0, 0.0, 0.0]),
        extensions: Default::default(),
        extras: Default::default(),
    }];

    let scene_node_indices: Vec<json::Index<json::Node>> =
        (0..nodes.len() as u32).map(json::Index::new).collect();

    let animations = if anim_channels.is_empty() {
        Vec::new()
    } else {
        vec![json::Animation {
            extensions: Default::default(),
            extras: Default::default(),
            channels: anim_channels,
            samplers: anim_samplers,
        }]
    };

    let root = json::Root {
        accessors,
        animations,
        buffers: vec![json::Buffer {
            byte_length: USize64(total_buffer as u64),
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
        meshes,
        nodes,
        scenes: vec![json::Scene {
            extensions: Default::default(),
            extras: Default::default(),
            nodes: scene_node_indices,
        }],
        scene: Some(json::Index::new(0)),
        ..Default::default()
    };

    let json_string = json::serialize::to_string(&root)
        .map_err(|e| MesherError::Export(format!("glTF JSON serialize failed: {}", e)))?;
    let json_bytes = json_string.as_bytes();
    let json_pad = (4 - (json_bytes.len() % 4)) % 4;
    let buf_pad = (4 - (buf.len() % 4)) % 4;

    let total = 12 + 8 + json_bytes.len() + json_pad + 8 + buf.len() + buf_pad;
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&((json_bytes.len() + json_pad) as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(json_bytes);
    glb.extend(std::iter::repeat(0x20u8).take(json_pad));
    glb.extend_from_slice(&((buf.len() + buf_pad) as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(&buf);
    glb.extend(std::iter::repeat(0u8).take(buf_pad));
    Ok(glb)
}

pub(super) fn empty_node() -> json::Node {
    json::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: None,
        mesh: None,
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    }
}

/// Append a keyframe channel (times + vec3 values) for one node property.
#[allow(clippy::too_many_arguments)]
pub(super) fn push_vec3_channel(
    buf: &mut Vec<u8>,
    buffer_views: &mut Vec<json::buffer::View>,
    accessors: &mut Vec<json::Accessor>,
    samplers: &mut Vec<json::animation::Sampler>,
    channels: &mut Vec<json::animation::Channel>,
    node: u32,
    keys: &[(f32, [f32; 3])],
    property: json::animation::Property,
    interpolation: json::animation::Interpolation,
) {
    if keys.is_empty() {
        return;
    }
    // input: f32 scalar times (accessor requires min/max)
    align(buf, 4);
    let t_off = buf.len();
    let mut tmin = f32::MAX;
    let mut tmax = f32::MIN;
    for (t, _) in keys {
        tmin = tmin.min(*t);
        tmax = tmax.max(*t);
        buf.extend_from_slice(&t.to_le_bytes());
    }
    // output: f32 vec3 values
    align(buf, 4);
    let v_off = buf.len();
    for (_, v) in keys {
        buf.extend_from_slice(cast_bytes(v));
    }
    let end = buf.len();

    let t_view = buffer_views.len() as u32;
    buffer_views.push(buffer_view(t_off, v_off - t_off, None));
    let v_view = buffer_views.len() as u32;
    buffer_views.push(buffer_view(v_off, end - v_off, None));

    let t_acc = accessors.len() as u32;
    accessors.push(accessor(
        t_view,
        keys.len(),
        json::accessor::Type::Scalar,
        json::accessor::ComponentType::F32,
        Some(json::Value::from(vec![tmin])),
        Some(json::Value::from(vec![tmax])),
    ));
    let v_acc = accessors.len() as u32;
    accessors.push(accessor(
        v_view,
        keys.len(),
        json::accessor::Type::Vec3,
        json::accessor::ComponentType::F32,
        None,
        None,
    ));

    let sampler_idx = samplers.len() as u32;
    samplers.push(json::animation::Sampler {
        input: json::Index::new(t_acc),
        output: json::Index::new(v_acc),
        interpolation: Valid(interpolation),
        extensions: Default::default(),
        extras: Default::default(),
    });
    channels.push(json::animation::Channel {
        sampler: json::Index::new(sampler_idx),
        target: json::animation::Target {
            node: json::Index::new(node),
            path: Valid(property),
            extensions: Default::default(),
            extras: Default::default(),
        },
        extensions: Default::default(),
        extras: Default::default(),
    });
}

/// Write one mesh's vertex streams and indices into the shared buffer and
/// register its accessors and `json::Mesh`. Returns the mesh index.
pub(super) fn push_mesh_geometry(
    buf: &mut Vec<u8>,
    buffer_views: &mut Vec<json::buffer::View>,
    accessors: &mut Vec<json::Accessor>,
    meshes: &mut Vec<json::Mesh>,
    mesh: &Mesh,
    material: u32,
) -> u32 {
    // positions (f32 vec3) + min/max
    align(buf, 4);
    let pos_off = buf.len();
    let mut pmin = [f32::MAX; 3];
    let mut pmax = [f32::MIN; 3];
    for v in &mesh.vertices {
        for i in 0..3 {
            pmin[i] = pmin[i].min(v.position[i]);
            pmax[i] = pmax[i].max(v.position[i]);
        }
        buf.extend_from_slice(cast_bytes(&v.position));
    }
    // normals (f32 vec3)
    align(buf, 4);
    let norm_off = buf.len();
    for v in &mesh.vertices {
        buf.extend_from_slice(cast_bytes(&v.normal));
    }
    // uvs (f32 vec2)
    align(buf, 4);
    let uv_off = buf.len();
    for v in &mesh.vertices {
        buf.extend_from_slice(cast_bytes(&v.uv));
    }
    // colors (f32 vec4)
    align(buf, 4);
    let color_off = buf.len();
    for v in &mesh.vertices {
        buf.extend_from_slice(cast_bytes(&v.color));
    }
    // indices (u32)
    align(buf, 4);
    let idx_off = buf.len();
    buf.extend_from_slice(cast_bytes(&mesh.indices));
    let end = buf.len();

    let vcount = mesh.vertices.len();
    let av = accessors.len() as u32;
    let bv = buffer_views.len() as u32;

    buffer_views.push(buffer_view(
        pos_off,
        norm_off - pos_off,
        Some(json::buffer::Target::ArrayBuffer),
    ));
    buffer_views.push(buffer_view(
        norm_off,
        uv_off - norm_off,
        Some(json::buffer::Target::ArrayBuffer),
    ));
    buffer_views.push(buffer_view(
        uv_off,
        color_off - uv_off,
        Some(json::buffer::Target::ArrayBuffer),
    ));
    buffer_views.push(buffer_view(
        color_off,
        idx_off - color_off,
        Some(json::buffer::Target::ArrayBuffer),
    ));
    buffer_views.push(buffer_view(
        idx_off,
        end - idx_off,
        Some(json::buffer::Target::ElementArrayBuffer),
    ));

    accessors.push(accessor(
        bv,
        vcount,
        json::accessor::Type::Vec3,
        json::accessor::ComponentType::F32,
        Some(json::Value::from(vec![pmin[0], pmin[1], pmin[2]])),
        Some(json::Value::from(vec![pmax[0], pmax[1], pmax[2]])),
    ));
    accessors.push(accessor(
        bv + 1,
        vcount,
        json::accessor::Type::Vec3,
        json::accessor::ComponentType::F32,
        None,
        None,
    ));
    accessors.push(accessor(
        bv + 2,
        vcount,
        json::accessor::Type::Vec2,
        json::accessor::ComponentType::F32,
        None,
        None,
    ));
    accessors.push(accessor(
        bv + 3,
        vcount,
        json::accessor::Type::Vec4,
        json::accessor::ComponentType::F32,
        None,
        None,
    ));
    accessors.push(accessor(
        bv + 4,
        mesh.indices.len(),
        json::accessor::Type::Scalar,
        json::accessor::ComponentType::U32,
        None,
        None,
    ));

    let mut attributes = std::collections::BTreeMap::new();
    attributes.insert(Valid(json::mesh::Semantic::Positions), json::Index::new(av));
    attributes.insert(
        Valid(json::mesh::Semantic::Normals),
        json::Index::new(av + 1),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::TexCoords(0)),
        json::Index::new(av + 2),
    );
    attributes.insert(
        Valid(json::mesh::Semantic::Colors(0)),
        json::Index::new(av + 3),
    );

    let mesh_idx = meshes.len() as u32;
    meshes.push(json::Mesh {
        extensions: Default::default(),
        extras: Default::default(),
        primitives: vec![json::mesh::Primitive {
            attributes,
            extensions: Default::default(),
            extras: Default::default(),
            indices: Some(json::Index::new(av + 4)),
            material: Some(json::Index::new(material)),
            mode: Valid(json::mesh::Mode::Triangles),
            targets: None,
        }],
        weights: None,
    });
    mesh_idx
}
