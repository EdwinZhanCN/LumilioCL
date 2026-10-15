//! Baked block assets (plan `schematic-studio`, D1).
//!
//! The baker is the only code that knows Mojang's resource layout and how it
//! changed between game versions: texture variables written as objects,
//! `bottom`/`top` face names, numeric multipart conditions, moved entity
//! textures, parent models. It reads client jars, mod jars and resource packs
//! and writes a [`BakedPack`] in which all of that is settled, so renderers
//! read one shape for every version from 1.13.2 on.

mod bake;
mod block_items;
mod catalog;
mod container;
mod entity;
mod names;
mod pack;
mod source;

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::Path;

pub use bake::{bake, bake_over, canonical_texture};
pub use block_items::{icon_item, icon_tint, table_items};
pub use catalog::{BlockCatalog, Values as CatalogValues};
pub use container::{Bundle, FORMAT as BUNDLE_FORMAT, write_bundle};
pub use entity::{
    Bone, Cube, DUMP_FORMAT, DUMPER_CLASS, DUMPER_CLASS_FILE, DUMPER_JAVA, DUMPER_MAIN,
    ENTITY_FORMAT, EntityBundle, EntityDump, EntityLayer, EntityLayers, EntityLook, EntitySource,
    EntityVersion, LAYER_DEFINITIONS_CLASS, borrow_layers, borrow_textures, borrowed_looks,
    dumper_arguments, dumper_failure, install_dumper, read_dump, write_entities,
};
pub use names::{BlockNames, NAME_LANGUAGES, NAMES_FORMAT, names_in_lang_file};
pub use pack::{
    Animation, BakedAssets, BakedBlockstate, BakedCase, BakedElement, BakedFace, BakedModel,
    BakedPack, BakedTexture, BakedVariant, Condition, ContentHash, Direction, DisplayTransform,
    Image, PackMeta, PaintingVariant, ParentModel, Rotation, SourceKind, SourceStamp, namespaced,
};
pub use source::AssetSource;

/// A source could not be read at all. Single unreadable entries do not fail
/// a bake; they are listed in [`PackMeta::skipped`].
#[derive(Debug)]
pub enum BakeError {
    Io {
        path: String,
        error: std::io::Error,
    },
    /// The archive is not a zip, or is larger than any resource pack.
    Archive(String),
}

impl BakeError {
    fn io(path: &Path, error: std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            error,
        }
    }

    fn at(self, path: &Path) -> Self {
        match self {
            Self::Archive(reason) => Self::Archive(format!("{}: {reason}", path.display())),
            other => other,
        }
    }
}

impl Display for BakeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{path}: {error}"),
            Self::Archive(reason) => write!(f, "not a readable resource archive: {reason}"),
        }
    }
}

impl Error for BakeError {}

/// A baked file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    Io(String),
    /// Not a zstd frame holding a baked container.
    NotABundle,
    /// Written by a newer launcher.
    UnsupportedFormat(u32),
    Corrupt(String),
    MissingPack(String),
}

impl Display for ContainerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(reason) => write!(f, "baked block assets: {reason}"),
            Self::NotABundle => f.write_str("the file is not a baked block-asset file"),
            Self::UnsupportedFormat(format) => write!(
                f,
                "the baked block-asset file uses format {format}, which this launcher cannot read"
            ),
            Self::Corrupt(reason) => write!(f, "the baked block-asset file is damaged: {reason}"),
            Self::MissingPack(key) => write!(f, "the baked block-asset file has no pack {key}"),
        }
    }
}

impl Error for ContainerError {}

#[cfg(test)]
mod tests;
