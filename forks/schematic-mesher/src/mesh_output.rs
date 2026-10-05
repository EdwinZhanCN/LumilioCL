//! Canonical mesh output types for renderer-agnostic mesh data.
//!
//! [`MeshOutput`] and [`MeshLayer`] are the primary public types for consuming mesh
//! data from the mesher. They replace the internal [`MesherOutput`](crate::mesher::MesherOutput)
//! for most use cases.
//!
//! ## Architecture
//!
//! [`MeshOutput`] contains three [`MeshLayer`]s — one per transparency class (opaque,
//! cutout, transparent) — plus the shared texture atlas, animation metadata, and spatial
//! bounds. Each layer stores vertex attributes in structure-of-arrays layout:
//!
//! | Attribute | Type | Stride | Accessor |
//! |-----------|------|--------|----------|
//! | Position  | `[f32; 3]` | 12 bytes | [`positions_bytes()`](MeshLayer::positions_bytes) |
//! | Normal    | `[f32; 3]` | 12 bytes | [`normals_bytes()`](MeshLayer::normals_bytes) |
//! | UV        | `[f32; 2]` | 8 bytes  | [`uvs_bytes()`](MeshLayer::uvs_bytes) |
//! | Color     | `[f32; 4]` | 16 bytes | [`colors_bytes()`](MeshLayer::colors_bytes) |
//! | Index     | `u32`      | 4 bytes  | [`indices_bytes()`](MeshLayer::indices_bytes) |
//!
//! The `_bytes()` methods return `&[u8]` slices over the existing memory — zero allocation,
//! zero copy — suitable for direct upload to GPU vertex/index buffers.
//!
//! ## Conversion
//!
//! Convert from the internal output type via [`From`]:
//!
//! ```ignore
//! let mesh_output = MeshOutput::from(&mesher_output);
//! ```
//!
//! This merges greedy material meshes into the opaque/transparent layers. To convert
//! back (e.g., for use with standalone export functions), use [`to_glb()`](MeshOutput::to_glb),
//! [`to_usdz()`](MeshOutput::to_usdz), or [`to_obj()`](MeshOutput::to_obj).

use crate::atlas::TextureAtlas;
use crate::error::Result;
use crate::export::obj::ObjExport;
use crate::mesher::AnimatedTextureExport;
use crate::types::BoundingBox;
use std::mem;

/// A single mesh layer with vertex attributes and indices.
///
/// Each layer represents geometry for one transparency class (opaque, cutout, or transparent).
/// Vertex data is stored in structure-of-arrays layout for efficient GPU upload.
#[derive(Debug, Clone, Default)]
pub struct MeshLayer {
    /// Vertex positions in world space.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (unit length).
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates into the atlas.
    pub uvs: Vec<[f32; 2]>,
    /// Vertex tint colors (biome coloring, AO, lighting baked in). RGBA, premultiplied.
    pub colors: Vec<[f32; 4]>,
    /// Triangle indices (three per triangle).
    pub indices: Vec<u32>,
}

impl MeshLayer {
    /// Create a new empty mesh layer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if this layer contains no vertices.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Number of vertices in this layer.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Number of triangles in this layer.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Raw bytes of the positions array. Zero-allocation view.
    pub fn positions_bytes(&self) -> &[u8] {
        cast_slice(&self.positions)
    }

    /// Raw bytes of the normals array. Zero-allocation view.
    pub fn normals_bytes(&self) -> &[u8] {
        cast_slice(&self.normals)
    }

    /// Raw bytes of the UVs array. Zero-allocation view.
    pub fn uvs_bytes(&self) -> &[u8] {
        cast_slice(&self.uvs)
    }

    /// Raw bytes of the colors array. Zero-allocation view.
    pub fn colors_bytes(&self) -> &[u8] {
        cast_slice(&self.colors)
    }

    /// Raw bytes of the indices array. Zero-allocation view.
    pub fn indices_bytes(&self) -> &[u8] {
        cast_slice(&self.indices)
    }

    /// Push one vertex's attributes (SoA) and return its index. Used by the
    /// transparency split so geometry lands directly in SoA layout — no AoS
    /// `Vertex`/`Mesh` intermediate that would have to be converted later.
    #[inline]
    pub(crate) fn push_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        color: [f32; 4],
    ) -> u32 {
        let idx = self.positions.len() as u32;
        self.positions.push(position);
        self.normals.push(normal);
        self.uvs.push(uv);
        self.colors.push(color);
        idx
    }

    /// Reserve capacity in all attribute arrays + indices.
    pub(crate) fn reserve(&mut self, verts: usize, indices: usize) {
        self.positions.reserve(verts);
        self.normals.reserve(verts);
        self.uvs.reserve(verts);
        self.colors.reserve(verts);
        self.indices.reserve(indices);
    }

    /// Merge another layer into this one, offsetting indices appropriately.
    pub fn merge(&mut self, other: &MeshLayer) {
        let offset = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|&i| i + offset));
    }
}

