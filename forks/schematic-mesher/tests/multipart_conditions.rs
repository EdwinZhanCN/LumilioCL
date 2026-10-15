//! Regression test: multipart `when` conditions nest and negate as in Minecraft.
//!
//! Backstory: `OR`/`AND` lists accepted only flat property maps, so a pack
//! that nested them, or wrote `"!north"`, failed to load the whole blockstate
//! and the block went undrawn. This is a LumilioCL local change (see
//! `forks/README.md`).

mod common;

use std::collections::HashMap;

use common::Fixture;
use schematic_mesher::resource_pack::MultipartCondition;
use schematic_mesher::BlockstateDefinition;

/// The condition a baked multipart case reads `json` as.
fn when(json: &str) -> MultipartCondition {
    let mut pack = Fixture::default();
    pack.blockstate(
        "wire",
        &format!(
            r#"{{"multipart":[{{"when":{json},"apply":{{"model":"minecraft:block/wire"}}}}]}}"#
        ),
    );
    pack.model("block/wire", "{}");
    let pack = pack.bake();
    let Some(BlockstateDefinition::Multipart(cases)) = pack.get_blockstate("minecraft:wire") else {
        panic!("a multipart blockstate");
    };
    cases[0].when.clone().unwrap()
}

fn props(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn conditions_nest() {
    // 26.x writes `true` where older versions write `"true"`.
    for powered in [r#""true""#, "true"] {
        let when = when(&format!(
            r#"{{"OR":[{{"AND":[{{"facing":"north"}},{{"powered":{powered}}}]}},{{"facing":"up"}}]}}"#
        ));
        assert!(when.matches(&props(&[("facing", "north"), ("powered", "true")])));
        assert!(!when.matches(&props(&[("facing", "north"), ("powered", "false")])));
        assert!(when.matches(&props(&[("facing", "up"), ("powered", "false")])));
    }
}

#[test]
fn a_leading_bang_negates_the_listed_values() {
    let when = when(r#"{"facing":"!north|south"}"#);
    assert!(when.matches(&props(&[("facing", "east")])));
    assert!(!when.matches(&props(&[("facing", "north")])));
    assert!(!when.matches(&props(&[("facing", "south")])));
}
