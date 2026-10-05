//! Blockstate definition parsing.
//!
//! Blockstates define how block properties map to model variants.
//! There are two formats: "variants" and "multipart".

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// A blockstate definition from blockstates/*.json.
#[derive(Debug, Clone)]
pub enum BlockstateDefinition {
    /// Simple variants: property combinations map to models.
    Variants(HashMap<String, Vec<ModelVariant>>),
    /// Multipart: conditional model application.
    Multipart(Vec<MultipartCase>),
}

impl<'de> Deserialize<'de> for BlockstateDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawBlockstate {
            variants: Option<HashMap<String, VariantValue>>,
            multipart: Option<Vec<MultipartCase>>,
        }

        let raw = RawBlockstate::deserialize(deserializer)?;

        if let Some(variants) = raw.variants {
            let parsed: HashMap<String, Vec<ModelVariant>> = variants
                .into_iter()
                .map(|(k, v)| (k, v.into_vec()))
                .collect();
            Ok(BlockstateDefinition::Variants(parsed))
        } else if let Some(multipart) = raw.multipart {
            Ok(BlockstateDefinition::Multipart(multipart))
        } else {
            // Empty blockstate (shouldn't happen but handle gracefully)
            Ok(BlockstateDefinition::Variants(HashMap::new()))
        }
    }
}

/// A variant value can be a single model or an array of weighted models.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum VariantValue {
    Single(ModelVariant),
    Multiple(Vec<ModelVariant>),
}

impl VariantValue {
    fn into_vec(self) -> Vec<ModelVariant> {
        match self {
            VariantValue::Single(v) => vec![v],
            VariantValue::Multiple(v) => v,
        }
    }
}

/// A model variant reference with optional rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVariant {
    /// Model resource location (e.g., "block/stone" or "minecraft:block/stone").
    pub model: String,
    /// X rotation in degrees (0, 90, 180, 270).
    #[serde(default)]
    pub x: i32,
    /// Y rotation in degrees (0, 90, 180, 270).
    #[serde(default)]
    pub y: i32,
    /// If true, UV coordinates don't rotate with the block.
    #[serde(default)]
    pub uvlock: bool,
    /// Weight for random selection (default 1).
    #[serde(default = "default_weight")]
    pub weight: u32,
}

fn default_weight() -> u32 {
    1
}

impl ModelVariant {
    /// Get the full resource location for the model.
    pub fn model_location(&self) -> String {
        if self.model.contains(':') {
            self.model.clone()
        } else {
            format!("minecraft:{}", self.model)
        }
    }
}

/// A multipart case with optional condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipartCase {
    /// Condition for when this case applies.
    #[serde(default)]
    pub when: Option<MultipartCondition>,
    /// Model(s) to apply when condition is met.
    pub apply: ApplyValue,
}

/// The apply value can be a single model or array.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ApplyValue {
    Single(ModelVariant),
    Multiple(Vec<ModelVariant>),
}

impl ApplyValue {
    pub fn variants(&self) -> Vec<&ModelVariant> {
        match self {
            ApplyValue::Single(v) => vec![v],
            ApplyValue::Multiple(v) => v.iter().collect(),
        }
    }
}

/// Multipart condition for when a case applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MultipartCondition {
    /// OR condition: any of the sub-conditions must match.
    Or { OR: Vec<HashMap<String, String>> },
    /// AND condition: all of the sub-conditions must match.
    And { AND: Vec<HashMap<String, String>> },
    /// Simple condition: all properties must match.
    Simple(HashMap<String, String>),
}

impl MultipartCondition {
    /// Check if the condition matches the given block properties.
    pub fn matches(&self, properties: &HashMap<String, String>) -> bool {
        match self {
            MultipartCondition::Or { OR } => {
                OR.iter().any(|cond| Self::matches_simple(cond, properties))
            }
            MultipartCondition::And { AND } => {
                AND.iter().all(|cond| Self::matches_simple(cond, properties))
            }
            MultipartCondition::Simple(cond) => Self::matches_simple(cond, properties),
        }
    }

    /// Check if a simple condition (property map) matches.
    fn matches_simple(
        condition: &HashMap<String, String>,
        properties: &HashMap<String, String>,
    ) -> bool {
        condition.iter().all(|(key, expected_value)| {
            // Handle pipe-separated values (e.g., "north|south")
            if expected_value.contains('|') {
                let allowed: Vec<&str> = expected_value.split('|').collect();
                properties
                    .get(key)
                    .map(|v| allowed.contains(&v.as_str()))
                    .unwrap_or_else(|| {
                        // If property is missing, check if any allowed value is a default
                        allowed.iter().any(|v| Self::is_default_value(v))
                    })
            } else {
                properties
                    .get(key)
                    .map(|v| v == expected_value)
                    .unwrap_or_else(|| {
                        // If property is missing, check if expected value is a default
                        Self::is_default_value(expected_value)
                    })
            }
        })
    }

    /// Check if a value is a common default for missing properties.
    /// Missing properties in Minecraft typically default to false/none/0.
    fn is_default_value(value: &str) -> bool {
        matches!(
            value,
            "false" | "none" | "0" | "normal" | "bottom" | "floor"
        )
    }
}