/// Cast a slice of `T` to a byte slice without allocation.
fn cast_slice<T: Copy>(slice: &[T]) -> &[u8] {
    let ptr = slice.as_ptr() as *const u8;
    let len = slice.len() * mem::size_of::<T>();
    // SAFETY: [f32; N], [u32] are all Pod-like types with no padding.
    unsafe { std::slice::from_raw_parts(ptr, len) }
}

/// A greedy-merged material: geometry that uses its own texture (not the
/// atlas) so tiled UVs `[0..w, 0..h]` can wrap cleanly via a REPEAT sampler.
///
/// Exporters emit one primitive + one texture + one material per
/// `GreedyMaterialOutput`. Collapsing these into the shared-atlas opaque
/// layer would apply the tile UVs to the atlas sampler, smearing the full
/// atlas across each merged face — the bug fix this type exists to prevent.
#[derive(Debug, Clone)]
pub struct GreedyMaterialOutput {
    /// Resource-pack texture path (e.g. `"block/stone"`).
    pub texture_path: String,
    /// Opaque geometry using this texture. UVs are tile-space `[0..w, 0..h]`.
    pub opaque: MeshLayer,
    /// Transparent geometry using this texture. UVs are tile-space.
    pub transparent: MeshLayer,
    /// PNG-encoded texture data for this material (atlas NOT applied).
    pub texture_png: Vec<u8>,
}

impl GreedyMaterialOutput {
    /// `true` if neither the opaque nor transparent layer has any geometry.
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.transparent.is_empty()
    }
}

/// The canonical mesh output from the mesher.
///
/// Contains three transparency layers (opaque, cutout, transparent), the texture atlas,
/// animation metadata, and spatial information. Provides convenience methods for export
/// and layer inspection.
#[derive(Debug, Clone)]
pub struct MeshOutput {
    /// Opaque geometry (rendered first, backface culled, depth write).
    pub opaque: MeshLayer,
    /// Alpha-test geometry (leaves, grass, fire — rendered second, depth write).
    pub cutout: MeshLayer,
    /// Alpha-blend geometry (glass, water — rendered last, no depth write).
    pub transparent: MeshLayer,
    /// The texture atlas shared by the `opaque` / `cutout` / `transparent` layers.
    pub atlas: TextureAtlas,
    /// Greedy-merged materials — one per unique `(texture_path, AO pattern)`
    /// in the source geometry. Each has its own texture and tile-space UVs;
    /// exporters must render them as separate primitives with REPEAT sampling.
    pub greedy_materials: Vec<GreedyMaterialOutput>,
    /// Animated texture metadata for viewer-side frame cycling.
    pub animated_textures: Vec<AnimatedTextureExport>,
    /// Axis-aligned bounding box of the meshed region.
    pub bounds: BoundingBox,
    /// Chunk coordinate, set when meshed via [`ChunkIter`]. `None` for single-shot meshing.
    pub chunk_coord: Option<(i32, i32, i32)>,
    /// LOD level. 0 = full detail, higher = coarser. No logic yet — hook for future LOD.
    pub lod_level: u8,
}

impl MeshOutput {
    /// Returns `true` if all layers are empty.
    pub fn is_empty(&self) -> bool {
        self.opaque.is_empty() && self.cutout.is_empty() && self.transparent.is_empty()
    }

    /// Total vertex count across all layers.
    pub fn total_vertices(&self) -> usize {
        self.opaque.vertex_count() + self.cutout.vertex_count() + self.transparent.vertex_count()
    }

    /// Total triangle count across all layers.
    pub fn total_triangles(&self) -> usize {
        self.opaque.triangle_count()
            + self.cutout.triangle_count()
            + self.transparent.triangle_count()
    }

    /// Returns `true` if there is any cutout or transparent geometry.
    pub fn has_transparency(&self) -> bool {
        !self.cutout.is_empty() || !self.transparent.is_empty()
    }

    /// Merge all layers into a single [`MeshLayer`]. Loses transparency ordering.
    pub fn flatten(&self) -> MeshLayer {
        let mut combined = self.opaque.clone();
        combined.merge(&self.cutout);
        combined.merge(&self.transparent);
        combined
    }

