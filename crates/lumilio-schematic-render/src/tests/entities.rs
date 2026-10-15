//! Entities drawn as instances (plan schematic-studio T8).

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use lumilio_block_assets::{
    AssetSource, BakedPack, Bone, Bundle, Cube, EntityBundle, EntityLayer, EntityLayers, bake,
};
use nucleation::{ArmorStandEquipment, Entity, NbtValue, UniversalSchematic};

use crate::scene::rgba_to_bgra;
use crate::{Scene, SceneError, View};

fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        width,
        height,
        image::Rgba(rgba),
    ))
    .write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    bytes
}

/// Red stone and a blue armor stand texture, baked as the launcher bakes a jar.
fn stand_pack() -> BakedPack {
    let face = r##"{"texture":"#all"}"##;
    let model = format!(
        r#"{{"textures":{{"all":"minecraft:block/stone"}},"elements":[{{"from":[0,0,0],"to":[16,16,16],"faces":{{"down":{face},"up":{face},"north":{face},"south":{face},"west":{face},"east":{face}}}}}]}}"#
    );
    let red = png(16, 16, [255, 0, 0, 255]);
    let blue = png(64, 64, [0, 0, 255, 255]);
    let files: [(&str, &[u8]); 5] = [
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":34,"description":""}}"#,
        ),
        (
            "assets/minecraft/blockstates/stone.json",
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        ),
        ("assets/minecraft/models/block/stone.json", model.as_bytes()),
        ("assets/minecraft/textures/block/stone.png", &red),
        (
            "assets/minecraft/textures/entity/armorstand/armorstand.png",
            &blue,
        ),
    ];
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
    }
    bake(&[AssetSource::ArchiveBytes(
        zip.finish().unwrap().into_inner(),
    )])
    .unwrap()
}

/// An armor stand layer: one 8×24×8 px box standing on the feet.
fn stand_layers() -> EntityLayers {
    let mut root = Bone {
        pivot: [0.0; 3],
        rotation: [0.0; 3],
        scale: [1.0; 3],
        cubes: Vec::new(),
        children: Default::default(),
    };
    root.cubes.push(Cube {
        origin: [-4.0, 0.0, -4.0],
        size: [8.0, 24.0, 8.0],
        uv: [0.0, 0.0],
        uv_scale: [1.0; 2],
        inflate: [0.0; 3],
        mirror: false,
        faces: None,
    });
    let layer = EntityLayer {
        texture_size: [64, 64],
        root,
    };
    [("minecraft:armor_stand#main".to_owned(), layer)]
        .into_iter()
        .collect()
}

fn stand(x: f64, z: f64, yaw: f32) -> Entity {
    Entity::armor_stand((x, 1.0, z), yaw, ArmorStandEquipment::default())
}

fn scene(schematic: &UniversalSchematic, pack: &BakedPack, layers: EntityLayers) -> Option<Scene> {
    match Scene::from_schematic(schematic.clone(), pack, layers) {
        Ok(scene) => Some(scene),
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

/// BGRA pixels whose blue clearly beats red and green.
fn blue_pixels(frame: &crate::Frame) -> usize {
    frame
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| {
            px[0] > 100 && px[0] > px[1].saturating_add(60) && px[0] > px[2].saturating_add(60)
        })
        .count()
}

/// Bound entities are drawn from the layers handed in, as instances; an entity without a row
/// is reported like an undrawable block, and so is a bound one whose layer is missing.
#[test]
fn entities_are_drawn_as_instances_and_unbound_ones_are_reported() {
    let mut schematic = UniversalSchematic::new("stands".into());
    schematic.fill_cuboid_str((0, 0, 0), (6, 0, 6), "minecraft:stone");
    for (x, z, yaw) in [(1.5, 1.5, 0.0), (3.5, 3.5, 45.0), (5.5, 5.5, -90.0)] {
        schematic.add_entity(stand(x, z, yaw));
    }
    schematic.add_entity(Entity::new("minecraft:fox".into(), (3.5, 1.0, 1.5)));
    let pack = stand_pack();

    let Some(mut drawn) = scene(&schematic, &pack, stand_layers()) else {
        return;
    };
    assert_eq!(drawn.entity_count(), 3);
    assert_eq!(drawn.undrawable_entities(), ["minecraft:fox"]);
    assert!(drawn.undrawable_blocks().is_empty());
    let frame = drawn.render(&View::new(), 160, 120).unwrap();
    let blue = blue_pixels(&frame);
    assert!(blue > 30, "the stands are drawn: {blue} blue pixels");

    let Some(mut bare) = scene(&schematic, &pack, EntityLayers::new()) else {
        return;
    };
    assert_eq!(bare.entity_count(), 0);
    assert_eq!(
        bare.undrawable_entities(),
        ["minecraft:armor_stand", "minecraft:fox"]
    );
    let frame = bare.render(&View::new(), 160, 120).unwrap();
    assert_eq!(blue_pixels(&frame), 0);
}

/// A schematic of entities and no blocks still has a picture.
#[test]
fn a_schematic_of_only_entities_is_drawn() {
    let mut schematic = UniversalSchematic::new("stands".into());
    schematic.add_entity(stand(0.5, 0.5, 0.0));
    let Some(mut scene) = scene(&schematic, &stand_pack(), stand_layers()) else {
        return;
    };
    let frame = scene.render(&View::new(), 160, 120).unwrap();
    assert!(blue_pixels(&frame) > 30);
}

/// `target/block-assets/` (`cargo xtask bake-blocks`), or the bundles named by
/// `LUMILIO_BLOCK_BUNDLE` and `LUMILIO_ENTITIES`.
fn real_game(version: &str) -> Option<(BakedPack, EntityLayers)> {
    let file = |variable: &str, name: &str| {
        std::env::var_os(variable)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/block-assets")
                    .join(name)
            })
    };
    let blocks = std::fs::read(file("LUMILIO_BLOCK_BUNDLE", "blocks.dat.z")).ok()?;
    let entities = std::fs::read(file("LUMILIO_ENTITIES", "entities.dat.z")).ok()?;
    let pack = Bundle::read(&blocks).unwrap().pack(version).unwrap();
    let layers = EntityBundle::read(&entities).unwrap().layers_of(version);
    Some((pack, layers))
}

