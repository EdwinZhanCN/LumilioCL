//! Entity coverage (LumilioCL local change, plan schematic-studio T35, D32):
//! every entity type in every bundled game version's registry is drawn, by
//! a table row or a row made from its renderer's look, or is one the game
//! draws nothing for. A gap fails here and in `cargo xtask bake-blocks`,
//! named, instead of reaching a schematic as an entity nothing draws.
//!
//! Reads the baked bundle in `target/block-assets/` (or `LUMILIO_BLOCK_BUNDLE`
//! and `LUMILIO_ENTITIES`); without it the check says so and passes.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use lumilio_block_assets::{Bundle, EntityBundle, EntityLook};
use schematic_mesher::mesher::entity::bindings::coverage;
use schematic_mesher::ResourcePack;

fn target_file(variable: &str, name: &str) -> PathBuf {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/block-assets")
                .join(name)
        })
}

#[test]
fn every_entity_type_of_every_version_is_drawn() {
    let blocks = target_file("LUMILIO_BLOCK_BUNDLE", "blocks.dat.z");
    let entities = target_file("LUMILIO_ENTITIES", "entities.dat.z");
    let (Ok(blocks), Ok(entities)) = (std::fs::read(&blocks), std::fs::read(&entities)) else {
        eprintln!("skipped: no baked bundle (cargo xtask bake-blocks)");
        return;
    };
    let blocks = Bundle::read(&blocks).unwrap();
    let entities = EntityBundle::read(&entities).unwrap();
    // Entity id → (reason, versions).
    let mut gaps: BTreeMap<(String, &str), Vec<String>> = BTreeMap::new();
    for version in &entities.versions {
        let Ok(pack) = blocks.pack(&version.id) else {
            continue;
        };
        let pack =
            ResourcePack::from_baked(&pack).with_entity_layers(entities.layers_of(&version.id));
        for (id, why) in coverage(&pack, &pack.entity_looks) {
            gaps.entry((id, why)).or_default().push(version.id.clone());
        }
    }
    let report: Vec<String> = gaps
        .iter()
        .map(|((id, why), versions)| {
            format!(
                "{id}: {why} ({} versions: {} … {})",
                versions.len(),
                versions.first().unwrap(),
                versions.last().unwrap()
            )
        })
        .collect();
    assert!(
        report.is_empty(),
        "entities nothing draws:\n{}",
        report.join("\n")
    );
}

/// The check itself: a type without a row or a usable look is named; one
/// the game draws nothing for, and one made from a living look, are not.
#[test]
fn the_check_names_what_nothing_draws() {
    let look = |renderer: &str, texture: Option<&str>| EntityLook {
        renderer: vec![renderer.to_owned(), "EntityRenderer".to_owned()],
        layers: vec!["minecraft:thing#main".to_owned()],
        texture: texture.map(str::to_owned),
        ..EntityLook::default()
    };
    let looks: HashMap<String, EntityLook> = [
        ("minecraft:marker", look("NoopRenderer", None)),
        (
            "minecraft:lightning_bolt",
            look("LightningBoltRenderer", None),
        ),
        ("minecraft:snowball", look("ThrownItemRenderer", None)),
        ("minecraft:mystery", look("MysteryRenderer", None)),
        (
            "minecraft:critter",
            EntityLook {
                renderer: vec!["CritterRenderer".into(), "LivingEntityRenderer".into()],
                ..look(
                    "CritterRenderer",
                    Some("minecraft:textures/entity/critter.png"),
                )
            },
        ),
    ]
    .into_iter()
    .map(|(id, look)| (id.to_owned(), look))
    .collect();
    let pack = ResourcePack::new();
    let missing: Vec<String> = coverage(&pack, &looks)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    // The critter's row is made, but this version has no layer for it.
    assert_eq!(missing, ["minecraft:critter", "minecraft:mystery"]);
}
