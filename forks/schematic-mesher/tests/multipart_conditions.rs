//! Regression test: multipart `when` conditions nest and negate as in Minecraft.
//!
//! Backstory: `OR`/`AND` lists accepted only flat property maps, so a pack
//! that nested them, or wrote `"!north"`, failed to load the whole blockstate
//! and the block went undrawn. This is a LumilioCL local change (see
//! `forks/README.md`).

use std::collections::HashMap;

use schematic_mesher::resource_pack::MultipartCondition;

fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn conditions_nest() {
    let when: MultipartCondition = serde_json::from_str(
        r#"{"OR":[{"AND":[{"facing":"north"},{"powered":"true"}]},{"facing":"up"}]}"#,
    )
    .unwrap();
    assert!(when.matches(&props(&[("facing", "north"), ("powered", "true")])));
    assert!(!when.matches(&props(&[("facing", "north"), ("powered", "false")])));
    assert!(when.matches(&props(&[("facing", "up"), ("powered", "false")])));
}

#[test]
fn a_leading_bang_negates_the_listed_values() {
    let when: MultipartCondition = serde_json::from_str(r#"{"facing":"!north|south"}"#).unwrap();
    assert!(when.matches(&props(&[("facing", "east")])));
    assert!(!when.matches(&props(&[("facing", "north")])));
    assert!(!when.matches(&props(&[("facing", "south")])));
}