fn average_frame(scene: &mut Scene, frames: u32) -> Duration {
    for n in 0..5 {
        scene
            .render(&View::new().orbit(n as f32 * 7.0, 0.), 1800, 1200)
            .unwrap();
    }
    let started = Instant::now();
    for n in 0..frames {
        std::hint::black_box(
            scene
                .render(&View::new().orbit(n as f32 * 3.0, 0.), 1800, 1200)
                .unwrap(),
        );
    }
    started.elapsed() / frames
}

fn save(scene: &mut Scene, view: &View, name: &str) {
    let Some(dir) = std::env::var_os("LUMILIO_FRAME_DIR") else {
        return;
    };
    let mut frame = scene.render(view, 1800, 1200).unwrap();
    rgba_to_bgra(&mut frame.bgra);
    let path = PathBuf::from(dir).join(name);
    image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
        .unwrap()
        .save(&path)
        .unwrap();
    eprintln!("saved {}", path.display());
}

/// Acceptance of plan schematic-studio T8: 500 armor stands cost at most 1.5× the frame time of
/// the same build without them. Measures `Scene::render` (draw and read back, 1800×1200) on the
/// real 26.3 bundle. `LUMILIO_FRAME_DIR` also saves the frame.
#[test]
#[ignore = "release performance measurement; requires a GPU and the baked bundle"]
fn five_hundred_armor_stands_frame_time() {
    let Some((pack, layers)) = real_game("26.3") else {
        eprintln!("skipped: no baked bundle in target/block-assets (cargo xtask bake-blocks)");
        return;
    };
    let mut build = UniversalSchematic::new("stands".into());
    build.fill_cuboid_str((0, 0, 0), (47, 0, 47), "minecraft:stone_bricks");
    build.fill_cuboid_str((0, 1, 0), (47, 3, 0), "minecraft:oak_planks");
    let mut stands = build.clone();
    let armored = ArmorStandEquipment {
        helmet: Some("minecraft:iron_helmet".into()),
        chestplate: Some("minecraft:iron_chestplate".into()),
        leggings: Some("minecraft:iron_leggings".into()),
        boots: Some("minecraft:iron_boots".into()),
    };
    for n in 0..500u32 {
        let (i, j) = (n % 25, n / 25);
        let equipment = if n % 5 == 0 {
            armored.clone()
        } else {
            ArmorStandEquipment::default()
        };
        stands.add_entity(Entity::armor_stand(
            (1.0 + i as f64 * 1.85, 1.0, 2.5 + j as f64 * 2.2),
            ((i * 37 + j * 11) % 360) as f32,
            equipment,
        ));
    }
    let mut without = Scene::from_schematic(build.clone(), &pack, layers.clone()).unwrap();
    let mut with = Scene::from_schematic(stands.clone(), &pack, layers).unwrap();
    assert_eq!(with.entity_count(), 500);
    assert!(with.undrawable_entities().is_empty());
    let base = average_frame(&mut without, 60);
    let entities = average_frame(&mut with, 60);
    let ratio = entities.as_secs_f64() / base.as_secs_f64();
    eprintln!(
        "frame without entities: {base:?}; with 500 armor stands: {entities:?}; ratio {ratio:.3}"
    );
    save(&mut with, &View::new(), "t8-500-armor-stands.png");
    assert!(
        ratio <= 1.5,
        "500 armor stands cost {ratio:.3}× the frame time"
    );
}

fn mob(id: &str, x: f64, z: f64, nbt: &[(&str, NbtValue)]) -> Entity {
    let mut entity = Entity::new(format!("minecraft:{id}"), (x, 1.0, z));
    entity.nbt.insert(
        "Rotation".into(),
        NbtValue::List(vec![NbtValue::Float(0.0), NbtValue::Float(0.0)]),
    );
    for (key, value) in nbt {
        entity.nbt.insert((*key).into(), value.clone());
    }
    entity
}

