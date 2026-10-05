//! Wavefront OBJ export.
//!
//! OBJ is a simple, widely-supported text-based 3D format.
//! This exports the mesh with UVs and vertex colors (as comments).

use crate::error::Result;
use crate::mesher::MesherOutput;
use std::fmt::Write;

/// Export a mesh to OBJ format.
/// Returns (obj_content, mtl_content) as strings.
/// Greedy materials get separate MTL entries referencing individual texture files.
pub fn export_obj(output: &MesherOutput, name: &str) -> Result<(String, String)> {
    // Combine atlas-based meshes. Layers are SoA (MeshLayer); the OBJ writer
    // below works on AoS Mesh, so convert at the boundary.
    use crate::mesh_output::layer_to_internal_mesh;
    let mut atlas_mesh = layer_to_internal_mesh(&output.opaque_mesh);
    atlas_mesh.merge(&layer_to_internal_mesh(&output.cutout_mesh));
    atlas_mesh.merge(&layer_to_internal_mesh(&output.transparent_mesh));

    let total_verts = output.total_vertices();
    let total_tris = output.total_triangles();

    // Pre-size buffers: ~60 bytes per vertex line (v/vt/vn) × 3 + ~40 per face
    let obj_capacity = 256 + total_verts * 180 + total_tris * 40;
    let mut obj = String::with_capacity(obj_capacity);
    let mut mtl = String::with_capacity(512);

    // OBJ header
    writeln!(obj, "# Schematic Mesher OBJ Export").unwrap();
    writeln!(obj, "# Vertices: {}", total_verts).unwrap();
    writeln!(obj, "# Triangles: {}", total_tris).unwrap();
    writeln!(obj).unwrap();

    // Reference material file
    writeln!(obj, "mtllib {}.mtl", name).unwrap();
    writeln!(obj).unwrap();

    // Object name
    writeln!(obj, "o {}", name).unwrap();
    writeln!(obj).unwrap();

    // Collect all meshes: atlas mesh first, then greedy materials
    let mut all_meshes: Vec<&crate::mesher::geometry::Mesh> = Vec::new();
    all_meshes.push(&atlas_mesh);
    for gm in &output.greedy_materials {
        if !gm.opaque_mesh.is_empty() {
            all_meshes.push(&gm.opaque_mesh);
        }
        if !gm.transparent_mesh.is_empty() {
            all_meshes.push(&gm.transparent_mesh);
        }
    }

    // Write all vertices, UVs, and normals globally (OBJ has global pools)
    for mesh in &all_meshes {
        for vertex in &mesh.vertices {
            writeln!(
                obj,
                "v {} {} {} {} {} {}",
                vertex.position[0],
                vertex.position[1],
                vertex.position[2],
                vertex.color[0],
                vertex.color[1],
                vertex.color[2]
            )
            .unwrap();
        }
    }
    writeln!(obj).unwrap();

    for mesh in &all_meshes {
        for vertex in &mesh.vertices {
            writeln!(obj, "vt {} {}", vertex.uv[0], vertex.uv[1]).unwrap();
        }
    }
    writeln!(obj).unwrap();

    for mesh in &all_meshes {
        for vertex in &mesh.vertices {
            writeln!(
                obj,
                "vn {} {} {}",
                vertex.normal[0], vertex.normal[1], vertex.normal[2]
            )
            .unwrap();
        }
    }
    writeln!(obj).unwrap();

    // Write faces per material group
    let mut vertex_offset: usize = 0;

    // Atlas material faces
    writeln!(obj, "usemtl {}_material", name).unwrap();
    writeln!(obj).unwrap();
    for i in (0..atlas_mesh.indices.len()).step_by(3) {
        let i0 = atlas_mesh.indices[i] as usize + vertex_offset + 1;
        let i1 = atlas_mesh.indices[i + 1] as usize + vertex_offset + 1;
        let i2 = atlas_mesh.indices[i + 2] as usize + vertex_offset + 1;
        writeln!(
            obj,
            "f {}/{}/{} {}/{}/{} {}/{}/{}",
            i0, i0, i0, i1, i1, i1, i2, i2, i2
        )
        .unwrap();
    }
    vertex_offset += atlas_mesh.vertex_count();

    // Greedy material faces
    for (gi, gm) in output.greedy_materials.iter().enumerate() {
        let mat_name = format!("greedy_{}", gi);
        for (sub_mesh, _is_transparent) in [(&gm.opaque_mesh, false), (&gm.transparent_mesh, true)] {
            if sub_mesh.is_empty() {
                continue;
            }
            writeln!(obj, "usemtl {}", mat_name).unwrap();
            for i in (0..sub_mesh.indices.len()).step_by(3) {
                let i0 = sub_mesh.indices[i] as usize + vertex_offset + 1;
                let i1 = sub_mesh.indices[i + 1] as usize + vertex_offset + 1;
                let i2 = sub_mesh.indices[i + 2] as usize + vertex_offset + 1;
                writeln!(
                    obj,
                    "f {}/{}/{} {}/{}/{} {}/{}/{}",
                    i0, i0, i0, i1, i1, i1, i2, i2, i2
                )
                .unwrap();
            }
            vertex_offset += sub_mesh.vertex_count();
        }
    }

    // MTL file
    writeln!(mtl, "# Schematic Mesher Material").unwrap();
    writeln!(mtl).unwrap();

    // Atlas material
    writeln!(mtl, "newmtl {}_material", name).unwrap();
    writeln!(mtl, "Ka 1.0 1.0 1.0").unwrap();
    writeln!(mtl, "Kd 1.0 1.0 1.0").unwrap();
    writeln!(mtl, "Ks 0.0 0.0 0.0").unwrap();
    writeln!(mtl, "Ns 10.0").unwrap();
    writeln!(mtl, "d 1.0").unwrap();
    writeln!(mtl, "illum 1").unwrap();
    writeln!(mtl, "map_Kd {}_atlas.png", name).unwrap();

    // Greedy materials
    for (gi, gm) in output.greedy_materials.iter().enumerate() {
        writeln!(mtl).unwrap();
        writeln!(mtl, "newmtl greedy_{}", gi).unwrap();
        writeln!(mtl, "Ka 1.0 1.0 1.0").unwrap();
        writeln!(mtl, "Kd 1.0 1.0 1.0").unwrap();
        writeln!(mtl, "Ks 0.0 0.0 0.0").unwrap();
        writeln!(mtl, "Ns 10.0").unwrap();
        writeln!(mtl, "d 1.0").unwrap();
        writeln!(mtl, "illum 1").unwrap();
        // Reference individual texture file
        let tex_filename = gm.texture_path.replace('/', "_");
        writeln!(mtl, "map_Kd {}.png", tex_filename).unwrap();
    }

    Ok((obj, mtl))
}