/// Build a property string from a properties map for variant lookup.
/// Properties are sorted alphabetically and joined with commas.
/// e.g., {"facing": "north", "half": "bottom"} -> "facing=north,half=bottom"
pub fn build_property_string(properties: &HashMap<String, String>) -> String {
    if properties.is_empty() {
        return String::new();
    }

    let mut pairs: Vec<_> = properties.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);

    pairs
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_variants() {
        let json = r#"{
            "variants": {
                "": { "model": "block/stone" }
            }
        }"#;

        let def: BlockstateDefinition = serde_json::from_str(json).unwrap();
        match def {
            BlockstateDefinition::Variants(variants) => {
                assert!(variants.contains_key(""));
                assert_eq!(variants[""].len(), 1);
                assert_eq!(variants[""][0].model, "block/stone");
            }
            _ => panic!("Expected Variants"),
        }
    }

    #[test]
    fn test_parse_variants_with_rotation() {
        let json = r#"{
            "variants": {
                "facing=north": { "model": "block/furnace", "y": 0 },
                "facing=east": { "model": "block/furnace", "y": 90 },
                "facing=south": { "model": "block/furnace", "y": 180 },
                "facing=west": { "model": "block/furnace", "y": 270 }
            }
        }"#;

        let def: BlockstateDefinition = serde_json::from_str(json).unwrap();
        match def {
            BlockstateDefinition::Variants(variants) => {
                assert_eq!(variants.len(), 4);
                assert_eq!(variants["facing=east"][0].y, 90);
            }
            _ => panic!("Expected Variants"),
        }
    }

    #[test]
    fn test_parse_weighted_variants() {
        let json = r#"{
            "variants": {
                "": [
                    { "model": "block/stone", "weight": 10 },
                    { "model": "block/stone_mirrored", "weight": 5 }
                ]
            }
        }"#;

        let def: BlockstateDefinition = serde_json::from_str(json).unwrap();
        match def {
            BlockstateDefinition::Variants(variants) => {
                assert_eq!(variants[""].len(), 2);
                assert_eq!(variants[""][0].weight, 10);
                assert_eq!(variants[""][1].weight, 5);
            }
            _ => panic!("Expected Variants"),
        }
    }

    #[test]
    fn test_parse_multipart() {
        let json = r#"{
            "multipart": [
                { "apply": { "model": "block/fence_post" } },
                { "when": { "north": "true" }, "apply": { "model": "block/fence_side" } }
            ]
        }"#;

        let def: BlockstateDefinition = serde_json::from_str(json).unwrap();
        match def {
            BlockstateDefinition::Multipart(cases) => {
                assert_eq!(cases.len(), 2);
                assert!(cases[0].when.is_none());
                assert!(cases[1].when.is_some());
            }
            _ => panic!("Expected Multipart"),
        }
    }

    #[test]
    fn test_multipart_condition_simple() {
        let cond = MultipartCondition::Simple(
            [("facing".to_string(), "north".to_string())]
                .into_iter()
                .collect(),
        );

        let props: HashMap<String, String> =
            [("facing".to_string(), "north".to_string())].into_iter().collect();
        assert!(cond.matches(&props));

        let wrong_props: HashMap<String, String> =
            [("facing".to_string(), "south".to_string())].into_iter().collect();
        assert!(!cond.matches(&wrong_props));
    }

    #[test]
    fn test_multipart_condition_or() {
        let json = r#"{ "OR": [{ "facing": "north" }, { "facing": "south" }] }"#;
        let cond: MultipartCondition = serde_json::from_str(json).unwrap();

        let north: HashMap<String, String> =
            [("facing".to_string(), "north".to_string())].into_iter().collect();
        let south: HashMap<String, String> =
            [("facing".to_string(), "south".to_string())].into_iter().collect();
        let east: HashMap<String, String> =
            [("facing".to_string(), "east".to_string())].into_iter().collect();

        assert!(cond.matches(&north));
        assert!(cond.matches(&south));
        assert!(!cond.matches(&east));
    }

    #[test]
    fn test_multipart_condition_pipe_values() {
        let cond = MultipartCondition::Simple(
            [("facing".to_string(), "north|south".to_string())]
                .into_iter()
                .collect(),
        );

        let north: HashMap<String, String> =
            [("facing".to_string(), "north".to_string())].into_iter().collect();
        let south: HashMap<String, String> =
            [("facing".to_string(), "south".to_string())].into_iter().collect();
        let east: HashMap<String, String> =
            [("facing".to_string(), "east".to_string())].into_iter().collect();

        assert!(cond.matches(&north));
        assert!(cond.matches(&south));
        assert!(!cond.matches(&east));
    }

    #[test]
    fn test_build_property_string() {
        let props: HashMap<String, String> = [
            ("facing".to_string(), "north".to_string()),
            ("half".to_string(), "bottom".to_string()),
        ]
        .into_iter()
        .collect();

        assert_eq!(build_property_string(&props), "facing=north,half=bottom");
    }

    #[test]
    fn test_build_property_string_empty() {
        let props: HashMap<String, String> = HashMap::new();
        assert_eq!(build_property_string(&props), "");
    }
}