/// For the Project Maintainer's eyes: every bound mob of the table, and two drawn from their
/// renderer looks (T34), facing the camera, and a real schematic (`LUMILIO_SCHEMATIC`), on 26.3
/// or `LUMILIO_GAME_VERSION`, saved under `LUMILIO_FRAME_DIR`.
#[test]
#[ignore = "visual review; requires a GPU and the baked bundle"]
fn entity_review_frames() {
    let version = std::env::var("LUMILIO_GAME_VERSION").unwrap_or_else(|_| "26.3".to_owned());
    let Some((pack, layers)) = real_game(&version) else {
        eprintln!("skipped: no baked bundle in target/block-assets (cargo xtask bake-blocks)");
        return;
    };
    let string = |s: &str| NbtValue::String(s.into());
    let villager = NbtValue::Compound(
        [
            ("type".to_owned(), string("minecraft:desert")),
            ("profession".to_owned(), string("minecraft:librarian")),
        ]
        .into(),
    );
    let row: Vec<(&str, Vec<(&str, NbtValue)>)> = vec![
        ("zombie", vec![]),
        ("skeleton", vec![]),
        ("wither_skeleton", vec![]),
        ("creeper", vec![]),
        ("enderman", vec![]),
        ("villager", vec![("VillagerData", villager)]),
        ("player", vec![]),
        ("armor_stand", vec![]),
        ("iron_golem", vec![]),
        ("blaze", vec![]),
        ("pig", vec![("variant", string("minecraft:warm"))]),
        ("cow", vec![("variant", string("minecraft:cold"))]),
        ("chicken", vec![]),
        ("sheep", vec![("Color", NbtValue::Byte(14))]),
        ("wolf", vec![]),
        ("cat", vec![("variant", string("minecraft:calico"))]),
        ("horse", vec![("Variant", NbtValue::Int(2))]),
        ("spider", vec![]),
        ("slime", vec![]),
        ("magma_cube", vec![]),
        ("bat", vec![]),
        ("minecart", vec![]),
        ("oak_boat", vec![]),
        ("fox", vec![]),
        // No rows: drawn from what their renderers used (T34).
        ("bee", vec![]),
        ("zombified_piglin", vec![]),
    ];
    let mut zoo = UniversalSchematic::new("zoo".into());
    zoo.fill_cuboid_str(
        (0, 0, 0),
        (3 * row.len() as i32, 0, 6),
        "minecraft:grass_block",
    );
    for (n, (id, nbt)) in row.iter().enumerate() {
        zoo.add_entity(mob(id, 1.5 + 3.0 * n as f64, 3.5, nbt));
    }
    let mut scene = Scene::from_schematic(zoo.clone(), &pack, layers.clone()).unwrap();
    eprintln!("not drawn: {:?}", scene.undrawable_entities());
    let front = View {
        yaw_deg: 0.0,
        pitch_deg: 15.0,
        ..View::new()
    };
    save(&mut scene, &front, "t8-entity-zoo.png");
    save(
        &mut scene,
        &View {
            yaw_deg: 0.0,
            pitch_deg: 10.0,
            zoom: 3.5,
            target: Some([12.0, 1.5, 3.0]),
            ..View::new()
        },
        "t8-entity-zoo-left.png",
    );
    save(
        &mut scene,
        &View {
            yaw_deg: 0.0,
            pitch_deg: 10.0,
            zoom: 3.5,
            target: Some([52.0, 1.5, 3.0]),
            ..View::new()
        },
        "t8-entity-zoo-right.png",
    );

    if let Ok(path) = std::env::var("LUMILIO_SCHEMATIC") {
        let bytes = std::fs::read(path).unwrap();
        let mut real = Scene::load(&bytes, &pack, layers).unwrap();
        eprintln!(
            "real schematic: {} entities, not drawn: {:?}, blocks not drawn: {:?}",
            real.entity_count(),
            real.undrawable_entities(),
            real.undrawable_blocks()
        );
        save(&mut real, &View::new(), "t8-real-schematic.png");
        // Close to its last entity (the mesher's block n spans n - 0.5 ..= n + 0.5).
        let entities = nucleation::formats::litematic::from_litematic(&bytes)
            .unwrap()
            .get_entities_as_list();
        if let Some(first) = entities.last() {
            eprintln!(
                "entities: {:?}",
                entities.iter().map(|e| &e.id).collect::<Vec<_>>()
            );
            let (x, y, z) = first.position;
            save(
                &mut real,
                &View {
                    yaw_deg: 30.0,
                    pitch_deg: 25.0,
                    zoom: 7.0,
                    target: Some([x as f32 - 0.5, y as f32, z as f32 - 0.5]),
                    ..View::new()
                },
                "t8-real-schematic-close.png",
            );
        }
    }
}