/// A named texture file for OBJ export.
pub struct ObjTexture {
    /// Filename for this texture (e.g., "block_stone.png").
    pub filename: String,
    /// PNG-encoded texture data.
    pub png_data: Vec<u8>,
}

/// Export mesh and atlas to OBJ format bytes for writing to files.
pub struct ObjExport {
    pub obj: String,
    pub mtl: String,
    pub texture_png: Vec<u8>,
    /// Additional texture files for greedy materials.
    pub greedy_textures: Vec<ObjTexture>,
}

impl ObjExport {
    pub fn from_output(output: &MesherOutput, name: &str) -> Result<Self> {
        let (obj, mtl) = export_obj(output, name)?;
        let texture_png = output.atlas.to_png()?;
        let greedy_textures = output.greedy_materials.iter().map(|gm| {
            let filename = format!("{}.png", gm.texture_path.replace('/', "_"));
            ObjTexture {
                filename,
                png_data: gm.texture_png.clone(),
            }
        }).collect();
        Ok(Self {
            obj,
            mtl,
            texture_png,
            greedy_textures,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    #[test]
    fn test_export_simple_obj() {
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

        let (obj, mtl) = export_obj(&output, "test").unwrap();

        assert!(obj.contains("v 0 0 0"));
        assert!(obj.contains("vt 0 0"));
        assert!(obj.contains("vn 0 1 0"));
        assert!(obj.contains("f 1/1/1 2/2/2 3/3/3"));
        assert!(mtl.contains("newmtl test_material"));
    }
}
