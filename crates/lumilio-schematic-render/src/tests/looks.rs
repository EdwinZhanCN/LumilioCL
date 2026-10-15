//! Entities drawn from their renderer looks (plan schematic-studio T34,
//! D32): a living entity the binding table has no row for is drawn from what
//! the game version's own renderer used, placed as living entities are and
//! turned by its yaw; the table's rows still come first. Meshed only.

use std::io::Write;

use lumilio_block_assets::{
    AssetSource, BakedPack, Bone, Cube, EntityLayer, EntityLayers, EntityLook, bake,
};
use nucleation::meshing::{EntityInstances, MeshConfig, ResourcePackSource};
use nucleation::{Entity, NbtValue, UniversalSchematic};

fn png(rgba: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(64, 64, image::Rgba(rgba)))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn pack() -> BakedPack {
    let files = [
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":34,"description":""}}"#.to_vec(),
        ),
        (
            "assets/minecraft/textures/entity/bee/bee.png",
            png([255, 200, 0, 255]),
        ),
        (
            "assets/minecraft/textures/entity/bee/bee_baby.png",
            png([255, 150, 0, 255]),
        ),
        (
            "assets/minecraft/textures/entity/zombie/zombie.png",
            png([0, 160, 0, 255]),
        ),
    ];
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    bake(&[AssetSource::ArchiveBytes(
        zip.finish().unwrap().into_inner(),
    )])
    .unwrap()
}

/// One box standing on the feet, `side` pixels a side.
fn layer(side: f32) -> EntityLayer {
    let mut root = Bone {
        pivot: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
        cubes: Vec::new(),
        children: Default::default(),
    };
    root.cubes.push(Cube {
        origin: [-side / 2., 24. - side, -side / 2.],
        size: [side; 3],
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

fn living(layers: &[&str], texture: &str, baby_texture: Option<&str>) -> EntityLook {
    EntityLook {
        renderer: vec!["SomeRenderer".into(), "LivingEntityRenderer".into()],
        entity: vec!["Some".into(), "LivingEntity".into(), "Entity".into()],
        layers: layers.iter().map(|layer| (*layer).to_owned()).collect(),
        texture: Some(format!("minecraft:textures/{texture}.png")),
        baby_texture: baby_texture.map(|texture| format!("minecraft:textures/{texture}.png")),
        ..EntityLook::default()
    }
}

fn layers(with_looks: bool) -> EntityLayers {
    let mut layers: EntityLayers = [
        ("minecraft:bee#main".to_owned(), layer(8.)),
        ("minecraft:bee_baby#main".to_owned(), layer(4.)),
        ("minecraft:zombie#main".to_owned(), layer(8.)),
    ]
    .into();
    if with_looks {
        layers.looks.insert(
            "minecraft:bee".into(),
            living(
                &["minecraft:bee#main", "minecraft:bee_baby#main"],
                "entity/bee/bee",
                Some("entity/bee/bee_baby"),
            ),
        );
        // The table has a zombie row; a look that would draw nothing must
        // not take its place.
        layers.looks.insert(
            "minecraft:zombie".into(),
            living(&["minecraft:no_such#main"], "entity/zombie/zombie", None),
        );
    }
    layers
}

fn mob(id: &str, x: f64, yaw: f32, baby: bool) -> Entity {
    let mut entity = Entity::new(format!("minecraft:{id}"), (x, 0.0, 0.5));
    entity.nbt.insert(
        "Rotation".into(),
        NbtValue::List(vec![NbtValue::Float(yaw), NbtValue::Float(0.0)]),
    );
    if baby {
        entity.nbt.insert("Age".into(), NbtValue::Int(-24000));
    }
    entity
}

fn meshed(layers: EntityLayers) -> EntityInstances {
    let mut schematic = UniversalSchematic::new("looks".into());
    schematic.add_entity(mob("bee", 0.5, 0.0, false));
    schematic.add_entity(mob("bee", 2.5, 90.0, false));
    schematic.add_entity(mob("bee", 4.5, 0.0, true));
    schematic.add_entity(mob("zombie", 6.5, 0.0, false));
    let pack = pack();
    let source = ResourcePackSource::from_baked(&pack, layers);
    schematic
        .to_mesh_with_entity_instances(&source, &MeshConfig::default())
        .unwrap()
        .1
}

fn height(entities: &EntityInstances, model: usize) -> f32 {
    let mesh = &entities.models[model].mesh;
    let ys = [&mesh.opaque, &mesh.cutout, &mesh.transparent]
        .into_iter()
        .flat_map(|layer| layer.positions.iter().map(|p| p[1]));
    let (low, high) = ys.fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(y), hi.max(y)));
    high - low
}

#[test]
fn a_living_entity_without_a_row_is_drawn_from_its_look() {
    let entities = meshed(layers(true));
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    assert_eq!(entities.instances.len(), 4, "three bees and the zombie");
    let model = |at: usize| entities.instances[at].model;
    // Turned by their yaw: two adults, one model, two turns.
    assert_eq!(model(0), model(1));
    assert_ne!(
        entities.instances[0].transform, entities.instances[1].transform,
        "each bee turned its own way"
    );
    // The baby draws its own, smaller layer.
    assert_ne!(model(2), model(0));
    let (adult, baby) = (height(&entities, model(0)), height(&entities, model(2)));
    assert!(
        baby < adult * 0.75,
        "the baby layer: {baby} against {adult}"
    );
}

#[test]
fn without_its_look_it_is_reported_and_the_table_still_comes_first() {
    let entities = meshed(layers(false));
    assert_eq!(entities.undrawable, ["minecraft:bee"]);
    // The zombie is drawn by its row both times: its look names a layer
    // that does not exist and would have left it out.
    assert_eq!(entities.instances.len(), 1);
    let with = meshed(layers(true));
    assert!(with.undrawable.is_empty());
}
