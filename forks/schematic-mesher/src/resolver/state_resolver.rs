//! Block state to model variant resolution.

use crate::error::{MesherError, Result};
use crate::resource_pack::{
    blockstate::build_property_string, BlockstateDefinition, ModelVariant, ResourcePack,
};
use crate::types::InputBlock;

/// Resolves block states to model variants.
pub struct StateResolver<'a> {
    pack: &'a ResourcePack,
}

impl<'a> StateResolver<'a> {
    pub fn new(pack: &'a ResourcePack) -> Self {
        Self { pack }
    }

    /// Resolve a block to its model variants.
    pub fn resolve(&self, block: &InputBlock) -> Result<Vec<ModelVariant>> {
        // Get the blockstate definition
        let blockstate = self.pack.get_blockstate(&block.name).ok_or_else(|| {
            MesherError::BlockstateResolution(format!(
                "No blockstate found for {}",
                block.name
            ))
        })?;

        match blockstate {
            BlockstateDefinition::Variants(variants) => {
                self.resolve_variants(variants, block)
            }
            BlockstateDefinition::Multipart(cases) => {
                self.resolve_multipart(cases, block)
            }
        }
    }

    /// Resolve using the variants format.
    fn resolve_variants(
        &self,
        variants: &std::collections::HashMap<String, Vec<ModelVariant>>,
        block: &InputBlock,
    ) -> Result<Vec<ModelVariant>> {
        // Build the property string to look up
        let prop_string = build_property_string(&block.properties);

        // Try exact match first
        if let Some(variant_list) = variants.get(&prop_string) {
            return Ok(vec![variant_list[0].clone()]);
        }

        // Try empty string (default variant)
        if let Some(variant_list) = variants.get("") {
            return Ok(vec![variant_list[0].clone()]);
        }

        // Find all variants that match the user's specified properties
        // (user properties are a subset of variant properties)
        let matching_variants: Vec<_> = variants
            .iter()
            .filter(|(key, _)| self.user_properties_match_variant(key, &block.properties))
            .collect();

        if !matching_variants.is_empty() {
            // Among matching variants, pick the one with the best default score
            // for the properties NOT specified by the user
            let best = matching_variants
                .into_iter()
                .max_by_key(|(key, _)| self.calculate_default_score_for_unspecified(key, &block.properties))
                .unwrap();
            return Ok(vec![best.1[0].clone()]);
        }

        // Last resort: find the most "default-like" variant overall
        if let Some((_, variant_list)) = self.find_default_variant(variants) {
            return Ok(vec![variant_list[0].clone()]);
        }

        Err(MesherError::BlockstateResolution(format!(
            "No matching variant for {} with properties {:?}",
            block.name, block.properties
        )))
    }

