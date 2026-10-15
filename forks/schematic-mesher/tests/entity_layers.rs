//! Regression test: block entities and entities are drawn from the game's own
//! model layers through the binding table, with the poses, overlays, armor and
//! baby fallbacks its rows ask for.
//!
//! Backstory: schematic-mesher drew them from about 38 hand-copied geometry
//! files, each a guess at one game version; a new version meant a new copy.
//! The layers now come baked from the game (`entities.dat.z`), so a row is the
//! only thing written by hand. This is a LumilioCL local change (see
//! `forks/README.md`, plan schematic-studio T7).

mod common;

use common::{entity_layers, entity_pack};
use schematic_mesher::mesher::entity::bindings::find;
use schematic_mesher::mesher::entity::model::Geometry;
use schematic_mesher::mesher::entity::render::{draw, Look};
use schematic_mesher::{InputBlock, ResourcePack};

fn drawn(pack: &ResourcePack, block: InputBlock) -> Option<Geometry> {
    draw(pack, &find(&block).expect("a binding"), &Look::default())
}

fn faces(block: InputBlock) -> usize {
    drawn(&entity_pack(), block).map_or(0, |(_, _, faces)| faces.len())
}

fn height(geometry: &Geometry) -> f32 {
    geometry
        .0
        .iter()
        .map(|v| v.position[1])
        .fold(f32::MIN, f32::max)
}

#[test]
fn layers_draw_with_the_faces_the_game_draws() {
    // Three boxes of six faces.
    assert_eq!(faces(InputBlock::new("minecraft:chest")), 18);
    // A double chest half leaves out the faces where the halves meet.
    assert_eq!(
        faces(InputBlock::new("minecraft:chest").with_property("type", "left")),
        15
    );
    // The shulker box's row hides the base top and lid bottom inside each other.
    assert_eq!(faces(InputBlock::new("minecraft:shulker_box")), 10);
    // An open book: seven pages and covers, none of them flat.
    assert_eq!(
        faces(InputBlock::new("minecraft:lectern").with_property("has_book", "true")),
        42
    );
    let (_, _, pot) = drawn(&entity_pack(), InputBlock::new("minecraft:decorated_pot")).unwrap();
    // Each pot side is one face; bottom and top are flat, so one face each.
    assert!(pot
        .iter()
        .any(|face| face.texture == "entity/decorated_pot/decorated_pot_side"));
    assert!(pot
        .iter()
        .any(|face| face.texture == "entity/decorated_pot/decorated_pot_base"));
}

#[test]
fn a_version_without_the_layer_draws_nothing() {
    let pack = ResourcePack::new();
    assert!(drawn(&pack, InputBlock::new("minecraft:chest")).is_none());
    assert!(drawn(&entity_pack(), InputBlock::new("entity:ghast")).is_none());
}

#[test]
fn worn_armor_uses_the_slot_layer_or_the_older_shared_one() {
    let bare = faces(InputBlock::new("entity:armor_stand"));
    let helmet =
        InputBlock::new("entity:armor_stand").with_property("helmet", "minecraft:iron_helmet");
    let (_, _, worn) = drawn(&entity_pack(), helmet.clone()).unwrap();
    let armor: Vec<_> = worn
        .iter()
        .filter(|face| face.texture == "entity/equipment/humanoid/iron")
        .collect();
    // 26.3's helmet layer: a head and a hat box.
    assert_eq!(armor.len(), 12);
    assert_eq!(worn.len(), bare + 12);

    // Before slot layers, the helmet is the head and hat of the outer armor.
    let mut older = entity_layers();
    older.remove("minecraft:armor_stand#helmet");
    let pack = ResourcePack::new().with_entity_layers(older);
    let (_, _, worn) = drawn(&pack, helmet).unwrap();
    let armor = worn
        .iter()
        .filter(|face| face.texture == "entity/equipment/humanoid/iron")
        .count();
    assert_eq!(armor, 12);
}

