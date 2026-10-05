//! USD/USDZ export.
//!
//! Generates USDA (ASCII) format manually via string formatting.
//! USDZ is a zero-compression ZIP archive with 64-byte alignment.

use crate::error::{MesherError, Result};
use crate::mesher::geometry::Mesh;
use crate::mesher::MesherOutput;
use std::fmt::Write;
use std::io::Write as IoWrite;

/// A texture referenced by the USDA file.
pub struct UsdTexture {
    /// Filename within the USDZ archive (e.g., "textures/greedy_0.png").
    pub filename: String,
    /// PNG-encoded texture data.
    pub png_data: Vec<u8>,
}

/// Result of USDA generation — text plus referenced textures.
pub struct UsdaExport {
    /// The USDA text content.
    pub usda: String,
    /// Atlas texture PNG data.
    pub atlas_png: Vec<u8>,
    /// Greedy material textures.
    pub greedy_textures: Vec<UsdTexture>,
}

/// Export mesh data as USDA text plus texture files.
pub fn export_usda(output: &MesherOutput) -> Result<UsdaExport> {
    let has_greedy = output
        .greedy_materials
        .iter()
        .any(|gm| !gm.opaque_mesh.is_empty() || !gm.transparent_mesh.is_empty());
    if output.opaque_mesh.is_empty() && output.cutout_mesh.is_empty() && output.transparent_mesh.is_empty() && !has_greedy {
        return Err(MesherError::Export("Cannot export empty mesh".to_string()));
    }

    let atlas_png = output.atlas.to_png()?;

    // Pre-size the output buffer: ~200 bytes per vertex (multiple arrays) + ~10 per index
    let vert_count = output.total_vertices();
    let idx_count = output.opaque_mesh.indices.len() + output.cutout_mesh.indices.len() + output.transparent_mesh.indices.len()
        + output.greedy_materials.iter().map(|gm| gm.opaque_mesh.indices.len() + gm.transparent_mesh.indices.len()).sum::<usize>();
    let estimated_size = 2048 + vert_count * 200 + idx_count * 10;
    let mut usda = String::with_capacity(estimated_size);

    // Header
    writeln!(usda, "#usda 1.0").unwrap();
    writeln!(usda, "(").unwrap();
    writeln!(usda, "    defaultPrim = \"Root\"").unwrap();
    writeln!(usda, "    metersPerUnit = 1").unwrap();
    writeln!(usda, "    upAxis = \"Y\"").unwrap();
    writeln!(usda, ")\n").unwrap();
    writeln!(usda, "def Xform \"Root\"").unwrap();
    writeln!(usda, "{{").unwrap();

    // Atlas materials
    write_material(&mut usda, "atlas_opaque", "textures/atlas.png", "clamp", 1.0);
    write_material(
        &mut usda,
        "atlas_transparent",
        "textures/atlas.png",
        "clamp",
        0.0,
    );

    // Greedy materials
    let mut greedy_textures = Vec::new();
    for (i, gm) in output.greedy_materials.iter().enumerate() {
        if gm.opaque_mesh.is_empty() && gm.transparent_mesh.is_empty() {
            continue;
        }
        let tex_filename = format!("textures/greedy_{}.png", i);
        let mat_name_opaque = format!("greedy_{}_opaque", i);
        let mat_name_transparent = format!("greedy_{}_transparent", i);
        write_material(&mut usda, &mat_name_opaque, &tex_filename, "repeat", 1.0);
        if !gm.transparent_mesh.is_empty() {
            write_material(
                &mut usda,
                &mat_name_transparent,
                &tex_filename,
                "repeat",
                0.0,
            );
        }
        greedy_textures.push(UsdTexture {
            filename: tex_filename,
            png_data: gm.texture_png.clone(),
        });
    }

    // Atlas-based meshes. Output layers are SoA (MeshLayer); the writer works on
    // AoS Mesh, so convert at the boundary.
    use crate::mesh_output::layer_to_internal_mesh;
    if !output.opaque_mesh.is_empty() {
        write_mesh_prim(&mut usda, "opaque", &layer_to_internal_mesh(&output.opaque_mesh), "atlas_opaque");
    }
    if !output.cutout_mesh.is_empty() {
        write_mesh_prim(&mut usda, "cutout", &layer_to_internal_mesh(&output.cutout_mesh), "atlas_opaque");
    }
    if !output.transparent_mesh.is_empty() {
        write_mesh_prim(
            &mut usda,
            "transparent",
            &layer_to_internal_mesh(&output.transparent_mesh),
            "atlas_transparent",
        );
    }

    // Greedy material meshes
    let mut greedy_tex_idx = 0;
    for (i, gm) in output.greedy_materials.iter().enumerate() {
        if gm.opaque_mesh.is_empty() && gm.transparent_mesh.is_empty() {
            continue;
        }
        if !gm.opaque_mesh.is_empty() {
            let prim_name = format!("greedy_{}_opaque", i);
            let mat_name = format!("greedy_{}_opaque", i);
            write_mesh_prim(&mut usda, &prim_name, &gm.opaque_mesh, &mat_name);
        }
        if !gm.transparent_mesh.is_empty() {
            let prim_name = format!("greedy_{}_transparent", i);
            let mat_name = format!("greedy_{}_transparent", i);
            write_mesh_prim(&mut usda, &prim_name, &gm.transparent_mesh, &mat_name);
        }
        greedy_tex_idx += 1;
    }
    let _ = greedy_tex_idx; // suppress unused warning

    // Close Root Xform
    writeln!(usda, "}}").unwrap();

    Ok(UsdaExport {
        usda,
        atlas_png,
        greedy_textures,
    })
}

