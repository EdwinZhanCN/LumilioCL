//! Regression test: a pack written only in 26.3's spelling meshes with no
//! rewriting at load time.
//!
//! Backstory: the mesher read 0.2-era JSON, so Nucleation rewrote every 26.x
//! client jar on each preview (`resource_pack_compat`), and entity textures
//! that moved folders were aliased back to old names. Now the baker settles
//! each version's spelling once and the mesher reads only the baked pack.
//! This is a LumilioCL local change (see `forks/README.md`, plan
//! schematic-studio T4).

mod common;

use common::{add_texture, Blocks, Fixture};
use schematic_mesher::{BlockPosition, InputBlock, Mesher};

#[test]
fn a_pack_in_26_3_spelling_meshes_as_written() {
    let mut pack = Fixture::default();
    pack.file("version.json", r#"{"id":"26.3","world_version":5023}"#);
    // Numeric multipart values, `sprite` texture objects and `bottom`/`top`
    // face and cull-face names are 26.x spellings (Euler rotations are baked
    // in `element_rotation`).
    pack.blockstate(
        "lamp",
        r#"{"multipart":[{"when":{"power":0},"apply":{"model":"minecraft:block/lamp"}}]}"#,
    );
    pack.model(
        "block/lamp",
        r##"{"textures":{"side":{"sprite":"minecraft:block/lamp","force_translucent":true}},
            "elements":[{"from":[0,0,0],"to":[16,16,16],
              "faces":{"bottom":{"texture":"#side","cullface":"bottom"},
                       "up":{"texture":"#side","cullface":"top"},
                       "north":{"texture":"#side"},"south":{"texture":"#side"},
                       "west":{"texture":"#side"},"east":{"texture":"#side"}}}]}"##,
    );
    add_texture(&mut pack, "block/lamp", false);
    add_texture(&mut pack, "entity/banner/banner_base", false);
    let pack = pack.bake();

    // The entity code's old flat name finds the moved texture.
    assert!(pack.get_texture("entity/banner_base").is_some());

    let lamp = InputBlock::new("minecraft:lamp").with_property("power", "0");
    let below = (
        BlockPosition::new(0, -1, 0),
        InputBlock::new("minecraft:lamp").with_property("power", "0"),
    );
    let one = Mesher::new(pack.clone())
        .mesh(&Blocks(vec![(BlockPosition::new(0, 0, 0), lamp.clone())]))
        .unwrap();
    // Six faces of four vertices: the condition matched and every face read.
    assert_eq!(one.total_vertices(), 24);
    let keys: Vec<&String> = one.atlas.regions.keys().collect();
    assert!(
        keys.iter().any(|key| *key == "minecraft:block/lamp"),
        "{keys:?}"
    );
    assert!(
        !keys.iter().any(|key| key.ends_with("block/missing")),
        "{keys:?}"
    );
    // `bottom` and `top` cull faces hide the faces two lamps share.
    let two = Mesher::new(pack)
        .mesh(&Blocks(vec![(BlockPosition::new(0, 0, 0), lamp), below]))
        .unwrap();
    assert_eq!(two.total_vertices(), 40);
}

/// Vanilla models name the same texture both as `block/dirt` and as
/// `minecraft:block/dirt`. Before T4 the mesher keyed the atlas by the
/// spelling, so such a texture took two atlas slots and moved its neighbours;
/// the baked pack names it once. That move of atlas slots (and three entity
/// texture renames) was the whole T4 pixel residual: the pre-T4 mesher with
/// only its atlas keys spelled the baked way rendered the 14 comparison
/// pictures byte for byte as the baked path did (plan schematic-studio, T4).
#[test]
fn a_texture_written_with_and_without_its_namespace_is_one_atlas_slot() {
    let mut pack = Fixture::default();
    for (block, texture) in [("bare", "block/dirt"), ("spelled", "minecraft:block/dirt")] {
        common::add_block(
            &mut pack,
            block,
            &format!(
                r##"{{"textures":{{"all":"{texture}"}},
                    "elements":[{{"from":[0,0,0],"to":[16,16,16],
                      "faces":{{"up":{{"texture":"#all"}},"north":{{"texture":"#all"}}}}}}]}}"##
            ),
        );
    }
    add_texture(&mut pack, "block/dirt", false);
    let mesh = Mesher::new(pack.bake())
        .mesh(&Blocks(vec![
            (
                BlockPosition::new(0, 0, 0),
                InputBlock::new("minecraft:bare"),
            ),
            (
                BlockPosition::new(2, 0, 0),
                InputBlock::new("minecraft:spelled"),
            ),
        ]))
        .unwrap();
    let dirt: Vec<&String> = mesh
        .atlas
        .regions
        .keys()
        .filter(|key| key.ends_with("block/dirt"))
        .collect();
    assert_eq!(dirt, ["minecraft:block/dirt"]);
}

/// Every hash map in the mesher gets its own random seed, so meshing the same
/// blocks twice in one process catches any output that follows hash order.
/// The previews compared for T4 were identical run to run before and after.
#[test]
fn meshing_the_same_blocks_twice_gives_the_same_atlas_and_buffers() {
    let mut pack = Fixture::default();
    let mut blocks = Vec::new();
    for (i, name) in ["a", "b", "c", "d", "e", "f"].into_iter().enumerate() {
        common::add_block(
            &mut pack,
            name,
            &format!(
                r##"{{"textures":{{"top":"block/{name}_top","side":"minecraft:block/{name}"}},
                    "elements":[{{"from":[0,0,0],"to":[16,16,16],
                      "faces":{{"up":{{"texture":"#top","cullface":"up"}},
                                "down":{{"texture":"#side","cullface":"down"}},
                                "north":{{"texture":"#side","cullface":"north"}},
                                "south":{{"texture":"#side","cullface":"south"}},
                                "west":{{"texture":"#side","cullface":"west"}},
                                "east":{{"texture":"#side","cullface":"east"}}}}}}]}}"##
            ),
        );
        add_texture(&mut pack, &format!("block/{name}"), false);
        add_texture(&mut pack, &format!("block/{name}_top"), i % 2 == 0);
        blocks.push((
            BlockPosition::new(i as i32, 0, 0),
            InputBlock::new(format!("minecraft:{name}")),
        ));
    }
    let pack = pack.bake();
    let mesh = || {
        Mesher::new(pack.clone())
            .mesh(&Blocks(blocks.clone()))
            .unwrap()
    };
    let (one, two) = (mesh(), mesh());
    assert_eq!(one.atlas.pixels, two.atlas.pixels);
    let regions = |m: &schematic_mesher::MesherOutput| {
        let mut regions: Vec<_> = m
            .atlas
            .regions
            .iter()
            .map(|(k, r)| (k.clone(), [r.u_min, r.v_min, r.u_max, r.v_max]))
            .collect();
        regions.sort_by(|a, b| a.0.cmp(&b.0));
        regions
    };
    assert_eq!(regions(&one), regions(&two));
    for (a, b) in [
        (&one.opaque_mesh, &two.opaque_mesh),
        (&one.cutout_mesh, &two.cutout_mesh),
        (&one.transparent_mesh, &two.transparent_mesh),
    ] {
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.normals, b.normals);
        assert_eq!(a.uvs, b.uvs);
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.indices, b.indices);
    }
    assert!(one.total_vertices() > 0);
}