#[test]
fn overlays_follow_the_entity_state() {
    let pig = InputBlock::new("entity:pig");
    let plain = faces(pig.clone());
    let saddled = drawn(&entity_pack(), pig.with_property("saddle", "true")).unwrap();
    assert!(saddled.2.len() > plain);
    assert!(saddled
        .2
        .iter()
        .any(|face| face.texture == "entity/equipment/pig_saddle/saddle"));

    let sheep = drawn(
        &entity_pack(),
        InputBlock::new("entity:sheep").with_property("color", "red"),
    )
    .unwrap();
    let wool: Vec<_> = sheep
        .2
        .iter()
        .zip(sheep.0.chunks(4))
        .filter(|(face, _)| face.texture == "entity/sheep/sheep_wool")
        .collect();
    assert!(!wool.is_empty());
    // Dyed red: the wool's vertex colour keeps red over green.
    assert!(wool.iter().all(|(_, v)| v[0].color[0] > v[0].color[1]));
}

#[test]
fn a_baby_without_its_own_layer_is_the_adult_scaled_down() {
    let adult = drawn(&entity_pack(), InputBlock::new("entity:zombie")).unwrap();
    let baby = drawn(
        &entity_pack(),
        InputBlock::new("entity:zombie").with_property("is_baby", "true"),
    )
    .unwrap();
    assert_eq!(adult.2.len(), baby.2.len());
    assert!(
        height(&baby) < height(&adult) * 0.7,
        "{} {}",
        height(&baby),
        height(&adult)
    );
    // Still standing on the ground.
    let lowest = baby
        .0
        .iter()
        .map(|v| v.position[1])
        .fold(f32::MAX, f32::min);
    assert!(lowest.abs() < 1e-3, "{lowest}");
}

#[test]
fn pose_properties_turn_their_bone() {
    let stand = InputBlock::new("entity:armor_stand").with_property("facing", "south");
    let raised = stand.clone().with_property("RightArmPose", "-90,0,0");
    let (a, _, _) = drawn(&entity_pack(), stand).unwrap();
    let (b, _, _) = drawn(&entity_pack(), raised).unwrap();
    assert_eq!(a.len(), b.len());
    assert!(a
        .iter()
        .zip(&b)
        .any(|(a, b)| (a.position[2] - b.position[2]).abs() > 0.1));
}

/// From 1.21.2 a layer's root bone carries its renderer's scale (a cat's 0.8);
/// older layers do not and the row's `renderer_scale` adds it. Either way the
/// cat is the same size and stands on the ground, never scaled twice.
#[test]
fn a_renderer_scale_is_applied_once() {
    use lumilio_block_assets::{Bone, Cube, EntityLayer};

    let layer = |pivot_y: f32, scale: f32| {
        let mut body = Bone {
            pivot: [0.0, 14.0, 0.0],
            rotation: [0.0; 3],
            scale: [1.0; 3],
            cubes: vec![Cube {
                origin: [-2.0, 0.0, -2.0],
                size: [4.0, 10.0, 4.0],
                uv: [0.0, 0.0],
                uv_scale: [1.0; 2],
                inflate: [0.0; 3],
                mirror: false,
                faces: None,
            }],
            children: Default::default(),
        };
        body.pivot[1] = 14.0;
        let mut root = Bone {
            pivot: [0.0, pivot_y, 0.0],
            rotation: [0.0; 3],
            scale: [scale; 3],
            cubes: Vec::new(),
            children: Default::default(),
        };
        root.children.insert("body".to_owned(), body);
        EntityLayer {
            texture_size: [64, 32],
            root,
        }
    };
    let drawn_with = |layer: EntityLayer| {
        let layers = [("minecraft:cat#main".to_owned(), layer)].into();
        drawn(
            &ResourcePack::new().with_entity_layers(layers),
            InputBlock::new("entity:cat"),
        )
        .unwrap()
    };
    // The game's own baking: about the feet, model y 24.
    let older = drawn_with(layer(0.0, 1.0));
    let newer = drawn_with(layer(24.0 - 24.0 * 0.8, 0.8));
    let size = |g: &Geometry| {
        (
            height(g),
            g.0.iter().map(|v| v.position[1]).fold(f32::MAX, f32::min),
        )
    };
    let (older, newer) = (size(&older), size(&newer));
    assert!((older.0 - newer.0).abs() < 1e-4, "{older:?} {newer:?}");
    assert!(
        older.1.abs() < 1e-4 && newer.1.abs() < 1e-4,
        "{older:?} {newer:?}"
    );
    // 10 pixels tall at 0.8.
    assert!((older.0 - 0.5).abs() < 1e-4, "{older:?}");
}
