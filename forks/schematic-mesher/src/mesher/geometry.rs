//! Mesh geometry types.

/// A vertex in the output mesh.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Position in 3D space.
    pub position: [f32; 3],
    /// Normal vector.
    pub normal: [f32; 3],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Vertex color (RGBA).
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], uv: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            uv,
            color: [1.0, 1.0, 1.0, 1.0], // White by default
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }
}

impl Default for Vertex {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// A triangle mesh.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Vertex data.
    pub vertices: Vec<Vertex>,
    /// Triangle indices (3 per triangle).
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, vertex: Vertex) -> u32 {
        let index = self.vertices.len() as u32;
        self.vertices.push(vertex);
        index
    }

    /// Add a triangle by vertex indices.
    pub fn add_triangle(&mut self, i0: u32, i1: u32, i2: u32) {
        self.indices.push(i0);
        self.indices.push(i1);
        self.indices.push(i2);
    }

    /// Add a quad (two triangles) by vertex indices.
    /// Vertices are provided in order around the quad. Triangles are wound CCW for front-facing.
    pub fn add_quad(&mut self, i0: u32, i1: u32, i2: u32, i3: u32) {
        // Reverse winding for CCW front faces (glTF standard)
        // First triangle: 0, 2, 1
        self.add_triangle(i0, i2, i1);
        // Second triangle: 0, 3, 2
        self.add_triangle(i0, i3, i2);
    }

    /// Add a quad with AO-aware triangulation to fix anisotropy.
    /// Uses the diagonal that minimizes interpolation artifacts.
    pub fn add_quad_ao(&mut self, i0: u32, i1: u32, i2: u32, i3: u32, ao: [u8; 4]) {
        // Compare diagonal sums to choose triangulation
        // See: https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/
        if ao[0] as u16 + ao[2] as u16 > ao[1] as u16 + ao[3] as u16 {
            // Flip: use 1-3 diagonal instead of 0-2
            // Triangles: (1, 3, 0) and (1, 2, 3) with CCW winding
            self.add_triangle(i1, i0, i3);
            self.add_triangle(i1, i3, i2);
        } else {
            // Normal: use 0-2 diagonal
            self.add_triangle(i0, i2, i1);
            self.add_triangle(i0, i3, i2);
        }
    }

    /// Get the number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Get the number of vertices.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Check if the mesh is empty.
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Merge another mesh into this one.
    pub fn merge(&mut self, other: &Mesh) {
        let offset = self.vertices.len() as u32;

        self.vertices.extend_from_slice(&other.vertices);

        for index in &other.indices {
            self.indices.push(index + offset);
        }
    }

    /// Translate all vertices by an offset.
    pub fn translate(&mut self, offset: [f32; 3]) {
        for vertex in &mut self.vertices {
            vertex.position[0] += offset[0];
            vertex.position[1] += offset[1];
            vertex.position[2] += offset[2];
        }
    }

    /// Get positions as a flat array (for glTF export).
    pub fn positions_flat(&self) -> Vec<f32> {
        self.vertices
            .iter()
            .flat_map(|v| v.position)
            .collect()
    }

    /// Get normals as a flat array (for glTF export).
    pub fn normals_flat(&self) -> Vec<f32> {
        self.vertices
            .iter()
            .flat_map(|v| v.normal)
            .collect()
    }

    /// Get UVs as a flat array (for glTF export).
    pub fn uvs_flat(&self) -> Vec<f32> {
        self.vertices
            .iter()
            .flat_map(|v| v.uv)
            .collect()
    }

    /// Get colors as a flat array (for glTF export).
    pub fn colors_flat(&self) -> Vec<f32> {
        self.vertices
            .iter()
            .flat_map(|v| v.color)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation() {
        let mut mesh = Mesh::new();
        assert!(mesh.is_empty());

        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([1.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 1.0]));

        mesh.add_triangle(v0, v1, v2);

        assert_eq!(mesh.vertex_count(), 3);
        assert_eq!(mesh.triangle_count(), 1);
    }

    #[test]
    fn test_mesh_quad() {
        let mut mesh = Mesh::new();

        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([1.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 1.0]));
        let v3 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));

        mesh.add_quad(v0, v1, v2, v3);

        assert_eq!(mesh.vertex_count(), 4);
        assert_eq!(mesh.triangle_count(), 2);
        // CCW winding: (0,2,1) and (0,3,2)
        assert_eq!(mesh.indices, vec![0, 2, 1, 0, 3, 2]);
    }

    #[test]
    fn test_mesh_merge() {
        let mut mesh1 = Mesh::new();
        let v0 = mesh1.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh1.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh1.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh1.add_triangle(v0, v1, v2);

        let mut mesh2 = Mesh::new();
        let v0 = mesh2.add_vertex(Vertex::new([2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh2.add_vertex(Vertex::new([3.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh2.add_vertex(Vertex::new([2.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh2.add_triangle(v0, v1, v2);

        mesh1.merge(&mesh2);

        assert_eq!(mesh1.vertex_count(), 6);
        assert_eq!(mesh1.triangle_count(), 2);
        // Second triangle indices should be offset by 3
        assert_eq!(mesh1.indices, vec![0, 1, 2, 3, 4, 5]);
    }
}