/// Export mesh data as a USDZ archive (zero-compression ZIP with 64-byte alignment).
pub fn export_usdz(output: &MesherOutput) -> Result<Vec<u8>> {
    let export = export_usda(output)?;

    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);

        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .with_alignment(64);

        // Root USDA file
        zip.start_file("root.usda", options)
            .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;
        zip.write_all(export.usda.as_bytes())
            .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;

        // Atlas texture
        zip.start_file("textures/atlas.png", options)
            .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;
        zip.write_all(&export.atlas_png)
            .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;

        // Greedy textures
        for tex in &export.greedy_textures {
            zip.start_file(&tex.filename, options)
                .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;
            zip.write_all(&tex.png_data)
                .map_err(|e| MesherError::Export(format!("USDZ write error: {}", e)))?;
        }

        zip.finish()
            .map_err(|e| MesherError::Export(format!("USDZ finalize error: {}", e)))?;
    }

    Ok(buf)
}

/// Write a UsdPreviewSurface material definition.
fn write_material(usda: &mut String, name: &str, texture_path: &str, wrap: &str, opacity: f32) {
    writeln!(usda, "    def Material \"{}\"", name).unwrap();
    writeln!(usda, "    {{").unwrap();
    writeln!(
        usda,
        "        token outputs:surface.connect = </Root/{}/shader.outputs:surface>",
        name
    )
    .unwrap();

    // Shader
    writeln!(usda, "        def Shader \"shader\"").unwrap();
    writeln!(usda, "        {{").unwrap();
    writeln!(
        usda,
        "            uniform token info:id = \"UsdPreviewSurface\""
    )
    .unwrap();
    writeln!(
        usda,
        "            color3f inputs:diffuseColor.connect = </Root/{}/diffuse.outputs:rgb>",
        name
    )
    .unwrap();
    writeln!(usda, "            float inputs:metallic = 0").unwrap();
    writeln!(usda, "            float inputs:roughness = 1").unwrap();
    if opacity < 1.0 {
        writeln!(
            usda,
            "            float inputs:opacity.connect = </Root/{}/diffuse.outputs:a>",
            name
        )
        .unwrap();
    } else {
        writeln!(usda, "            float inputs:opacity = 1").unwrap();
    }
    writeln!(usda, "            token outputs:surface").unwrap();
    writeln!(usda, "        }}").unwrap();

    // Texture
    writeln!(usda, "        def Shader \"diffuse\"").unwrap();
    writeln!(usda, "        {{").unwrap();
    writeln!(
        usda,
        "            uniform token info:id = \"UsdUVTexture\""
    )
    .unwrap();
    writeln!(
        usda,
        "            asset inputs:file = @{}@",
        texture_path
    )
    .unwrap();
    writeln!(
        usda,
        "            float2 inputs:st.connect = </Root/{}/st.outputs:result>",
        name
    )
    .unwrap();
    writeln!(
        usda,
        "            token inputs:wrapS = \"{}\"",
        wrap
    )
    .unwrap();
    writeln!(
        usda,
        "            token inputs:wrapT = \"{}\"",
        wrap
    )
    .unwrap();
    writeln!(usda, "            float3 outputs:rgb").unwrap();
    if opacity < 1.0 {
        writeln!(usda, "            float outputs:a").unwrap();
    }
    writeln!(usda, "        }}").unwrap();

    // Primvar reader
    writeln!(usda, "        def Shader \"st\"").unwrap();
    writeln!(usda, "        {{").unwrap();
    writeln!(
        usda,
        "            uniform token info:id = \"UsdPrimvarReader_float2\""
    )
    .unwrap();
    writeln!(
        usda,
        "            string inputs:varname = \"st\""
    )
    .unwrap();
    writeln!(usda, "            float2 outputs:result").unwrap();
    writeln!(usda, "        }}").unwrap();

    writeln!(usda, "    }}\n").unwrap();
}

