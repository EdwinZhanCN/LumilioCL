//! Regression test: a model's `ambientocclusion` and an element's 26.x
//! `shade_direction_override` decide how its faces are shaded.
//!
//! Backstory: the mesher read `ambientocclusion` but never used it, and a child
//! model that did not set it switched it back on, so cross plants, torches,
//! doors, rails and panes picked up dark ambient-occlusion corners Minecraft
//! does not draw. Minecraft 26.x lights plants, torches and redstone dust as if
//! every face pointed up; ignoring `shade_direction_override` left one side of
//! each cross in shadow. This is a LumilioCL local change (see `forks/README.md`).

mod common;

use common::{add_block, add_texture, cube_model, Blocks};
use schematic_mesher::{BlockPosition, InputBlock, Mesher, ResourcePack};

fn at(x: i32, y: i32, z: i32) -> BlockPosition {
    BlockPosition::new(x, y, z)
}

/// The darkest vertex color of the block at the origin, with stone around the
/// top of it so ambient occlusion would darken its upper corners.
fn darkest_corner(plant_parent: &str) -> f32 {
    let mut pack = ResourcePack::new();
    add_block(&mut pack, "stone", &cube_model("minecraft:block/stone"));
    add_texture(&mut pack, "block/stone", false);
    add_texture(&mut pack, "block/plant", false);
    let parent = cube_model("minecraft:block/plant").replacen('{', plant_parent, 1);
    pack.add_model("minecraft", "block/parent", serde_json::from_str(&parent).unwrap());
    add_block(
        &mut pack,
        "plant",
        r#"{"parent":"minecraft:block/parent","textures":{"all":"minecraft:block/plant"}}"#,
    );
    let mut blocks = vec![(at(0, 0, 0), InputBlock::new("minecraft:plant"))];
    for (x, z) in [(1, 0), (0, 1), (1, 1), (-1, 0), (0, -1)] {
        blocks.push((at(x, 1, z), InputBlock::new("minecraft:stone")));
    }
    let out = Mesher::new(pack).mesh(&Blocks(blocks)).unwrap();
    // Faces are four consecutive vertices; the plant's faces are centred on its
    // own block, the stone's are not.
    let layer = &out.opaque_mesh;
    let plant_faces = (0..layer.positions.len() / 4).filter(|face| {
        let corners = &layer.positions[face * 4..face * 4 + 4];
        (0..3).all(|axis| (corners.iter().map(|p| p[axis]).sum::<f32>() / 4.0).abs() < 0.51)
    });
    plant_faces
        .flat_map(|face| &layer.colors[face * 4..face * 4 + 4])
        .map(|color| color[0])
        .fold(f32::MAX, f32::min)
}

#[test]
fn a_parent_without_ambient_occlusion_keeps_the_child_flat() {
    assert!(darkest_corner("{") < 0.99, "ambient occlusion applies by default");
    assert!(darkest_corner(r#"{"ambientocclusion":false,"#) > 0.999);
}

#[test]
fn shade_direction_override_lights_faces_as_if_they_faced_up() {
    let element = |extra: &str| {
        format!(
            r##"{{"textures":{{"all":"minecraft:block/cross"}},"elements":[{{"from":[8,0,0],"to":[8,16,16],{extra}"rotation":{{"origin":[8,8,8],"axis":"y","angle":45,"rescale":true}},"faces":{{"west":{{"texture":"#all"}},"east":{{"texture":"#all"}}}}}}]}}"##
        )
    };
    let normals = |model: String| {
        let mut pack = ResourcePack::new();
        add_block(&mut pack, "cross", &model);
        add_texture(&mut pack, "block/cross", false);
        let source = Blocks(vec![(at(0, 0, 0), InputBlock::new("minecraft:cross"))]);
        Mesher::new(pack).mesh(&source).unwrap().opaque_mesh.normals
    };
    let plain = normals(element(""));
    assert!(plain.iter().all(|n| n[1].abs() < 1e-4), "{plain:?}");
    let up = normals(element(r#""shade_direction_override":"up","#));
    assert_eq!(up.len(), 8);
    assert!(up.iter().all(|n| (n[1] - 1.0).abs() < 1e-4), "{up:?}");
}
