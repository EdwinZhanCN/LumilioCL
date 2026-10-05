//! Mesh export formats.
//!
//! This module provides two ways to export mesh data:
//!
//! ## Standalone Functions (from [`MesherOutput`](crate::mesher::MesherOutput))
//!
//! These operate on the internal `MesherOutput` and support greedy materials natively:
//!
//! - [`export_glb()`] — Binary glTF with quantized vertices and embedded textures
//! - [`export_obj()`] — Wavefront OBJ + MTL + PNG textures
//! - [`export_usda()`] / [`export_usdz()`] — Universal Scene Description (text or archive)
//! - [`export_raw()`] — Raw vertex arrays for custom renderers
//!
//! ## Trait-based API (from [`MeshOutput`](crate::mesh_output::MeshOutput))
//!
//! The [`MeshExporter`] trait provides a uniform interface over the canonical output type:
//!
//! | Exporter | Output Type | Description |
//! |----------|-------------|-------------|
//! | [`GlbExporter`] | `Vec<u8>` | Binary glTF, optional layer merging |
//! | [`ObjExporter`] | [`ObjExport`] | OBJ + MTL text + PNG files |
//! | [`UsdzExporter`] | `Vec<u8>` | USDZ archive for Apple AR |
//!
//! ```ignore
//! use schematic_mesher::export::{MeshExporter, GlbExporter};
//!
//! let exporter = GlbExporter { merge_layers: false };
//! let glb_bytes = exporter.export(&mesh_output)?;
//! ```

pub mod gltf;
pub mod gltf_animated;
pub mod gltf_build;
pub mod obj;
pub mod raw;
pub mod usd;

pub use gltf::export_glb;
pub use gltf_animated::{export_animated_glb, AnimatedPiece};
pub use gltf_build::{
    export_build_glb, mesh_from_layers, BuildChild, BuildNode, BuildScene, Interpolation, Track,
};
pub use obj::{export_obj, ObjExport};
pub use raw::{export_raw, RawMeshData};
pub use usd::{export_usda, export_usdz, UsdTexture, UsdaExport};

use crate::mesh_output::MeshOutput;

/// Trait for exporting a [`MeshOutput`] to a specific format.
///
/// Implementations receive a reference to the canonical mesh output and produce
/// a format-specific result. This allows format selection at runtime while
/// keeping a uniform API:
///
/// ```ignore
/// fn save(exporter: &dyn MeshExporter<Output = Vec<u8>>, mesh: &MeshOutput) -> Result<()> {
///     let bytes = exporter.export(mesh)?;
///     std::fs::write("output.bin", bytes)?;
///     Ok(())
/// }
/// ```
pub trait MeshExporter {
    /// The output type produced by this exporter.
    type Output;

    /// Export the given mesh output.
    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output>;
}

/// GLB (binary glTF) exporter.
///
/// Produces a `Vec<u8>` containing a valid `.glb` file with embedded textures.
/// Uses [`KHR_mesh_quantization`] for ~56% vertex data reduction (positions as
/// i16, normals as i8, colors as u8).
///
/// # Layer Handling
///
/// By default (`merge_layers: false`), emits separate primitives per transparency
/// layer with appropriate alpha modes. Set `merge_layers: true` to flatten
/// everything into a single opaque primitive (loses transparency ordering).
pub struct GlbExporter {
    /// If `true`, merge all layers into a single primitive. If `false` (default),
    /// emit separate primitives per transparency layer for correct rendering.
    pub merge_layers: bool,
}

impl MeshExporter for GlbExporter {
    type Output = Vec<u8>;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        if self.merge_layers {
            // Flatten to single layer, build a temporary MesherOutput with merged mesh
            let flat = mesh.flatten();
            let internal = crate::mesher::MesherOutput {
                opaque_mesh: flat,
                cutout_mesh: crate::mesh_output::MeshLayer::new(),
                transparent_mesh: crate::mesh_output::MeshLayer::new(),
                atlas: mesh.atlas.clone(),
                bounds: mesh.bounds,
                greedy_materials: Vec::new(),
                animated_textures: mesh.animated_textures.clone(),
            };
            gltf::export_glb(&internal)
        } else {
            mesh.to_glb()
        }
    }
}

/// Wavefront OBJ exporter.
///
/// Produces an [`ObjExport`] containing the OBJ text, MTL material library, and
/// texture PNG files. The atlas texture is referenced as `{name}.png` in the MTL.
///
/// OBJ is widely supported by 3D editors (Blender, Maya, 3ds Max) but does not
/// support embedded textures — files must be written to disk alongside each other.
pub struct ObjExporter {
    /// Object name used in the OBJ `o` declaration and MTL `mtllib` reference.
    pub name: String,
}

impl MeshExporter for ObjExporter {
    type Output = ObjExport;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        mesh.to_obj(&self.name)
    }
}

/// USDZ exporter.
///
/// Produces a `Vec<u8>` containing a valid USDZ archive (zero-compression ZIP
/// with 64-byte-aligned assets). This is the format used by Apple AR Quick Look
/// on iOS and macOS.
///
/// The archive contains a USDA text file and PNG texture assets.
pub struct UsdzExporter;

impl MeshExporter for UsdzExporter {
    type Output = Vec<u8>;

    fn export(&self, mesh: &MeshOutput) -> crate::Result<Self::Output> {
        mesh.to_usdz()
    }
}
