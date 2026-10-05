//! Resource pack loading and parsing.
//!
//! This module handles loading Minecraft resource packs (ZIP files or directories)
//! and parsing their contents including blockstates, models, and textures.

pub mod loader;
pub mod blockstate;
pub mod model;
pub mod texture;

pub use blockstate::{BlockstateDefinition, ModelVariant, MultipartCase, MultipartCondition};
pub use model::{BlockModel, ModelElement, ModelFace};
pub use texture::{TextureData, AnimationMeta, AnimFrame};

use std::collections::HashMap;

/// A loaded Minecraft resource pack.
#[derive(Debug, Default, Clone)]
pub struct ResourcePack {
    /// Blockstate definitions by namespace and block ID.
    /// Key: namespace (e.g., "minecraft"), Value: map of block_id to definition.
    pub blockstates: HashMap<String, HashMap<String, BlockstateDefinition>>,

    /// Model definitions by namespace and model path.
    /// Key: namespace, Value: map of model_path to model.
    pub models: HashMap<String, HashMap<String, BlockModel>>,

    /// Texture data by namespace and texture path.
    /// Key: namespace, Value: map of texture_path to data.
    pub textures: HashMap<String, HashMap<String, TextureData>>,
}

impl ResourcePack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a blockstate definition by full resource location (e.g., "minecraft:stone").
    pub fn get_blockstate(&self, resource_location: &str) -> Option<&BlockstateDefinition> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.blockstates
            .get(namespace)
            .and_then(|ns| ns.get(path))
    }

    /// Get a model by full resource location (e.g., "minecraft:block/stone").
    pub fn get_model(&self, resource_location: &str) -> Option<&BlockModel> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.models.get(namespace).and_then(|ns| ns.get(path))
    }

    /// Get a texture by full resource location (e.g., "minecraft:block/stone").
    pub fn get_texture(&self, resource_location: &str) -> Option<&TextureData> {
        let (namespace, path) = parse_resource_location(resource_location);
        self.textures.get(namespace).and_then(|ns| ns.get(path))
    }

    /// Add a blockstate definition.
    pub fn add_blockstate(
        &mut self,
        namespace: &str,
        block_id: &str,
        definition: BlockstateDefinition,
    ) {
        self.blockstates
            .entry(namespace.to_string())
            .or_default()
            .insert(block_id.to_string(), definition);
    }

    /// Add a model.
    pub fn add_model(&mut self, namespace: &str, model_path: &str, model: BlockModel) {
        self.models
            .entry(namespace.to_string())
            .or_default()
            .insert(model_path.to_string(), model);
    }

    /// Add a texture.
    pub fn add_texture(&mut self, namespace: &str, texture_path: &str, texture: TextureData) {
        self.textures
            .entry(namespace.to_string())
            .or_default()
            .insert(texture_path.to_string(), texture);
    }

    /// Get the total number of blockstate definitions.
    pub fn blockstate_count(&self) -> usize {
        self.blockstates.values().map(|m| m.len()).sum()
    }

    /// Get the total number of models.
    pub fn model_count(&self) -> usize {
        self.models.values().map(|m| m.len()).sum()
    }

    /// Get the total number of textures.
    pub fn texture_count(&self) -> usize {
        self.textures.values().map(|m| m.len()).sum()
    }

    /// Get all namespaces in the resource pack.
    pub fn namespaces(&self) -> Vec<&str> {
        let mut namespaces: Vec<_> = self.blockstates.keys()
            .chain(self.models.keys())
            .chain(self.textures.keys())
            .map(|s| s.as_str())
            .collect();
        namespaces.sort();
        namespaces.dedup();
        namespaces
    }

    /// Overlay `higher` on top of this pack. Entries in `higher` replace
    /// entries in `self` on per-key collision (blockstate id, model path,
    /// texture path). Namespaces present only in one side are preserved
    /// as-is. Mirrors Minecraft's resource-pack priority model where packs
    /// loaded later override packs loaded earlier.
    pub fn overlay(&mut self, higher: ResourcePack) {
        let ResourcePack { blockstates, models, textures } = higher;

        for (ns, entries) in blockstates {
            self.blockstates.entry(ns).or_default().extend(entries);
        }
        for (ns, entries) in models {
            self.models.entry(ns).or_default().extend(entries);
        }
        for (ns, entries) in textures {
            self.textures.entry(ns).or_default().extend(entries);
        }
    }
}

