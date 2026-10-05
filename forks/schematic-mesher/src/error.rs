//! Error types for the schematic mesher.

use thiserror::Error;

/// Result type alias using MesherError.
pub type Result<T> = std::result::Result<T, MesherError>;

/// Main error type for schematic meshing operations.
#[derive(Error, Debug)]
pub enum MesherError {
    /// Failed to read or parse a ZIP archive.
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// Failed to parse JSON data.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Failed to read or process an image.
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    /// I/O error during file operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Resource not found in the resource pack.
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// Invalid resource pack structure.
    #[error("Invalid resource pack: {0}")]
    InvalidResourcePack(String),

    /// Failed to resolve a block model.
    #[error("Model resolution error: {0}")]
    ModelResolution(String),

    /// Failed to resolve a blockstate.
    #[error("Blockstate resolution error: {0}")]
    BlockstateResolution(String),

    /// Texture reference could not be resolved.
    #[error("Unresolved texture reference: {0}")]
    UnresolvedTexture(String),

    /// Model inheritance chain too deep (circular reference protection).
    #[error("Model inheritance too deep (possible circular reference): {0}")]
    ModelInheritanceTooDeep(String),

    /// Failed to build texture atlas.
    #[error("Atlas building error: {0}")]
    AtlasBuild(String),

    /// Failed to export mesh.
    #[error("Export error: {0}")]
    Export(String),
}
