use std::collections::HashMap;

use lumilio_block_assets::{BakedPack, Bone, Cube, EntityLayer, EntityLayers};
use schematic_mesher::{Mesher, ResourcePack};

use super::*;
use crate::entity::{ArmorStandEquipment, Entity, NbtValue};

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|axis| (a[axis] - b[axis]).abs() < 1e-5)
}

fn mob(id: &str, position: (f64, f64, f64), yaw: f32) -> Entity {
    let mut entity = Entity::new(format!("minecraft:{id}"), position);
    entity.nbt.insert(
        "Rotation".to_owned(),
        NbtValue::List(vec![NbtValue::Float(yaw), NbtValue::Float(0.0)]),
    );
    entity
}

/// One layer: a single 8×8×8 box, enough for the mesher to draw a row.
fn one_box_layer() -> EntityLayer {
    let mut root = Bone {
        pivot: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
        cubes: Vec::new(),
        children: Default::default(),
    };
    root.cubes.push(Cube {
        origin: [-4.0, 16.0, -4.0],
        size: [8.0, 8.0, 8.0],
        uv: [0.0, 0.0],
        uv_scale: [1.0; 2],
        inflate: [0.0; 3],
        mirror: false,
        faces: None,
    });
    EntityLayer {
        texture_size: [64, 64],
        root,
    }
}

fn mesher(layers: EntityLayers) -> Mesher {
    Mesher::new(ResourcePack::from_baked(&BakedPack::default()).with_entity_layers(layers))
}

#[test]
fn south_is_the_meshed_model_and_east_turns_its_face_to_plus_x() {
    let south = turned_at([10.0, 64.0, -3.0], 0.0, MODEL_FEET);
    // The model's feet land on the given point.
    assert!(close(
        transform_point(&south, MODEL_FEET),
        [10.0, 64.0, -3.0]
    ));
    // Its face (+Z) stays +Z.
    assert!(close(
        transform_point(&south, [0.0, 0.5, 1.0]),
        [10.0, 65.0, -2.0]
    ));
    // Minecraft yaw -90 (= 270) looks east: the face points +X.
    let east = turned_at([0.0; 3], -90.0, MODEL_FEET);
    assert!(close(
        transform_point(&east, [0.0, -0.5, 1.0]),
        [1.0, 0.0, 0.0]
    ));
    // Yaw 90 looks west.
    let west = turned_at([0.0; 3], 90.0, MODEL_FEET);
    assert!(close(
        transform_point(&west, [0.0, -0.5, 1.0]),
        [-1.0, 0.0, 0.0]
    ));
}

/// A mob whose row turns by facing is placed at its exact position and yaw,
/// drawn once facing south; an item frame or an id without a row stays in
/// its block as before.
#[test]
fn bound_mobs_turn_by_yaw_and_others_keep_their_block() {
    let (look, transform, _) = placed(&mob("zombie", (2.25, 1.5, -0.75), 30.0), &Looks::new());
    assert_eq!(look.name, "entity:zombie");
    assert_eq!(
        look.properties.get("facing").map(String::as_str),
        Some("south")
    );
    // The mesher's block n spans n - 0.5 ..= n + 0.5.
    assert!(close(
        transform_point(&transform, MODEL_FEET),
        [1.75, 1.0, -1.25]
    ));
    assert_eq!(transform, turned_at([1.75, 1.0, -1.25], 30.0, MODEL_FEET));

    for id in ["item_frame", "fox"] {
        let (_, transform, _) = placed(&mob(id, (2.25, 1.5, -0.75), 30.0), &Looks::new());
        assert_eq!(
            transform,
            turned_at([2.0, 1.0, -1.0], 0.0, [0.0; 3]),
            "{id}"
        );
    }
}

/// Entities that differ only in place and yaw share one model; any property
/// the table reads (here a pose) makes another.
#[test]
fn place_and_yaw_do_not_split_a_look() {
    let stand =
        |x: f64, yaw: f32| Entity::armor_stand((x, 0.0, 0.0), yaw, ArmorStandEquipment::default());
    let mut posed = stand(9.0, 0.0);
    posed.nbt.insert(
        "HeadPose".to_owned(),
        NbtValue::List(vec![
            NbtValue::Float(20.0),
            NbtValue::Float(0.0),
            NbtValue::Float(0.0),
        ]),
    );
    let (looks, instances) = group(
        &[stand(0.0, 0.0), stand(3.0, 45.0), stand(6.0, 180.0), posed],
        &Looks::new(),
    );
    assert_eq!(looks.len(), 2);
    assert_eq!(
        instances.iter().map(|i| i.model).collect::<Vec<_>>(),
        [0, 0, 0, 1]
    );
}

