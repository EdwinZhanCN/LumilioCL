//! Model inheritance resolution.

use crate::error::{MesherError, Result};
use crate::resource_pack::{BlockModel, ResourcePack};
use std::collections::HashMap;

/// Maximum depth for model inheritance to prevent infinite loops.
const MAX_INHERITANCE_DEPTH: usize = 10;

/// Resolves model inheritance chains.
pub struct ModelResolver<'a> {
    pack: &'a ResourcePack,
    cache: std::cell::RefCell<HashMap<String, BlockModel>>,
}

impl<'a> ModelResolver<'a> {
    pub fn new(pack: &'a ResourcePack) -> Self {
        Self {
            pack,
            cache: std::cell::RefCell::new(HashMap::new()),
        }
    }

    /// Resolve a model with all inherited properties.
    pub fn resolve(&self, model_location: &str) -> Result<BlockModel> {
        // Check cache first
        if let Some(cached) = self.cache.borrow().get(model_location) {
            return Ok(cached.clone());
        }

        let resolved = self.resolve_internal(model_location, 0)?;

        // Cache the result
        self.cache
            .borrow_mut()
            .insert(model_location.to_string(), resolved.clone());

        Ok(resolved)
    }

    fn resolve_internal(&self, model_location: &str, depth: usize) -> Result<BlockModel> {
        if depth >= MAX_INHERITANCE_DEPTH {
            return Err(MesherError::ModelInheritanceTooDeep(
                model_location.to_string(),
            ));
        }

        // Normalize the location
        let normalized = self.normalize_location(model_location);

        // Get the base model
        let base_model = self.pack.get_model(&normalized).ok_or_else(|| {
            MesherError::ModelResolution(format!("Model not found: {}", normalized))
        })?;

        // If there's no parent, return the model as-is
        let parent_location = match &base_model.parent {
            Some(parent) => parent.clone(),
            None => return Ok(base_model.clone()),
        };

        // Skip builtin parents (like builtin/generated, builtin/entity)
        if parent_location.starts_with("builtin/") {
            return Ok(base_model.clone());
        }

        // Resolve the parent recursively
        let parent_model = self.resolve_internal(&parent_location, depth + 1)?;

        // Merge parent into child
        Ok(self.merge_models(&parent_model, base_model))
    }

    /// Merge a parent model into a child model.
    /// Child properties override parent properties.
    fn merge_models(&self, parent: &BlockModel, child: &BlockModel) -> BlockModel {
        let mut merged = parent.clone();

        // Merge textures (child overrides parent)
        for (key, value) in &child.textures {
            merged.textures.insert(key.clone(), value.clone());
        }

        // Use child elements if present, otherwise keep parent elements
        if !child.elements.is_empty() {
            merged.elements = child.elements.clone();
        }

        // Use child ambient_occlusion setting
        merged.ambient_occlusion = child.ambient_occlusion;

        // Merge display contexts: child contexts override parent, but parent
        // contexts not present in child are preserved. This is important because
        // e.g. item/handheld defines thirdperson views while item/generated
        // defines the fixed view — both need to survive the merge.
        match (&merged.display, &child.display) {
            (Some(parent_display), Some(child_display)) => {
                if let (Some(parent_obj), Some(child_obj)) =
                    (parent_display.as_object(), child_display.as_object())
                {
                    let mut merged_display = parent_obj.clone();
                    for (key, value) in child_obj {
                        merged_display.insert(key.clone(), value.clone());
                    }
                    merged.display = Some(serde_json::Value::Object(merged_display));
                } else {
                    merged.display = child.display.clone();
                }
            }
            (None, Some(_)) => {
                merged.display = child.display.clone();
            }
            _ => {} // parent kept or both None
        }

        // Clear parent reference (model is now resolved)
        merged.parent = None;

        merged
    }

    /// Normalize a model location to full resource path.
    fn normalize_location(&self, location: &str) -> String {
        if location.contains(':') {
            location.to_string()
        } else {
            format!("minecraft:{}", location)
        }
    }