    /// Find the most "default-like" variant when no properties are specified.
    /// Prefers variants with values like 0, false, none, north, bottom, etc.
    fn find_default_variant<'b>(
        &self,
        variants: &'b std::collections::HashMap<String, Vec<ModelVariant>>,
    ) -> Option<(&'b String, &'b Vec<ModelVariant>)> {
        let mut best_key: Option<&String> = None;
        let mut best_score = i32::MIN;

        for key in variants.keys() {
            let score = self.calculate_default_score(key);
            if score > best_score {
                best_score = score;
                best_key = Some(key);
            }
        }

        best_key.and_then(|k| variants.get_key_value(k))
    }

    /// Calculate a "default-ness" score for a variant key.
    /// Higher scores indicate more default-like values.
    fn calculate_default_score(&self, key: &str) -> i32 {
        if key.is_empty() {
            return i32::MAX; // Empty key is the most default
        }

        let mut score = 0;

        for pair in key.split(',') {
            if let Some((prop, value)) = pair.split_once('=') {
                score += self.value_default_score(prop, value);
            }
        }

        score
    }

    /// Score how "default-like" a property value is.
    fn value_default_score(&self, property: &str, value: &str) -> i32 {
        // Numeric properties: lower is more default (power=0 > power=15)
        if let Ok(num) = value.parse::<i32>() {
            return -num * 10; // Prefer 0 over higher numbers
        }

        // Property-specific defaults
        match property {
            "axis" => match value {
                "y" => return 50,  // Y is default for logs, pillars
                _ => return 0,
            },
            "waterlogged" | "powered" | "open" | "lit" | "enabled" |
            "triggered" | "inverted" | "extended" | "locked" | "attached" |
            "disarmed" | "occupied" | "has_record" | "has_book" | "signal_fire" |
            "hanging" | "persistent" | "unstable" | "bottom" | "drag" |
            "eye" | "in_wall" | "snowy" | "up" | "conditional" => {
                match value {
                    "false" => return 100,
                    "true" => return -100,
                    _ => return 0,
                }
            }
            "half" => match value {
                "bottom" | "lower" => return 50,
                "top" | "upper" => return -50,
                _ => return 0,
            },
            "type" => match value {
                "single" | "normal" | "bottom" => return 50,
                "double" | "top" => return -50,
                _ => return 0,
            },
            "facing" => match value {
                "north" => return 50,
                "south" => return 40,
                "east" => return 30,
                "west" => return 20,
                "up" => return 10,
                "down" => return 0,
                _ => return 0,
            },
            "shape" => match value {
                "straight" => return 50,
                "ascending_north" | "ascending_south" | "ascending_east" | "ascending_west" => return 0,
                _ => return -20,
            },
            // Connection properties (fences, walls, redstone)
            "north" | "south" | "east" | "west" => match value {
                "none" | "false" => return 50,
                "low" | "side" => return 0,
                "tall" | "up" => return -20,
                "true" => return -50,
                _ => return 0,
            },
            _ => {}
        }

        // Generic value defaults
        match value {
            "false" | "off" | "none" | "0" => 100,
            "true" | "on" => -100,
            _ => 0,
        }
    }

    /// Resolve using the multipart format.
    fn resolve_multipart(
        &self,
        cases: &[crate::resource_pack::MultipartCase],
        block: &InputBlock,
    ) -> Result<Vec<ModelVariant>> {
        let mut result = Vec::new();

        for case in cases {
            // Check if this case applies
            let applies = match &case.when {
                Some(condition) => condition.matches(&block.properties),
                None => true, // No condition = always applies
            };

            if applies {
                // Add all variants from this case
                for variant in case.apply.variants() {
                    result.push(variant.clone());
                }
            }
        }

        if result.is_empty() {
            Err(MesherError::BlockstateResolution(format!(
                "No multipart cases matched for {} with properties {:?}",
                block.name, block.properties
            )))
        } else {
            Ok(result)
        }
    }

    /// Check if the variant key's properties are consistent with the user's block properties.
    /// For each property in the variant key, the user's block must have a matching value.
    /// User properties not mentioned in the variant key are ignored (e.g., `waterlogged`
    /// is absent from slab variant keys but present on blocks).
    fn user_properties_match_variant(
        &self,
        variant_key: &str,
        user_properties: &std::collections::HashMap<String, String>,
    ) -> bool {
        if variant_key.is_empty() {
            return true;
        }

        // Parse variant key into pairs and check each against user properties
        for pair in variant_key.split(',') {
            if let Some((variant_prop, variant_value)) = pair.split_once('=') {
                match user_properties.get(variant_prop) {
                    Some(user_value) => {
                        if user_value != variant_value {
                            return false; // User has this property but with a different value
                        }
                    }
                    None => {
                        // User didn't specify this property — can't confirm match,
                        // but don't reject (default scoring handles unspecified props)
                    }
                }
            }
        }

        true
    }

    /// Calculate default score only for properties NOT specified by the user.
    fn calculate_default_score_for_unspecified(
        &self,
        variant_key: &str,
        user_properties: &std::collections::HashMap<String, String>,
    ) -> i32 {
        if variant_key.is_empty() {
            return i32::MAX;
        }

        let mut score = 0;

        for pair in variant_key.split(',') {
            if let Some((prop, value)) = pair.split_once('=') {
                // Only score properties not specified by the user
                if !user_properties.contains_key(prop) {
                    score += self.value_default_score(prop, value);
                }
            }
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_pack::blockstate::BlockstateDefinition;

    fn create_test_pack() -> ResourcePack {
        let mut pack = ResourcePack::new();

        // Add a simple stone blockstate
        let stone_json = r#"{
            "variants": {
                "": { "model": "block/stone" }
            }
        }"#;
        let stone_def: BlockstateDefinition = serde_json::from_str(stone_json).unwrap();
        pack.add_blockstate("minecraft", "stone", stone_def);

        // Add a directional block
        let furnace_json = r#"{
            "variants": {
                "facing=north": { "model": "block/furnace", "y": 0 },
                "facing=east": { "model": "block/furnace", "y": 90 },
                "facing=south": { "model": "block/furnace", "y": 180 },
                "facing=west": { "model": "block/furnace", "y": 270 }
            }
        }"#;
        let furnace_def: BlockstateDefinition = serde_json::from_str(furnace_json).unwrap();
        pack.add_blockstate("minecraft", "furnace", furnace_def);

        pack
    }

    #[test]
    fn test_resolve_simple_block() {
        let pack = create_test_pack();
        let resolver = StateResolver::new(&pack);

        let block = InputBlock::new("minecraft:stone");
        let variants = resolver.resolve(&block).unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/stone");
    }

    #[test]
    fn test_resolve_directional_block() {
        let pack = create_test_pack();
        let resolver = StateResolver::new(&pack);

        let block = InputBlock::new("minecraft:furnace")
            .with_property("facing", "east");
        let variants = resolver.resolve(&block).unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/furnace");
        assert_eq!(variants[0].y, 90);
    }

    #[test]
    fn test_missing_blockstate() {
        let pack = create_test_pack();
        let resolver = StateResolver::new(&pack);

        let block = InputBlock::new("minecraft:nonexistent");
        let result = resolver.resolve(&block);

        assert!(result.is_err());
    }

    #[test]
    fn test_partial_properties() {
        let mut pack = ResourcePack::new();

        // Add a piston blockstate with multiple properties
        let piston_json = r#"{
            "variants": {
                "extended=false,facing=down": { "model": "block/piston", "x": 180 },
                "extended=false,facing=east": { "model": "block/piston", "y": 90 },
                "extended=false,facing=north": { "model": "block/piston" },
                "extended=false,facing=south": { "model": "block/piston", "y": 180 },
                "extended=false,facing=up": { "model": "block/piston", "x": 270 },
                "extended=false,facing=west": { "model": "block/piston", "y": 270 },
                "extended=true,facing=down": { "model": "block/piston_extended", "x": 180 },
                "extended=true,facing=east": { "model": "block/piston_extended", "y": 90 },
                "extended=true,facing=north": { "model": "block/piston_extended" },
                "extended=true,facing=south": { "model": "block/piston_extended", "y": 180 },
                "extended=true,facing=up": { "model": "block/piston_extended", "x": 270 },
                "extended=true,facing=west": { "model": "block/piston_extended", "y": 270 }
            }
        }"#;
        let piston_def: BlockstateDefinition = serde_json::from_str(piston_json).unwrap();
        pack.add_blockstate("minecraft", "piston", piston_def);

        let resolver = StateResolver::new(&pack);

        // Test with only facing specified - should pick extended=false (more default)
        let block = InputBlock::new("minecraft:piston")
            .with_property("facing", "north");
        let variants = resolver.resolve(&block).unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/piston"); // extended=false version

        // Test with only extended specified - should pick facing=north (more default)
        let block = InputBlock::new("minecraft:piston")
            .with_property("extended", "true");
        let variants = resolver.resolve(&block).unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/piston_extended");

        // Test with no properties - should get extended=false, facing=north
        let block = InputBlock::new("minecraft:piston");
        let variants = resolver.resolve(&block).unwrap();

        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/piston");
    }

    #[test]
    fn test_slab_type_with_waterlogged() {
        let mut pack = ResourcePack::new();

        // Slab blockstate: variant keys only have `type`, not `waterlogged`
        let slab_json = r#"{
            "variants": {
                "type=bottom": { "model": "block/stone_slab" },
                "type=top": { "model": "block/stone_slab_top" },
                "type=double": { "model": "block/stone" }
            }
        }"#;
        let slab_def: BlockstateDefinition = serde_json::from_str(slab_json).unwrap();
        pack.add_blockstate("minecraft", "stone_slab", slab_def);

        let resolver = StateResolver::new(&pack);

        // Block has both type and waterlogged — should match type=top
        let block = InputBlock::new("minecraft:stone_slab")
            .with_property("type", "top")
            .with_property("waterlogged", "false");
        let variants = resolver.resolve(&block).unwrap();
        assert_eq!(variants.len(), 1);
        assert_eq!(variants[0].model, "block/stone_slab_top");

        // type=bottom with waterlogged
        let block = InputBlock::new("minecraft:stone_slab")
            .with_property("type", "bottom")
            .with_property("waterlogged", "false");
        let variants = resolver.resolve(&block).unwrap();
        assert_eq!(variants[0].model, "block/stone_slab");

        // type=double with waterlogged
        let block = InputBlock::new("minecraft:stone_slab")
            .with_property("type", "double")
            .with_property("waterlogged", "false");
        let variants = resolver.resolve(&block).unwrap();
        assert_eq!(variants[0].model, "block/stone");
    }
}