    /// Export to GLB (binary glTF) format.
    ///
    /// Converts the [`MeshOutput`] back to the internal [`MesherOutput`] format and
    /// delegates to the existing GLB exporter.
    pub fn to_glb(&self) -> Result<Vec<u8>> {
        let internal = self.to_mesher_output();
        crate::export::gltf::export_glb(&internal)
    }

    /// Export to USDZ format.
    pub fn to_usdz(&self) -> Result<Vec<u8>> {
        let internal = self.to_mesher_output();
        crate::export::usd::export_usdz(&internal)
    }

    /// Export to OBJ format.
    pub fn to_obj(&self, name: &str) -> Result<ObjExport> {
        let internal = self.to_mesher_output();
        ObjExport::from_output(&internal, name)
    }

    /// Convert back to the internal `MesherOutput` for use with existing exporters.
    fn to_mesher_output(&self) -> crate::mesher::MesherOutput {
        use crate::mesher::element::GreedyMaterial;

        let greedy_materials = self
            .greedy_materials
            .iter()
            .map(|gm| GreedyMaterial {
                texture_path: gm.texture_path.clone(),
                opaque_mesh: layer_to_mesh(&gm.opaque),
                transparent_mesh: layer_to_mesh(&gm.transparent),
                texture_png: gm.texture_png.clone(),
            })
            .collect();

        crate::mesher::MesherOutput {
            opaque_mesh: self.opaque.clone(),
            cutout_mesh: self.cutout.clone(),
            transparent_mesh: self.transparent.clone(),
            atlas: self.atlas.clone(),
            bounds: self.bounds,
            greedy_materials,
            animated_textures: self.animated_textures.clone(),
        }
    }
}

/// Convert a [`MeshLayer`] to the internal [`Mesh`](crate::mesher::geometry::Mesh) type.
///
/// This is exposed as `pub(crate)` for use by exporters.
pub(crate) fn layer_to_internal_mesh(layer: &MeshLayer) -> crate::mesher::geometry::Mesh {
    layer_to_mesh(layer)
}

/// Convert a [`MeshLayer`] to the internal [`Mesh`](crate::mesher::geometry::Mesh) type.
fn layer_to_mesh(layer: &MeshLayer) -> crate::mesher::geometry::Mesh {
    use crate::mesher::geometry::{Mesh, Vertex};

    let mut mesh = Mesh::new();
    for i in 0..layer.positions.len() {
        mesh.vertices.push(Vertex {
            position: layer.positions[i],
            normal: layer.normals[i],
            uv: layer.uvs[i],
            color: layer.colors[i],
        });
    }
    mesh.indices = layer.indices.clone();
    mesh
}

/// Convert an internal [`Mesh`](crate::mesher::geometry::Mesh) to a [`MeshLayer`].
///
/// Single pass over the (AoS) vertex buffer, splitting into the four SoA arrays
/// at once — the naive version made four separate strided passes over tens of
/// millions of 48-byte vertices (4x the memory reads).
pub(crate) fn mesh_to_layer(mesh: &crate::mesher::geometry::Mesh) -> MeshLayer {
    let n = mesh.vertices.len();
    let mut positions = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);
    let mut uvs = Vec::with_capacity(n);
    let mut colors = Vec::with_capacity(n);
    for v in &mesh.vertices {
        positions.push(v.position);
        normals.push(v.normal);
        uvs.push(v.uv);
        colors.push(v.color);
    }
    MeshLayer {
        positions,
        normals,
        uvs,
        colors,
        indices: mesh.indices.clone(),
    }
}

