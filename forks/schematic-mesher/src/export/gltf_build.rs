//! Build GLB export — one node per construction group with translation /
//! rotation / scale keyframe tracks, named child anchors, per-node `extras`,
//! and one material per texture atlas.
//!
//! This is the general form of `gltf_animated`: any recorder that can produce
//! meshes and typed tracks can emit a scene. Node and animation names are
//! written into the serialized JSON directly (gltf-json's `names` feature stays
//! off), and whatever core glTF cannot express — colour tracks, cameras as
//! orbit parameters — travels in `extras`, where viewers that do not know it
//! ignore it.
use super::gltf_animated::{
    accessor, align, buffer_view, cast_bytes, empty_node, push_mesh_geometry,
};
use crate::atlas::TextureAtlas;
use crate::error::{MesherError, Result};
use crate::mesh_output::MeshLayer;
use crate::mesher::geometry::{Mesh, Vertex};
use gltf_json as json;
use json::validation::Checked::Valid;

/// Keyframe interpolation for one track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
}

impl From<Interpolation> for json::animation::Interpolation {
    fn from(value: Interpolation) -> Self {
        match value {
            Interpolation::Linear => json::animation::Interpolation::Linear,
            Interpolation::Step => json::animation::Interpolation::Step,
        }
    }
}

/// A keyframe track: `times` in seconds, one value per time.
#[derive(Debug, Clone, PartialEq)]
pub struct Track<const N: usize> {
    pub times: Vec<f32>,
    pub values: Vec<[f32; N]>,
    pub interpolation: Interpolation,
}

/// An empty child node — a named point that follows its parent's pose.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildChild {
    pub name: String,
    pub translation: [f32; 3],
    pub extras: Option<serde_json::Value>,
}

/// One animated node: model-space geometry, the atlas it samples, optional
/// TRS tracks, `extras`, and child anchors. An empty mesh yields an empty node
/// so indices stay aligned with the recorder's groups.
#[derive(Debug, Clone)]
pub struct BuildNode {
    pub name: String,
    pub mesh: Mesh,
    pub atlas: usize,
    pub translation: Option<Track<3>>,
    pub rotation: Option<Track<4>>,
    pub scale: Option<Track<3>>,
    pub extras: Option<serde_json::Value>,
    pub children: Vec<BuildChild>,
}

/// The whole scene: root node `build:<name>`, one animation named `<name>`.
#[derive(Debug, Clone)]
pub struct BuildScene {
    pub name: String,
    pub atlases: Vec<TextureAtlas>,
    pub nodes: Vec<BuildNode>,
    pub extras: Option<serde_json::Value>,
}

/// Flatten mesher layers (opaque, cutout, transparent) into one mesh.
pub fn mesh_from_layers(layers: &[&MeshLayer]) -> Mesh {
    let mut mesh = Mesh::new();
    for layer in layers {
        let base = mesh.vertices.len() as u32;
        for (i, position) in layer.positions.iter().enumerate() {
            let normal = layer.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let uv = layer.uvs.get(i).copied().unwrap_or([0.0, 0.0]);
            let color = layer.colors.get(i).copied().unwrap_or([1.0; 4]);
            mesh.add_vertex(Vertex::new(*position, normal, uv).with_color(color));
        }
        mesh.indices
            .extend(layer.indices.iter().map(|index| index + base));
    }
    mesh
}

