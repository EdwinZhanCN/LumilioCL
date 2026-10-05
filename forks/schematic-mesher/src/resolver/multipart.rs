//! Multipart blockstate condition evaluation.
//!
//! This module provides utilities for evaluating multipart blockstate conditions.
//! The main logic is in the `MultipartCondition::matches` method in blockstate.rs.

// Re-export the condition type for convenience
pub use crate::resource_pack::blockstate::MultipartCondition;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_multipart_condition_and() {
        let json = r#"{ "AND": [{ "north": "true" }, { "south": "true" }] }"#;
        let cond: MultipartCondition = serde_json::from_str(json).unwrap();

        let both: HashMap<String, String> = [
            ("north".to_string(), "true".to_string()),
            ("south".to_string(), "true".to_string()),
        ]
        .into_iter()
        .collect();

        let only_north: HashMap<String, String> =
            [("north".to_string(), "true".to_string())].into_iter().collect();

        assert!(cond.matches(&both));
        assert!(!cond.matches(&only_north));
    }
}