/// Write a comma-separated array inline, streaming values directly to the buffer.
fn write_array_inline<T, F>(usda: &mut String, items: &[T], mut fmt: F)
where
    F: FnMut(&T, &mut String),
{
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            usda.push_str(", ");
        }
        fmt(item, usda);
    }
}

/// Write a Mesh prim with positions, normals, UVs, vertex colors, and material binding.
fn write_mesh_prim(usda: &mut String, name: &str, mesh: &Mesh, material: &str) {
    writeln!(usda, "    def Mesh \"{}\"", name).unwrap();
    writeln!(usda, "    {{").unwrap();

    // Face vertex counts — all triangles (stream "3" values directly)
    let tri_count = mesh.triangle_count();
    write!(usda, "        int[] faceVertexCounts = [").unwrap();
    for i in 0..tri_count {
        if i > 0 {
            usda.push_str(", ");
        }
        usda.push('3');
    }
    writeln!(usda, "]").unwrap();

    // Face vertex indices
    write!(usda, "        int[] faceVertexIndices = [").unwrap();
    write_array_inline(usda, &mesh.indices, |i, s| {
        write!(s, "{}", i).unwrap();
    });
    writeln!(usda, "]").unwrap();

    // Points
    write!(usda, "        point3f[] points = [").unwrap();
    write_array_inline(usda, &mesh.vertices, |v, s| {
        write!(s, "({}, {}, {})", v.position[0], v.position[1], v.position[2]).unwrap();
    });
    writeln!(usda, "]").unwrap();

    // Normals (vertex interpolation)
    write!(usda, "        normal3f[] normals = [").unwrap();
    write_array_inline(usda, &mesh.vertices, |v, s| {
        write!(s, "({}, {}, {})", v.normal[0], v.normal[1], v.normal[2]).unwrap();
    });
    writeln!(usda, "] (").unwrap();
    writeln!(usda, "            interpolation = \"vertex\"").unwrap();
    writeln!(usda, "        )").unwrap();

    // UVs (vertex interpolation)
    write!(usda, "        texCoord2f[] primvars:st = [").unwrap();
    write_array_inline(usda, &mesh.vertices, |v, s| {
        write!(s, "({}, {})", v.uv[0], v.uv[1]).unwrap();
    });
    writeln!(usda, "] (").unwrap();
    writeln!(usda, "            interpolation = \"vertex\"").unwrap();
    writeln!(usda, "        )").unwrap();

    // Vertex colors
    let has_non_white = mesh.vertices.iter().any(|v| {
        v.color[0] != 1.0 || v.color[1] != 1.0 || v.color[2] != 1.0 || v.color[3] != 1.0
    });
    if has_non_white {
        write!(usda, "        color3f[] primvars:displayColor = [").unwrap();
        write_array_inline(usda, &mesh.vertices, |v, s| {
            write!(s, "({}, {}, {})", v.color[0], v.color[1], v.color[2]).unwrap();
        });
        writeln!(usda, "] (").unwrap();
        writeln!(usda, "            interpolation = \"vertex\"").unwrap();
        writeln!(usda, "        )").unwrap();

        let has_non_opaque = mesh.vertices.iter().any(|v| v.color[3] != 1.0);
        if has_non_opaque {
            write!(usda, "        float[] primvars:displayOpacity = [").unwrap();
            write_array_inline(usda, &mesh.vertices, |v, s| {
                write!(s, "{}", v.color[3]).unwrap();
            });
            writeln!(usda, "] (").unwrap();
            writeln!(usda, "            interpolation = \"vertex\"").unwrap();
            writeln!(usda, "        )").unwrap();
        }
    }

    // Material binding
    writeln!(
        usda,
        "        rel material:binding = </Root/{}>",
        material
    )
    .unwrap();

    writeln!(usda, "    }}\n").unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::TextureAtlas;
    use crate::mesher::geometry::{Mesh, Vertex};
    use crate::types::BoundingBox;

    fn make_triangle_output() -> MesherOutput {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]));
        let v1 = mesh.add_vertex(Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]));
        let v2 = mesh.add_vertex(Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]));
        mesh.add_triangle(v0, v1, v2);
        MesherOutput {
            opaque_mesh: mesh,
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 0.0, 1.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        }
    }

    #[test]
    fn test_export_usda_header() {
        let output = make_triangle_output();
        let export = export_usda(&output).unwrap();
        assert!(export.usda.starts_with("#usda 1.0"));
        assert!(export.usda.contains("defaultPrim = \"Root\""));
        assert!(export.usda.contains("upAxis = \"Y\""));
    }

    #[test]
    fn test_export_usda_has_mesh() {
        let output = make_triangle_output();
        let export = export_usda(&output).unwrap();
        assert!(export.usda.contains("def Mesh \"opaque\""));
        assert!(export.usda.contains("faceVertexCounts = [3]"));
        assert!(export.usda.contains("faceVertexIndices = [0, 1, 2]"));
        assert!(export.usda.contains("point3f[] points"));
    }

    #[test]
    fn test_export_usda_has_material() {
        let output = make_triangle_output();
        let export = export_usda(&output).unwrap();
        assert!(export.usda.contains("def Material \"atlas_opaque\""));
        assert!(export.usda.contains("UsdPreviewSurface"));
        assert!(export.usda.contains("UsdUVTexture"));
        assert!(export.usda.contains("@textures/atlas.png@"));
    }

    #[test]
    fn test_export_usda_empty_fails() {
        let output = MesherOutput {
            opaque_mesh: Mesh::new(),
            cutout_mesh: Mesh::new(),
            transparent_mesh: Mesh::new(),
            atlas: TextureAtlas::empty(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
        };
        assert!(export_usda(&output).is_err());
    }

    #[test]
    fn test_export_usda_transparent_only() {
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
        let export = export_usda(&output).unwrap();
        assert!(export.usda.contains("def Mesh \"transparent\""));
        assert!(!export.usda.contains("def Mesh \"opaque\""));
        assert!(export.usda.contains("atlas_transparent"));
    }

    #[test]
    fn test_export_usda_vertex_colors() {
        let mut mesh = Mesh::new();
        let v0 = mesh.add_vertex(
            Vertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0])
                .with_color([0.5, 0.6, 0.7, 1.0]),
        );
        let v1 = mesh.add_vertex(
            Vertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0])
                .with_color([0.5, 0.6, 0.7, 1.0]),
        );
        let v2 = mesh.add_vertex(
            Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0])
                .with_color([0.5, 0.6, 0.7, 1.0]),
        );
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
        let export = export_usda(&output).unwrap();
        assert!(export.usda.contains("primvars:displayColor"));
    }

    #[test]
    fn test_export_usdz_is_valid_zip() {
        let output = make_triangle_output();
        let usdz = export_usdz(&output).unwrap();
        // USDZ is a ZIP — check for PK magic bytes
        assert_eq!(&usdz[0..2], b"PK");
        // Verify it can be read as a ZIP
        let cursor = std::io::Cursor::new(&usdz);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut found_usda = false;
        let mut found_atlas = false;
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            match file.name() {
                "root.usda" => found_usda = true,
                "textures/atlas.png" => found_atlas = true,
                _ => {}
            }
            // Verify stored (no compression)
            assert_eq!(file.compression(), zip::CompressionMethod::Stored);
        }
        assert!(found_usda, "USDZ missing root.usda");
        assert!(found_atlas, "USDZ missing textures/atlas.png");
    }

    #[test]
    fn test_export_usdz_alignment() {
        let output = make_triangle_output();
        let usdz = export_usdz(&output).unwrap();
        let cursor = std::io::Cursor::new(&usdz);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        for i in 0..archive.len() {
            let file = archive.by_index(i).unwrap();
            let offset = file.data_start();
            assert_eq!(
                offset % 64,
                0,
                "File '{}' data not 64-byte aligned (offset={})",
                file.name(),
                offset
            );
        }
    }
}