fn extras(value: &Option<serde_json::Value>) -> Result<json::Extras> {
    match value {
        None => Ok(Default::default()),
        Some(value) => {
            let text = serde_json::to_string(value)
                .map_err(|e| MesherError::Export(format!("extras serialize failed: {e}")))?;
            let raw = serde_json::value::RawValue::from_string(text)
                .map_err(|e| MesherError::Export(format!("extras are not valid JSON: {e}")))?;
            Ok(Some(raw))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_channel<const N: usize>(
    buf: &mut Vec<u8>,
    buffer_views: &mut Vec<json::buffer::View>,
    accessors: &mut Vec<json::Accessor>,
    samplers: &mut Vec<json::animation::Sampler>,
    channels: &mut Vec<json::animation::Channel>,
    node: u32,
    track: &Track<N>,
    property: json::animation::Property,
) -> Result<()> {
    if track.times.is_empty() || track.times.len() != track.values.len() {
        return Err(MesherError::Export(format!(
            "track for node {node} has {} times and {} values",
            track.times.len(),
            track.values.len()
        )));
    }
    let type_ = match N {
        1 => json::accessor::Type::Scalar,
        2 => json::accessor::Type::Vec2,
        3 => json::accessor::Type::Vec3,
        4 => json::accessor::Type::Vec4,
        _ => return Err(MesherError::Export(format!("unsupported track width {N}"))),
    };
    align(buf, 4);
    let t_off = buf.len();
    let mut tmin = f32::MAX;
    let mut tmax = f32::MIN;
    for t in &track.times {
        tmin = tmin.min(*t);
        tmax = tmax.max(*t);
        buf.extend_from_slice(&t.to_le_bytes());
    }
    align(buf, 4);
    let v_off = buf.len();
    for v in &track.values {
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
        track.times.len(),
        json::accessor::Type::Scalar,
        json::accessor::ComponentType::F32,
        Some(json::Value::from(vec![tmin])),
        Some(json::Value::from(vec![tmax])),
    ));
    let v_acc = accessors.len() as u32;
    accessors.push(accessor(
        v_view,
        track.values.len(),
        type_,
        json::accessor::ComponentType::F32,
        None,
        None,
    ));
    let sampler = samplers.len() as u32;
    samplers.push(json::animation::Sampler {
        input: json::Index::new(t_acc),
        interpolation: Valid(track.interpolation.into()),
        output: json::Index::new(v_acc),
        extensions: Default::default(),
        extras: Default::default(),
    });
    channels.push(json::animation::Channel {
        sampler: json::Index::new(sampler),
        target: json::animation::Target {
            node: json::Index::new(node),
            path: Valid(property),
            extensions: Default::default(),
            extras: Default::default(),
        },
        extensions: Default::default(),
        extras: Default::default(),
    });
    Ok(())
}

/// Assemble a build scene into a binary glTF.
pub fn export_build_glb(scene: &BuildScene) -> Result<Vec<u8>> {
    if scene.nodes.iter().all(|node| node.mesh.is_empty()) {
        return Err(MesherError::Export(
            "Cannot export a build with no geometry".to_string(),
        ));
    }
    if let Some(node) = scene
        .nodes
        .iter()
        .find(|node| node.atlas >= scene.atlases.len())
    {
        return Err(MesherError::Export(format!(
            "node {} references atlas {} but the scene has {}",
            node.name,
            node.atlas,
            scene.atlases.len()
        )));
    }

    let mut buf: Vec<u8> = Vec::new();
    let mut buffer_views: Vec<json::buffer::View> = Vec::new();
    let mut accessors: Vec<json::Accessor> = Vec::new();
    let mut meshes: Vec<json::Mesh> = Vec::new();
    let mut nodes: Vec<json::Node> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut channels: Vec<json::animation::Channel> = Vec::new();
    let mut samplers: Vec<json::animation::Sampler> = Vec::new();

    // Root first, so index 0 is always the build.
    let mut root = empty_node();
    root.extras = extras(&scene.extras)?;
    nodes.push(root);
    names.push(format!("build:{}", scene.name));

    let mut group_indices: Vec<json::Index<json::Node>> = Vec::with_capacity(scene.nodes.len());
    for node in &scene.nodes {
        let node_idx = nodes.len() as u32;
        group_indices.push(json::Index::new(node_idx));
        let mut json_node = empty_node();
        if !node.mesh.is_empty() {
            let mesh_idx = push_mesh_geometry(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut meshes,
                &node.mesh,
                node.atlas as u32,
            );
            json_node.mesh = Some(json::Index::new(mesh_idx));
        }
        // Resting TRS = the first key of each track, so frame 0 reads right in a
        // viewer that has not started the animation player.
        json_node.translation = node
            .translation
            .as_ref()
            .and_then(|track| track.values.first().copied());
        json_node.rotation = node
            .rotation
            .as_ref()
            .and_then(|track| track.values.first().copied())
            .map(json::scene::UnitQuaternion);
        json_node.scale = node
            .scale
            .as_ref()
            .and_then(|track| track.values.first().copied());
        json_node.extras = extras(&node.extras)?;
        if !node.children.is_empty() {
            json_node.children = Some(
                (0..node.children.len())
                    .map(|i| json::Index::new(node_idx + 1 + i as u32))
                    .collect(),
            );
        }
        nodes.push(json_node);
        names.push(node.name.clone());
        for child in &node.children {
            let mut child_node = empty_node();
            child_node.translation = Some(child.translation);
            child_node.extras = extras(&child.extras)?;
            nodes.push(child_node);
            names.push(child.name.clone());
        }
        if let Some(track) = &node.translation {
            push_channel(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut samplers,
                &mut channels,
                node_idx,
                track,
                json::animation::Property::Translation,
            )?;
        }
        if let Some(track) = &node.rotation {
            push_channel(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut samplers,
                &mut channels,
                node_idx,
                track,
                json::animation::Property::Rotation,
            )?;
        }
        if let Some(track) = &node.scale {
            push_channel(
                &mut buf,
                &mut buffer_views,
                &mut accessors,
                &mut samplers,
                &mut channels,
                node_idx,
                track,
                json::animation::Property::Scale,
            )?;
        }
    }
    nodes[0].children = Some(group_indices);

    // One image / texture / material per atlas.
    let mut images = Vec::with_capacity(scene.atlases.len());
    let mut textures = Vec::with_capacity(scene.atlases.len());
    let mut materials = Vec::with_capacity(scene.atlases.len());
    for (index, atlas) in scene.atlases.iter().enumerate() {
        let png = atlas.to_png()?;
        align(&mut buf, 4);
        let tex_off = buf.len();
        buf.extend_from_slice(&png);
        let tex_view = buffer_views.len() as u32;
        buffer_views.push(buffer_view(tex_off, png.len(), None));
        images.push(json::Image {
            buffer_view: Some(json::Index::new(tex_view)),
            mime_type: Some(json::image::MimeType("image/png".to_string())),
            uri: None,
            extensions: Default::default(),
            extras: Default::default(),
        });
        textures.push(json::Texture {
            sampler: Some(json::Index::new(0)),
            source: json::Index::new(index as u32),
            extensions: Default::default(),
            extras: Default::default(),
        });
        materials.push(json::Material {
            pbr_metallic_roughness: json::material::PbrMetallicRoughness {
                base_color_texture: Some(json::texture::Info {
                    index: json::Index::new(index as u32),
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
        });
    }
    let total_buffer = buf.len();

    let animations = if channels.is_empty() {
        Vec::new()
    } else {
        vec![json::Animation {
            extensions: Default::default(),
            extras: Default::default(),
            channels,
            samplers,
        }]
    };
    let root = json::Root {
        accessors,
        animations,
        buffers: vec![json::Buffer {
            byte_length: json::validation::USize64(total_buffer as u64),
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
            nodes: vec![json::Index::new(0)],
        }],
        scene: Some(json::Index::new(0)),
        ..Default::default()
    };

    // Names live outside gltf-json's optional `names` feature: inject them.
    let mut value = serde_json::to_value(&root)
        .map_err(|e| MesherError::Export(format!("glTF JSON serialize failed: {e}")))?;
    if let Some(json_nodes) = value["nodes"].as_array_mut() {
        for (node, name) in json_nodes.iter_mut().zip(&names) {
            node["name"] = serde_json::Value::String(name.clone());
        }
    }
    if let Some(animation) = value["animations"].get_mut(0) {
        animation["name"] = serde_json::Value::String(scene.name.clone());
    }
    let json_bytes = serde_json::to_vec(&value)
        .map_err(|e| MesherError::Export(format!("glTF JSON serialize failed: {e}")))?;
    let json_pad = (4 - (json_bytes.len() % 4)) % 4;
    let buf_pad = (4 - (buf.len() % 4)) % 4;

    let total = 12 + 8 + json_bytes.len() + json_pad + 8 + buf.len() + buf_pad;
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&((json_bytes.len() + json_pad) as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(&json_bytes);
    glb.extend(std::iter::repeat(0x20u8).take(json_pad));
    glb.extend_from_slice(&((buf.len() + buf_pad) as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    glb.extend_from_slice(&buf);
    glb.extend(std::iter::repeat(0u8).take(buf_pad));
    Ok(glb)
}
