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
pub use error::{BakeError, ContainerError};
pub use names::{BlockNames, NAME_LANGUAGES, NAMES_FORMAT, names_in_lang_file};
pub use pack::{
    Animation, BakedAssets, BakedBlockstate, BakedCase, BakedElement, BakedFace, BakedModel,
    BakedPack, BakedTexture, BakedVariant, Condition, ContentHash, Direction, DisplayTransform,
    Image, PackMeta, PaintingVariant, ParentModel, Rotation, SourceKind, SourceStamp, namespaced,
};
pub use source::AssetSource;

#[cfg(test)]
mod tests;

mod error;
