//! Overlays baked over an already baked base (plan schematic-studio, T17).

use image::codecs::png::CompressionType;

use super::{archive, json, png};
use crate::{AssetSource, BakedPack, Direction, bake, bake_over};

/// A client with the vanilla template chain `stone → cube_all → cube`.
fn vanilla() -> Vec<u8> {
    archive(&[
        (
            "version.json",
            json(r#"{"id":"1.20.1","world_version":3465}"#),
        ),
        (
            "assets/minecraft/blockstates/stone.json",
            json(r#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#),
        ),
        (
            "assets/minecraft/models/block/cube.json",
            json(
                r##"{"elements":[{"from":[0,0,0],"to":[16,16,16],
                    "faces":{"down":{"texture":"#down","cullface":"down"},
                             "north":{"texture":"#north","cullface":"north"}}}]}"##,
            ),
        ),
        (
            "assets/minecraft/models/block/cube_all.json",
            json(
                r##"{"parent":"block/cube",
                    "textures":{"particle":"#all","down":"#all","north":"#all"}}"##,
            ),
        ),
        (
            "assets/minecraft/models/block/stone.json",
            json(r#"{"parent":"minecraft:block/cube_all","textures":{"all":"block/stone"}}"#),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            png(3, CompressionType::Fast),
        ),
    ])
}

/// A mod whose ore inherits a vanilla template and whose slab inherits a
/// vanilla block that is no template.
fn mod_jar() -> Vec<u8> {
    archive(&[
        ("fabric.mod.json", json(r#"{"id":"gems"}"#)),
        (
            "assets/gems/blockstates/ruby_ore.json",
            json(r#"{"variants":{"":{"model":"gems:block/ruby_ore"}}}"#),
        ),
        (
            "assets/gems/blockstates/stone_copy.json",
            json(r#"{"variants":{"":{"model":"gems:block/stone_copy"}}}"#),
        ),
        (
            "assets/gems/models/block/ruby_ore.json",
            json(
                r#"{"parent":"minecraft:block/cube_all","textures":{"all":"gems:block/ruby_ore"}}"#,
            ),
        ),
        (
            "assets/gems/models/block/stone_copy.json",
            json(r#"{"parent":"minecraft:block/stone"}"#),
        ),
        (
            "assets/gems/textures/block/ruby_ore.png",
            png(9, CompressionType::Fast),
        ),
    ])
}

/// The bundle's way: the base read back from its container, as a download
/// gives it, the mod baked over it.
fn base_from_container() -> BakedPack {
    let base = bake(&[AssetSource::ArchiveBytes(vanilla())]).unwrap();
    BakedPack::from_bytes(&base.to_bytes().unwrap()).unwrap()
}

/// D13's follow-up: a mod model inheriting a vanilla parent has its
/// geometry when baked over a baked base, with the mod's texture on it.
#[test]
fn a_mod_model_with_a_vanilla_parent_keeps_its_geometry_over_a_baked_base() {
    let base = base_from_container();
    assert!(base.assets.parents.contains_key("minecraft:block/cube_all"));
    assert!(base.assets.parents.contains_key("minecraft:block/cube"));
    assert!(
        !base.assets.parents.contains_key("minecraft:block/stone"),
        "only models something inherits from are kept as written"
    );

    let alone = bake(&[AssetSource::ArchiveBytes(mod_jar())]).unwrap();
    assert!(
        alone.assets.models["gems:block/ruby_ore"]
            .elements
            .is_empty(),
        "without the base the parent is missing, as before T17"
    );

    let layer = bake_over(&base, &[AssetSource::ArchiveBytes(mod_jar())]).unwrap();
    let ore = &layer.assets.models["gems:block/ruby_ore"];
    assert_eq!(ore.elements.len(), 1);
    let faces = &ore.elements[0].faces;
    assert_eq!(
        faces[&Direction::North].texture.as_deref(),
        Some("gems:block/ruby_ore")
    );
    assert_eq!(faces[&Direction::Down].cullface, Some(Direction::Down));
    assert!(layer.assets.textures.contains_key("gems:block/ruby_ore"));

    let copy = &layer.assets.models["gems:block/stone_copy"];
    assert_eq!(
        copy.elements[0].faces[&Direction::North].texture.as_deref(),
        Some("minecraft:block/stone"),
        "a flattened base model serves as a parent too"
    );

    assert!(
        !layer.assets.blockstates.contains_key("minecraft:stone")
            && !layer.assets.models.contains_key("minecraft:block/stone"),
        "the layer holds only what the overlays define"
    );
    assert!(layer.assets.parents.is_empty());
}

/// Baking over the base gives the mod the same models as baking the client
/// jar and the mod together.
#[test]
fn baking_over_the_base_matches_baking_with_the_jar() {
    let together = bake(&[
        AssetSource::ArchiveBytes(vanilla()),
        AssetSource::ArchiveBytes(mod_jar()),
    ])
    .unwrap();
    let layer = bake_over(
        &base_from_container(),
        &[AssetSource::ArchiveBytes(mod_jar())],
    )
    .unwrap();
    assert_eq!(
        layer.assets.models["gems:block/ruby_ore"],
        together.assets.models["gems:block/ruby_ore"]
    );
}

/// A jar that is no zip, or a library mod without assets, is no resource
/// source: the bake goes on and its stamp says so.
#[test]
fn a_source_without_assets_is_left_out_without_an_error() {
    let library = archive(&[("com/example/Lib.class", b"code".to_vec())]);
    let layer = bake_over(
        &base_from_container(),
        &[
            AssetSource::ArchiveBytes(b"not a zip".to_vec()),
            AssetSource::ArchiveBytes(library),
            AssetSource::ArchiveBytes(mod_jar()),
        ],
    )
    .unwrap();
    let stamps = &layer.meta.sources;
    assert_eq!(stamps.len(), 3);
    assert!(stamps[0].unreadable && stamps[0].assets == 0);
    assert!(!stamps[1].unreadable && stamps[1].assets == 0);
    assert_eq!((stamps[2].blocks, stamps[2].assets), (2, 5));
    assert_eq!(layer.meta.skipped.len(), 1, "{:?}", layer.meta.skipped);
    assert!(layer.assets.blockstates.contains_key("gems:ruby_ore"));

    let error = bake(&[AssetSource::ArchiveBytes(b"not a zip".to_vec())]);
    assert!(error.is_err(), "an unreadable base still fails the bake");
}

/// A pack that replaces a vanilla model alone reaches the base's block.
#[test]
fn a_replaced_base_model_is_baked_into_the_layer() {
    let pack = archive(&[(
        "assets/minecraft/models/block/stone.json",
        json(
            r#"{"parent":"minecraft:block/cube_all","textures":{"all":"minecraft:block/granite"}}"#,
        ),
    )]);
    let layer = bake_over(&base_from_container(), &[AssetSource::ArchiveBytes(pack)]).unwrap();
    let stone = &layer.assets.models["minecraft:block/stone"];
    assert_eq!(stone.textures["all"], "minecraft:block/granite");
    assert_eq!(stone.elements.len(), 1);
}