/// Parse a resource location into namespace and path.
/// "minecraft:block/stone" -> ("minecraft", "block/stone")
/// "block/stone" -> ("minecraft", "block/stone")
fn parse_resource_location(resource_location: &str) -> (&str, &str) {
    if let Some((namespace, path)) = resource_location.split_once(':') {
        (namespace, path)
    } else {
        ("minecraft", resource_location)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_resource_location() {
        assert_eq!(
            parse_resource_location("minecraft:block/stone"),
            ("minecraft", "block/stone")
        );
        assert_eq!(
            parse_resource_location("mymod:block/custom"),
            ("mymod", "block/custom")
        );
        assert_eq!(
            parse_resource_location("block/stone"),
            ("minecraft", "block/stone")
        );
    }

    fn solid_color_texture(r: u8, g: u8, b: u8, a: u8) -> TextureData {
        TextureData::new(1, 1, vec![r, g, b, a])
    }

    fn empty_blockstate() -> BlockstateDefinition {
        BlockstateDefinition::Variants(HashMap::new())
    }

    fn empty_model() -> BlockModel {
        BlockModel::default()
    }

    #[test]
    fn overlay_empty_onto_pack_is_identity() {
        let mut base = ResourcePack::new();
        base.add_texture("minecraft", "block/stone", solid_color_texture(10, 20, 30, 255));
        base.add_model("minecraft", "block/stone", empty_model());
        base.add_blockstate("minecraft", "stone", empty_blockstate());

        base.overlay(ResourcePack::new());

        assert_eq!(base.texture_count(), 1);
        assert_eq!(base.model_count(), 1);
        assert_eq!(base.blockstate_count(), 1);
        let tex = base.get_texture("minecraft:block/stone").unwrap();
        assert_eq!(tex.pixels, vec![10, 20, 30, 255]);
    }

    #[test]
    fn overlay_pack_onto_empty_copies_content() {
        let mut base = ResourcePack::new();
        let mut higher = ResourcePack::new();
        higher.add_texture("minecraft", "block/stone", solid_color_texture(5, 6, 7, 255));

        base.overlay(higher);
        assert_eq!(base.texture_count(), 1);
        assert_eq!(
            base.get_texture("minecraft:block/stone").unwrap().pixels,
            vec![5, 6, 7, 255]
        );
    }

    #[test]
    fn overlay_higher_pack_wins_on_collision() {
        let mut base = ResourcePack::new();
        base.add_texture("minecraft", "block/stone", solid_color_texture(10, 20, 30, 255));

        let mut higher = ResourcePack::new();
        higher.add_texture("minecraft", "block/stone", solid_color_texture(250, 0, 0, 255));

        base.overlay(higher);
        assert_eq!(base.texture_count(), 1);
        assert_eq!(
            base.get_texture("minecraft:block/stone").unwrap().pixels,
            vec![250, 0, 0, 255]
        );
    }

    #[test]
    fn overlay_preserves_non_colliding_entries() {
        let mut base = ResourcePack::new();
        base.add_texture("minecraft", "block/stone", solid_color_texture(10, 10, 10, 255));

        let mut higher = ResourcePack::new();
        higher.add_texture("minecraft", "block/dirt", solid_color_texture(120, 72, 0, 255));
        higher.add_texture("mymod", "block/custom", solid_color_texture(1, 2, 3, 255));

        base.overlay(higher);
        assert_eq!(base.texture_count(), 3);
        assert!(base.get_texture("minecraft:block/stone").is_some());
        assert!(base.get_texture("minecraft:block/dirt").is_some());
        assert!(base.get_texture("mymod:block/custom").is_some());
    }

    #[test]
    fn overlay_merges_all_three_categories() {
        let mut base = ResourcePack::new();
        base.add_blockstate("minecraft", "stone", empty_blockstate());
        base.add_model("minecraft", "block/stone", empty_model());
        base.add_texture("minecraft", "block/stone", solid_color_texture(1, 1, 1, 255));

        let mut higher = ResourcePack::new();
        higher.add_blockstate("minecraft", "dirt", empty_blockstate());
        higher.add_model("minecraft", "block/dirt", empty_model());
        higher.add_texture("minecraft", "block/dirt", solid_color_texture(2, 2, 2, 255));

        base.overlay(higher);
        assert_eq!(base.blockstate_count(), 2);
        assert_eq!(base.model_count(), 2);
        assert_eq!(base.texture_count(), 2);
    }

    #[test]
    fn load_resource_packs_applies_in_order() {
        // Simulate layered loading manually (without hitting the filesystem):
        // the deterministic test is that overlay is applied in iteration order.
        let mut low = ResourcePack::new();
        low.add_texture("minecraft", "block/stone", solid_color_texture(10, 10, 10, 255));

        let mut mid = ResourcePack::new();
        mid.add_texture("minecraft", "block/stone", solid_color_texture(20, 20, 20, 255));

        let mut high = ResourcePack::new();
        high.add_texture("minecraft", "block/stone", solid_color_texture(30, 30, 30, 255));

        let mut merged = ResourcePack::new();
        for pack in [low, mid, high] {
            merged.overlay(pack);
        }

        // Highest priority applied last → its pixels survive.
        assert_eq!(
            merged.get_texture("minecraft:block/stone").unwrap().pixels,
            vec![30, 30, 30, 255]
        );
    }
}