/// Every entity with a row and a layer becomes an instance of one model per
/// look; an id with no row, or a row whose layer this version lacks, is
/// reported instead of drawn.
#[test]
fn unbound_and_layerless_entities_are_reported_not_drawn() {
    let layers: EntityLayers = [("minecraft:zombie#main".to_owned(), one_box_layer())].into();
    let entities = [
        mob("zombie", (0.5, 0.0, 0.5), 0.0),
        mob("zombie", (4.5, 0.0, 0.5), 90.0),
        mob("fox", (8.5, 0.0, 0.5), 0.0),
        mob("skeleton", (12.5, 0.0, 0.5), 0.0),
        mob("fox", (16.5, 0.0, 0.5), 0.0),
    ];
    let built = instances(&mesher(layers), &entities).unwrap();
    assert_eq!(built.models.len(), 1);
    assert_eq!(built.models[0].name, "entity:zombie");
    assert_eq!(built.instances.len(), 2);
    assert_eq!(built.undrawable, ["minecraft:fox", "minecraft:skeleton"]);

    // The box is 8 px wide about the model's centre line.
    let model = &built.models[0];
    assert_eq!(model.mesh.total_vertices(), 24);
    // Its feet on the model's feet point, 8 px about the centre line.
    assert!(close(model.mesh.bounds.min, [-0.25, -0.5, -0.25]));
    assert!(close(model.mesh.bounds.max, [0.25, 0.0, 0.25]));
    assert!(close(model.center, [0.0, -0.25, 0.0]));
    let (min, max) = built.bounds().unwrap();
    // Zombies at x 0.5 and 4.5 stand in mesher cells 0 and 4.
    assert!(
        min[0] < 0.0 && max[0] > 4.0 && max[0] < 4.5,
        "{min:?} {max:?}"
    );
}

#[test]
fn variant_textures_come_from_any_versions_nbt() {
    let variants = |id: &str, nbt: Vec<(&str, NbtValue)>| {
        let nbt: HashMap<String, NbtValue> =
            nbt.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        let mut input = InputBlock::new(format!("entity:{id}"));
        add_variants(id, &nbt, &mut input);
        let mut properties: Vec<_> = input.properties.into_iter().collect();
        properties.sort();
        properties
    };
    let pair = |k: &str, v: &str| (k.to_owned(), v.to_owned());
    assert_eq!(
        variants(
            "cow",
            vec![("variant", NbtValue::String("minecraft:warm".into()))]
        ),
        [pair("variant", "warm")]
    );
    // White markings over a dark brown coat: the coat is the low byte.
    assert_eq!(
        variants("horse", vec![("Variant", NbtValue::Int(0x0106))]),
        [pair("variant", "darkbrown")]
    );
    assert_eq!(
        variants("cat", vec![("CatType", NbtValue::Int(10))]),
        [pair("variant", "all_black")]
    );
    let villager = NbtValue::Compound(
        [
            ("type".to_owned(), NbtValue::String("minecraft:snow".into())),
            (
                "profession".to_owned(),
                NbtValue::String("minecraft:librarian".into()),
            ),
        ]
        .into(),
    );
    assert_eq!(
        variants("villager", vec![("VillagerData", villager)]),
        [pair("biome", "snow"), pair("profession", "librarian")]
    );
    assert!(variants("zombie", vec![("Variant", NbtValue::Int(3))]).is_empty());
}

/// Plan schematic-studio T33: an entity hung on a block faces the way its
/// save says, whatever its yaw; the save, not an id list, says it is hung.
#[test]
fn a_hung_entity_faces_as_saved_and_others_are_not_hung() {
    let hung = |pairs: &[(&str, NbtValue)]| {
        let nbt: HashMap<String, NbtValue> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect();
        hung_facing(&nbt)
    };
    let tile = ("TileX", NbtValue::Int(3));
    // A floor frame: yaw 0 would say south.
    assert_eq!(
        hung(&[tile.clone(), ("Facing", NbtValue::Byte(1))]),
        Some("up")
    );
    assert_eq!(
        hung(&[
            ("block_pos", NbtValue::IntArray(vec![0, 0, 0])),
            ("Facing", NbtValue::Byte(5))
        ]),
        Some("east")
    );
    // A painting's four-way id: 2 is north.
    assert_eq!(
        hung(&[tile.clone(), ("facing", NbtValue::Byte(2))]),
        Some("north")
    );
    // A leash knot hangs without a facing; a mob is not hung at all.
    assert_eq!(hung(&[tile]), None);
    assert_eq!(hung(&[("Facing", NbtValue::Byte(1))]), None);

    let mut frame = mob("item_frame", (0.5, 0.03125, 0.5), 0.0);
    frame.nbt.insert("TileX".to_owned(), NbtValue::Int(0));
    frame.nbt.insert("Facing".to_owned(), NbtValue::Byte(1));
    frame
        .nbt
        .insert("ItemRotation".to_owned(), NbtValue::Byte(3));
    let (look, transform, _) = placed(&frame, &Looks::new());
    assert_eq!(
        look.properties.get("facing").map(String::as_str),
        Some("up")
    );
    assert_eq!(
        look.properties.get("item_rotation").map(String::as_str),
        Some("3")
    );
    assert!(
        close(transform_point(&transform, [0.0; 3]), [0.0, 0.0, 0.0]),
        "in its own block"
    );
}