impl From<crate::mesher::MesherOutput> for MeshOutput {
    /// Convert from the internal [`MesherOutput`] to the canonical [`MeshOutput`].
    ///
    /// Takes the output by value: the main layers are already SoA [`MeshLayer`]s,
    /// so they MOVE across with no copy (this is the whole point of the SoA
    /// pipeline — `to_mesh` no longer pays an AoS→SoA conversion). Greedy
    /// materials are preserved as dedicated [`GreedyMaterialOutput`] entries
    /// (their tile-space UVs only render correctly against their own per-material
    /// REPEAT-sampled texture, so they are not merged into the atlas layers).
    fn from(output: crate::mesher::MesherOutput) -> Self {
        let greedy_materials = output
            .greedy_materials
            .iter()
            .map(|gm| GreedyMaterialOutput {
                texture_path: gm.texture_path.clone(),
                opaque: mesh_to_layer(&gm.opaque_mesh),
                transparent: mesh_to_layer(&gm.transparent_mesh),
                texture_png: gm.texture_png.clone(),
            })
            .collect();

        MeshOutput {
            opaque: output.opaque_mesh,
            cutout: output.cutout_mesh,
            transparent: output.transparent_mesh,
            atlas: output.atlas,
            greedy_materials,
            animated_textures: output.animated_textures,
            bounds: output.bounds,
            chunk_coord: None,
            lod_level: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_layer_merge() {
        let mut a = MeshLayer {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let b = MeshLayer {
            positions: vec![[2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [2.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        a.merge(&b);

        assert_eq!(a.vertex_count(), 6);
        assert_eq!(a.triangle_count(), 2);
        // Second triangle's indices should be offset by 3
        assert_eq!(a.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(a.positions[3], [2.0, 0.0, 0.0]);
    }

    #[test]
    fn test_mesh_layer_bytes_zero_alloc() {
        let layer = MeshLayer {
            positions: vec![[1.0, 2.0, 3.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.5, 0.5]],
            colors: vec![[1.0, 0.0, 0.0, 1.0]],
            indices: vec![0],
        };

        assert_eq!(layer.positions_bytes().len(), 12); // 3 * 4 bytes
        assert_eq!(layer.normals_bytes().len(), 12);
        assert_eq!(layer.uvs_bytes().len(), 8); // 2 * 4 bytes
        assert_eq!(layer.colors_bytes().len(), 16); // 4 * 4 bytes
        assert_eq!(layer.indices_bytes().len(), 4); // 1 * 4 bytes
    }

    #[test]
    fn test_mesh_output_flatten() {
        let opaque = MeshLayer {
            positions: vec![[0.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.0, 0.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]],
            indices: vec![0],
        };
        let cutout = MeshLayer {
            positions: vec![[1.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[1.0, 0.0]],
            colors: vec![[1.0, 1.0, 1.0, 1.0]],
            indices: vec![0],
        };
        let transparent = MeshLayer {
            positions: vec![[2.0, 0.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]],
            uvs: vec![[0.0, 1.0]],
            colors: vec![[1.0, 1.0, 1.0, 0.5]],
            indices: vec![0],
        };

        let output = MeshOutput {
            opaque,
            cutout,
            transparent,
            atlas: TextureAtlas::empty(),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [3.0, 1.0, 1.0]),
            chunk_coord: None,
            lod_level: 0,
        };

        assert_eq!(output.total_vertices(), 3);
        assert_eq!(output.total_triangles(), 0); // each layer has 1 index, not enough for a full triangle
        assert!(output.has_transparency());

        let flat = output.flatten();
        assert_eq!(flat.vertex_count(), 3);
        // indices: [0] merged with [0] (offset 1) merged with [0] (offset 2) = [0, 1, 2]
        assert_eq!(flat.indices, vec![0, 1, 2]);
    }

    #[test]
    fn test_mesh_output_is_empty() {
        let output = MeshOutput {
            opaque: MeshLayer::new(),
            cutout: MeshLayer::new(),
            transparent: MeshLayer::new(),
            atlas: TextureAtlas::empty(),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            chunk_coord: None,
            lod_level: 0,
        };
        assert!(output.is_empty());
        assert!(!output.has_transparency());
    }

    #[test]
    fn test_mesh_output_to_glb() {
        use crate::mesher::geometry::{Mesh, Vertex};

        // Build a MeshOutput with a triangle in the opaque layer
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);

        let mesher_output = crate::mesher::MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let output = MeshOutput::from(&mesher_output);
        let glb = output.to_glb().unwrap();
        assert!(!glb.is_empty());
        assert_eq!(&glb[0..4], b"glTF");
    }

    /// Build a minimal `MesherOutput` that contains one greedy material with
    /// tiled UVs spanning `(0..w, 0..h)`. Used by the regression tests below.
    fn build_mesher_output_with_greedy(w: f32, h: f32) -> crate::mesher::MesherOutput {
        use crate::mesher::element::GreedyMaterial;
        use crate::mesher::geometry::{Mesh, Vertex};

        // Greedy quad: tile UVs (0,0)..(w,h), texture = "block/stone"
        let mut greedy_mesh = Mesh::new();
        let g0 = greedy_mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let g1 = greedy_mesh.add_vertex(Vertex::new([w, 0.0, 0.0], [0.0, 1.0, 0.0], [w, 0.0]));
        let g2 = greedy_mesh.add_vertex(Vertex::new([w, 0.0, h], [0.0, 1.0, 0.0], [w, h]));
        let g3 = greedy_mesh.add_vertex(Vertex::new([0.0, 0.0, h], [0.0, 1.0, 0.0], [0.0, h]));
        greedy_mesh.add_quad(g0, g1, g2, g3);

        // Non-greedy atlas-space quad: UVs inside (0..1, 0..1)
        let mut opaque_mesh = Mesh::new();
        let a0 = opaque_mesh.add_vertex(Vertex::new([10.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.1, 0.1]));
        let a1 = opaque_mesh.add_vertex(Vertex::new([11.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.2, 0.1]));
        let a2 = opaque_mesh.add_vertex(Vertex::new([11.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.2, 0.2]));
        let a3 = opaque_mesh.add_vertex(Vertex::new([10.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.1, 0.2]));
        opaque_mesh.add_quad(a0, a1, a2, a3);

        let greedy = GreedyMaterial {
            texture_path: "block/stone".to_string(),
            opaque_mesh: greedy_mesh,
            transparent_mesh: Mesh::new(),
            texture_png: vec![0u8; 32], // non-empty marker; content doesn't matter for UV tests
        };

        crate::mesher::MesherOutput {
            opaque_mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [11.0, 0.0, h.max(1.0)]),
            greedy_materials: vec![greedy],
            animated_textures: Vec::new(),
        }
    }

    /// Regression: `From<&MesherOutput>` must NOT merge greedy-material meshes
    /// (which carry tile-space UVs `[0..w, 0..h]`) into the opaque layer. The
    /// opaque layer is shared with the atlas texture, and tile UVs applied to
    /// an atlas sampler produce the "full atlas smeared across the face"
    /// artefact seen in production GLBs.
    #[test]
    fn greedy_tile_uvs_do_not_leak_into_opaque_layer() {
        let mesher_output = build_mesher_output_with_greedy(5.0, 3.0);
        let output = MeshOutput::from(&mesher_output);

        for &[u, v] in &output.opaque.uvs {
            assert!(
                u <= 1.0 && v <= 1.0,
                "greedy tile UV ({u}, {v}) leaked into opaque atlas layer"
            );
        }
    }

    /// Regression: `MeshOutput` must preserve the greedy_materials across
    /// `From<&MesherOutput>` so the GLB exporter can emit a per-material
    /// primitive (with its own texture and REPEAT sampler) instead of
    /// collapsing greedy geometry into the atlas layer.
    #[test]
    fn greedy_materials_survive_meshoutput_from() {
        let mesher_output = build_mesher_output_with_greedy(2.0, 2.0);
        let output = MeshOutput::from(&mesher_output);

        assert_eq!(
            output.greedy_materials.len(),
            1,
            "greedy_materials lost when converting MesherOutput → MeshOutput"
        );
        assert_eq!(output.greedy_materials[0].texture_path, "block/stone");
    }

    /// Regression: round-tripping through `to_mesher_output` must retain
    /// greedy_materials so `to_glb()` (and all the other export helpers)
    /// produce GLBs with per-greedy primitives.
    #[test]
    fn greedy_materials_survive_mesher_output_roundtrip() {
        let mesher_output = build_mesher_output_with_greedy(2.0, 2.0);
        let output = MeshOutput::from(&mesher_output);
        let roundtripped = output.to_mesher_output();

        assert_eq!(
            roundtripped.greedy_materials.len(),
            1,
            "greedy_materials wiped by MeshOutput::to_mesher_output roundtrip"
        );
        assert_eq!(
            roundtripped.greedy_materials[0].texture_path,
            "block/stone"
        );
    }

    /// Regression: the GLB produced via `MeshOutput::to_glb` must contain
    /// at least one primitive PER greedy material (plus whatever atlas
    /// primitives are needed). A GLB with only the atlas primitive means
    /// greedy geometry got merged into the atlas layer — the bug we fixed.
    #[test]
    fn to_glb_emits_separate_primitive_for_each_greedy_material() {
        let mesher_output = build_mesher_output_with_greedy(2.0, 2.0);
        let output = MeshOutput::from(&mesher_output);
        let glb = output.to_glb().expect("export glb");

        // Parse the GLB JSON chunk to count primitives.
        let json_len =
            u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json_bytes = &glb[20..20 + json_len];
        let root: serde_json::Value =
            serde_json::from_slice(json_bytes).expect("parse glb json");
        let prims = root["meshes"][0]["primitives"]
            .as_array()
            .expect("primitives array");
        assert!(
            prims.len() >= 2,
            "expected ≥2 primitives (atlas + greedy), got {}",
            prims.len()
        );
    }
}
