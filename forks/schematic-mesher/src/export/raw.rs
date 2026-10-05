//! Raw mesh data export for custom rendering.

use crate::mesher::MesherOutput;

/// Raw mesh data for custom use.
#[derive(Debug)]
pub struct RawMeshData {
    /// Vertex positions (3 floats per vertex).
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (3 floats per vertex).
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates (2 floats per vertex).
    pub uvs: Vec<[f32; 2]>,
    /// Vertex colors (4 floats per vertex, RGBA).
    pub colors: Vec<[f32; 4]>,
    /// Triangle indices (3 per triangle).
    pub indices: Vec<u32>,
    /// Texture atlas RGBA data.
    pub texture_rgba: Vec<u8>,
    /// Texture atlas width.
    pub texture_width: u32,
    /// Texture atlas height.
    pub texture_height: u32,
}

/// Export mesh as raw data.
/// Note: This combines opaque and transparent meshes. For separate handling,
/// access output.opaque_mesh and output.transparent_mesh directly.
pub fn export_raw(output: &MesherOutput) -> RawMeshData {
    // Combine opaque and transparent meshes
    let mesh = output.mesh();
    let atlas = &output.atlas;

    RawMeshData {
        positions: mesh.vertices.iter().map(|v| v.position).collect(),
        normals: mesh.vertices.iter().map(|v| v.normal).collect(),
        uvs: mesh.vertices.iter().map(|v| v.uv).collect(),
        colors: mesh.vertices.iter().map(|v| v.color).collect(),
        indices: mesh.indices.clone(),
        texture_rgba: atlas.pixels.clone(),
        texture_width: atlas.width,
        texture_height: atlas.height,
    }
}

impl RawMeshData {
    /// Get positions as a flat array.
    pub fn positions_flat(&self) -> Vec<f32> {
        self.positions.iter().flat_map(|p| p.iter().copied()).collect()
    }

    /// Get normals as a flat array.
    pub fn normals_flat(&self) -> Vec<f32> {
        self.normals.iter().flat_map(|n| n.iter().copied()).collect()
    }

    /// Get UVs as a flat array.
    pub fn uvs_flat(&self) -> Vec<f32> {
        self.uvs.iter().flat_map(|uv| uv.iter().copied()).collect()
    }

    /// Get colors as a flat array.
    pub fn colors_flat(&self) -> Vec<f32> {
        self.colors.iter().flat_map(|c| c.iter().copied()).collect()
    }

    /// Get the number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    #[test]
    fn test_export_raw() {
        let mut mesh = Mesh::new();
        mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(0, 1, 2);

        let output = MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };

        let raw = export_raw(&output);

        assert_eq!(raw.vertex_count(), 3);
        assert_eq!(raw.triangle_count(), 1);
        assert_eq!(raw.positions.len(), 3);
        assert_eq!(raw.indices.len(), 3);
    }
}