    /// Fully resolve texture references in a model.
    /// Resolves chains like #side -> #all -> block/stone.
    pub fn resolve_textures(&self, model: &BlockModel) -> HashMap<String, String> {
        let mut resolved = HashMap::new();

        for (key, value) in &model.textures {
            let final_value = self.resolve_texture_chain(value, &model.textures, 0);
            resolved.insert(key.clone(), final_value);
        }

        resolved
    }

    fn resolve_texture_chain(
        &self,
        reference: &str,
        textures: &HashMap<String, String>,
        depth: usize,
    ) -> String {
        if depth >= 10 || !reference.starts_with('#') {
            return reference.to_string();
        }

        let key = &reference[1..];
        if let Some(value) = textures.get(key) {
            self.resolve_texture_chain(value, textures, depth + 1)
        } else {
            reference.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_pack::model::{BlockModel, ModelElement, ModelFace};
    use crate::types::Direction;

    fn create_test_pack() -> ResourcePack {
        let mut pack = ResourcePack::new();

        // Add cube_all (parent)
        let cube_all = BlockModel {
            parent: Some("block/cube".to_string()),
            textures: [("particle".to_string(), "#all".to_string())]
                .into_iter()
                .collect(),
            elements: vec![ModelElement {
                from: [0.0, 0.0, 0.0],
                to: [16.0, 16.0, 16.0],
                rotation: None,
                shade: true,
                faces: Direction::ALL
                    .iter()
                    .map(|d| {
                        (
                            *d,
                            ModelFace {
                                texture: "#all".to_string(),
                                uv: None,
                                cullface: Some(*d),
                                rotation: 0,
                                tintindex: -1,
                            },
                        )
                    })
                    .collect(),
            }],
            ..Default::default()
        };
        pack.add_model("minecraft", "block/cube_all", cube_all);

        // Add cube (grandparent) - just to test inheritance works
        let cube = BlockModel {
            parent: None,
            ambient_occlusion: true,
            textures: HashMap::new(),
            elements: vec![],
            ..Default::default()
        };
        pack.add_model("minecraft", "block/cube", cube);

        // Add stone (child of cube_all)
        let stone = BlockModel {
            parent: Some("block/cube_all".to_string()),
            textures: [("all".to_string(), "block/stone".to_string())]
                .into_iter()
                .collect(),
            elements: vec![],
            ..Default::default()
        };
        pack.add_model("minecraft", "block/stone", stone);

        pack
    }

    #[test]
    fn test_resolve_simple_model() {
        let pack = create_test_pack();
        let resolver = ModelResolver::new(&pack);

        let model = resolver.resolve("minecraft:block/cube").unwrap();
        assert!(model.parent.is_none());
    }

    #[test]
    fn test_resolve_with_inheritance() {
        let pack = create_test_pack();
        let resolver = ModelResolver::new(&pack);

        let model = resolver.resolve("minecraft:block/stone").unwrap();

        // Should have inherited elements from cube_all
        assert!(!model.elements.is_empty());

        // Should have merged textures
        assert!(model.textures.contains_key("all"));
        assert_eq!(model.textures.get("all"), Some(&"block/stone".to_string()));
    }

    #[test]
    fn test_resolve_texture_chain() {
        let pack = create_test_pack();
        let resolver = ModelResolver::new(&pack);

        let model = resolver.resolve("minecraft:block/stone").unwrap();
        let resolved_textures = resolver.resolve_textures(&model);

        // particle -> #all -> block/stone
        assert_eq!(
            resolved_textures.get("particle"),
            Some(&"block/stone".to_string())
        );
    }

    #[test]
    fn test_missing_model() {
        let pack = create_test_pack();
        let resolver = ModelResolver::new(&pack);

        let result = resolver.resolve("minecraft:block/nonexistent");
        assert!(result.is_err());
    }
}
