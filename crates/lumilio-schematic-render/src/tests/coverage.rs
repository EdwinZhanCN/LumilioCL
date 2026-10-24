//! Entities drawn by their kind of renderer (plan schematic-studio T35, D32):
//! a row per renderer class turns every entity that renderer draws as it
//! turns it; carried blocks, paintings, sprites and display entities follow
//! what their renderers do with the entity's save. Meshed only.

use std::collections::HashMap;
use std::io::Write;

use lumilio_block_assets::{
    AssetSource, BakedPack, Bone, Cube, EntityLayer, EntityLayers, EntityLook, bake,
};
use nucleation::meshing::{
    BillboardKind, EntityInstances, MeshConfig, ResourcePackSource, transform_point,
};
use nucleation::{Entity, NbtValue, UniversalSchematic};

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

/// An ASCII font sheet whose every glyph is five pixels wide, so a glyph
/// advances six font units.
fn font() -> Vec<u8> {
    let mut sheet = image::RgbaImage::new(128, 128);
    for cell in 0..256u32 {
        let (x0, y0) = ((cell % 16) * 8, (cell / 16) * 8);
        for x in 0..5 {
            for y in 0..7 {
                sheet.put_pixel(x0 + x, y0 + y, image::Rgba([255, 255, 255, 255]));
            }
        }
    }
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(sheet)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn pack() -> BakedPack {
    let stone_model = br##"{"textures":{"all":"minecraft:block/stone"},"elements":[{"from":[0,0,0],"to":[16,16,16],"faces":{"down":{"texture":"#all"},"up":{"texture":"#all"},"north":{"texture":"#all"},"south":{"texture":"#all"},"west":{"texture":"#all"},"east":{"texture":"#all"}}}]}"##;
    let files: Vec<(&str, Vec<u8>)> = vec![
        (
            "pack.mcmeta",
            br#"{"pack":{"pack_format":34,"description":""}}"#.to_vec(),
        ),
        (
            "assets/minecraft/blockstates/stone.json",
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#.to_vec(),
        ),
        (
            "assets/minecraft/models/block/stone.json",
            stone_model.to_vec(),
        ),
        (
            "assets/minecraft/textures/block/stone.png",
            png(16, 16, [255, 0, 0, 255]),
        ),
        (
            "assets/minecraft/textures/entity/projectiles/arrow.png",
            png(32, 32, [200, 200, 200, 255]),
        ),
        (
            "assets/minecraft/textures/entity/projectiles/arrow_spectral.png",
            png(32, 32, [255, 255, 0, 255]),
        ),
        (
            "assets/minecraft/textures/entity/minecart/minecart.png",
            png(64, 32, [90, 90, 90, 255]),
        ),
        (
            "assets/minecraft/textures/painting/kebab.png",
            png(16, 16, [0, 0, 255, 255]),
        ),
        (
            "assets/minecraft/textures/painting/wanderer.png",
            png(16, 32, [0, 0, 255, 255]),
        ),
        (
            "assets/minecraft/textures/painting/back.png",
            png(16, 16, [120, 80, 40, 255]),
        ),
        (
            "assets/minecraft/textures/entity/experience/experience_orb.png",
            png(64, 64, [0, 255, 0, 255]),
        ),
        (
            "assets/minecraft/textures/entity/fish/tropical_b.png",
            png(32, 32, [255, 255, 255, 255]),
        ),
        (
            "assets/minecraft/textures/entity/fish/tropical_b_pattern_3.png",
            png(32, 32, [255, 255, 255, 255]),
        ),
        ("assets/minecraft/textures/font/ascii.png", font()),
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

/// One box, in pixels.
fn layer(origin: [f32; 3], size: [f32; 3]) -> EntityLayer {
    EntityLayer {
        texture_size: [32, 32],
        root: Bone {
            pivot: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
            cubes: vec![Cube {
                origin,
                size,
                uv: [0.0, 0.0],
                uv_scale: [1.0; 2],
                inflate: [0.0; 3],
                mirror: false,
                faces: None,
            }],
            children: Default::default(),
        },
    }
}

fn look(renderer: &[&str], texture: Option<&str>) -> EntityLook {
    EntityLook {
        renderer: renderer.iter().map(|class| (*class).to_owned()).collect(),
        texture: texture.map(|texture| format!("minecraft:textures/{texture}.png")),
        ..EntityLook::default()
    }
}

fn layers() -> EntityLayers {
    // An arrow pointing +X, sixteen pixels long, as the game's model does.
    let mut layers: EntityLayers = [
        (
            "minecraft:arrow#main".to_owned(),
            layer([0.0, -1.0, -1.0], [16.0, 2.0, 2.0]),
        ),
        (
            "minecraft:minecart#main".to_owned(),
            layer([-8.0, -4.0, -8.0], [16.0, 8.0, 16.0]),
        ),
        (
            "minecraft:tropical_fish_large#main".to_owned(),
            layer([-1.0, 18.0, -3.0], [2.0, 6.0, 6.0]),
        ),
        (
            "minecraft:tropical_fish_large#pattern".to_owned(),
            layer([-1.0, 18.0, -3.0], [2.0, 6.0, 6.0]),
        ),
    ]
    .into();
    let looks: HashMap<&str, EntityLook> = [
        (
            "arrow",
            look(
                &["TippableArrowRenderer", "ArrowRenderer"],
                Some("entity/projectiles/arrow"),
            ),
        ),
        (
            "spectral_arrow",
            look(
                &["SpectralArrowRenderer", "ArrowRenderer"],
                Some("entity/projectiles/arrow_spectral"),
            ),
        ),
        ("falling_block", look(&["FallingBlockRenderer"], None)),
        ("painting", look(&["PaintingRenderer"], None)),
        (
            "experience_orb",
            EntityLook {
                texture_fields: vec![
                    "minecraft:textures/entity/experience/experience_orb.png".into(),
                ],
                ..look(&["ExperienceOrbRenderer"], None)
            },
        ),
        (
            "block_display",
            look(&["BlockDisplayRenderer", "DisplayRenderer"], None),
        ),
        (
            "text_display",
            look(&["TextDisplayRenderer", "DisplayRenderer"], None),
        ),
        (
            "tropical_fish",
            look(
                &[
                    "TropicalFishRenderer",
                    "MobRenderer",
                    "LivingEntityRenderer",
                ],
                None,
            ),
        ),
    ]
    .into_iter()
    .collect();
    for (id, look) in looks {
        layers.looks.insert(format!("minecraft:{id}"), look);
    }
    layers
}

fn entity(id: &str, at: (f64, f64, f64), yaw: f32, pitch: f32) -> Entity {
    let mut entity = Entity::new(format!("minecraft:{id}"), at);
    entity.nbt.insert(
        "Rotation".into(),
        NbtValue::List(vec![NbtValue::Float(yaw), NbtValue::Float(pitch)]),
    );
    entity
}

fn compound(entries: &[(&str, NbtValue)]) -> NbtValue {
    NbtValue::Compound(
        entries
            .iter()
            .map(|(key, value)| ((*key).to_owned(), value.clone()))
            .collect(),
    )
}

fn stone() -> NbtValue {
    compound(&[("Name", NbtValue::String("minecraft:stone".into()))])
}

fn meshed(entities: Vec<Entity>) -> EntityInstances {
    let mut schematic = UniversalSchematic::new("coverage".into());
    for entity in entities {
        schematic.add_entity(entity);
    }
    let pack = pack();
    let source = ResourcePackSource::from_baked(&pack, layers());
    schematic
        .to_mesh_with_entity_instances(&source, &MeshConfig::default())
        .unwrap()
        .1
}

/// Every vertex of instance `at`, where the picture puts it.
fn placed(entities: &EntityInstances, at: usize) -> Vec<[f32; 3]> {
    let instance = &entities.instances[at];
    let mesh = &entities.models[instance.model].mesh;
    [&mesh.opaque, &mesh.cutout, &mesh.transparent]
        .into_iter()
        .flat_map(|layer| layer.positions.iter())
        .map(|p| transform_point(&instance.transform, *p))
        .collect()
}

fn extent(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    points
        .iter()
        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(low, high), p| {
            (
                [0, 1, 2].map(|i| low[i].min(p[i])),
                [0, 1, 2].map(|i| high[i].max(p[i])),
            )
        })
}

/// Minecraft's `(x, y, z)` in the picture's space, whose block `n` spans
/// `n - 0.5 ..= n + 0.5`.
fn picture(at: [f32; 3]) -> [f32; 3] {
    at.map(|c| c - 0.5)
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
}

/// The point farthest from `from`.
fn farthest(points: &[[f32; 3]], from: [f32; 3]) -> [f32; 3] {
    let distance = |p: &[f32; 3]| (0..3).map(|i| (p[i] - from[i]).powi(2)).sum::<f32>();
    *points
        .iter()
        .max_by(|a, b| distance(a).total_cmp(&distance(b)))
        .unwrap()
}

#[test]
fn one_row_for_a_renderer_turns_each_arrow_by_its_yaw_and_pitch() {
    let at = (0.5, 1.0, 0.5);
    let entities = meshed(vec![
        entity("arrow", at, 0.0, 0.0),
        entity("arrow", at, 90.0, 0.0),
        entity("arrow", at, 0.0, 45.0),
        entity("spectral_arrow", at, 0.0, 0.0),
    ]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    assert_eq!(entities.instances.len(), 4);
    let origin = picture([0.5, 1.0, 0.5]);
    let tip = |index: usize| {
        let tip = farthest(&placed(&entities, index), origin);
        [0, 1, 2].map(|i| tip[i] - origin[i])
    };
    // The game's arrow yaw: 0 flies south (+Z), 90 east (+X); a positive
    // pitch climbs.
    let south = tip(0);
    assert!(south[2] > 0.9 && south[0].abs() < 0.1, "{south:?}");
    let east = tip(1);
    assert!(east[0] > 0.9 && east[2].abs() < 0.1, "{east:?}");
    let up = tip(2);
    assert!(up[1] > 0.6 && up[2] > 0.6, "{up:?}");
    // Both arrows share the renderer's row, each with its own texture.
    assert_eq!(entities.instances[0].model, entities.instances[1].model);
    assert_ne!(entities.instances[0].model, entities.instances[3].model);
}

#[test]
fn a_falling_block_and_a_minecart_carry_their_blocks() {
    let mut falling = entity("falling_block", (0.5, 3.0, 0.5), 0.0, 0.0);
    falling.nbt.insert("BlockState".into(), stone());
    let mut cart = entity("hopper_minecart", (4.5, 0.0, 0.5), 0.0, 0.0);
    cart.nbt.insert("DisplayState".into(), stone());
    cart.nbt.insert("DisplayOffset".into(), NbtValue::Int(6));
    // A plain minecart carries air; before 1.21 a saved block counts only
    // with `CustomDisplayTile`.
    let mut empty = entity("minecart", (8.5, 0.0, 0.5), 0.0, 0.0);
    empty.nbt.insert("DisplayState".into(), stone());
    empty
        .nbt
        .insert("CustomDisplayTile".into(), NbtValue::Byte(0));
    let entities = meshed(vec![falling, cart, empty]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    // FallingBlockRenderer: the block's unit cube, centred on the entity
    // across and standing on it.
    let (low, high) = extent(&placed(&entities, 0));
    assert!(
        close(low, picture([0.0, 3.0, 0.0])) && close(high, picture([1.0, 4.0, 1.0])),
        "{low:?} {high:?}"
    );
    // The cart with its stone inside: the stone's cube is its six faces
    // more than the cart that carries nothing.
    let corners = |at: usize| placed(&entities, at).len();
    assert_eq!(corners(1), corners(2) + 24, "the stone's six faces");
    // Three quarters of a block wide, centred in the cart.
    let cart = placed(&entities, 1);
    let empty: Vec<[f32; 3]> = placed(&entities, 2)
        .into_iter()
        .map(|p| [p[0] - 4.0, p[1], p[2]])
        .collect();
    let block: Vec<[f32; 3]> = cart
        .into_iter()
        .filter(|p| !empty.iter().any(|q| close(*p, *q)))
        .collect();
    let (low, high) = extent(&block);
    assert!(
        (high[0] - low[0] - 0.75).abs() < 1e-3 && (high[2] - low[2] - 0.75).abs() < 1e-3,
        "{low:?} {high:?}"
    );
    assert!(((low[0] + high[0]) / 2.0 - picture([4.5, 0.0, 0.0])[0]).abs() < 1e-3);
}

#[test]
fn a_painting_is_as_large_as_its_variant_and_hangs_on_its_wall() {
    // Two blocks high, facing south, hung in block (2, 1, 1): the game puts
    // its centre half a block up and 15/32 back toward the wall.
    let painting = |variant: &str, at: (f64, f64, f64)| {
        let mut painting = Entity::new("minecraft:painting".into(), at);
        painting
            .nbt
            .insert("variant".into(), NbtValue::String(variant.into()));
        painting.nbt.insert("facing".into(), NbtValue::Byte(0));
        painting
            .nbt
            .insert("block_pos".into(), NbtValue::IntArray(vec![2, 1, 1]));
        painting
    };
    let entities = meshed(vec![
        painting("minecraft:wanderer", (2.5, 2.0, 1.03125)),
        painting("minecraft:kebab", (2.5, 1.5, 1.03125)),
    ]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    let (low, high) = extent(&placed(&entities, 0));
    assert!(
        close(low, picture([2.0, 1.0, 1.0])) && close(high, picture([3.0, 3.0, 1.0625])),
        "the back on the wall, the picture 1/16 off it: {low:?} {high:?}"
    );
    let (low, high) = extent(&placed(&entities, 1));
    assert!(
        close(low, picture([2.0, 1.0, 1.0])) && close(high, picture([3.0, 2.0, 1.0625])),
        "{low:?} {high:?}"
    );
}

#[test]
fn sprites_keep_turned_to_the_camera() {
    let mut orb = entity("experience_orb", (0.5, 0.0, 0.5), 0.0, 0.0);
    orb.nbt.insert("Value".into(), NbtValue::Short(20));
    let entities = meshed(vec![orb]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    let billboard = entities.instances[0].billboard.expect("turned each frame");
    assert_eq!(billboard.kind, BillboardKind::Center);
    for back in [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 0.6, -0.8]] {
        let turned = billboard.turned(back);
        let facing = [turned[2][0], turned[2][1], turned[2][2]];
        assert!(close(facing, back), "{facing:?} toward {back:?}");
    }
}

#[test]
fn a_display_entity_takes_its_transformation() {
    let display = |transformation: NbtValue| {
        let mut display = entity("block_display", (0.0, 0.0, 0.0), 0.0, 0.0);
        display.nbt.insert("block_state".into(), stone());
        display.nbt.insert("transformation".into(), transformation);
        display
    };
    let floats =
        |values: &[f32]| NbtValue::List(values.iter().map(|v| NbtValue::Float(*v)).collect());
    let parts = compound(&[
        ("translation", floats(&[0.0, 1.0, 0.0])),
        ("left_rotation", floats(&[0.0, 0.0, 0.0, 1.0])),
        ("scale", floats(&[2.0, 2.0, 2.0])),
        ("right_rotation", floats(&[0.0, 0.0, 0.0, 1.0])),
    ]);
    let rows = floats(&[
        2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 1.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    let entities = meshed(vec![display(parts), display(rows)]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    for at in 0..2 {
        // The block's cube from the entity's corner, doubled and lifted.
        let (low, high) = extent(&placed(&entities, at));
        assert!(
            close(low, picture([0.0, 1.0, 0.0])) && close(high, picture([2.0, 3.0, 2.0])),
            "{at}: {low:?} {high:?}"
        );
    }
    assert!(entities.instances[0].billboard.is_none(), "fixed");
}

#[test]
fn a_tropical_fish_takes_its_two_dyes() {
    let mut fish = entity("tropical_fish", (0.5, 0.0, 0.5), 0.0, 0.0);
    // Large, its third pattern, a red body with blue markings.
    let packed: i32 = (11 << 24) | (14 << 16) | (2 << 8) | 1;
    fish.nbt.insert("Variant".into(), NbtValue::Int(packed));
    let entities = meshed(vec![fish]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    let mesh = &entities.models[entities.instances[0].model].mesh;
    let colors: Vec<[f32; 4]> = [&mesh.opaque, &mesh.cutout, &mesh.transparent]
        .into_iter()
        .flat_map(|layer| layer.colors.iter().copied())
        .collect();
    let has = |rgb: u32| {
        let want = [(rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff].map(|c| c as f32 / 255.0);
        colors
            .iter()
            .any(|c| (0..3).all(|i| (c[i] - want[i]).abs() < 0.02))
    };
    assert!(has(0xb02e26), "the red body");
    assert!(has(0x3c44aa), "the blue pattern");
}

#[test]
fn a_text_display_lays_out_its_text() {
    let mut text = entity("text_display", (0.5, 0.0, 0.5), 0.0, 0.0);
    text.nbt
        .insert("text".into(), NbtValue::String(r#""hi""#.into()));
    let entities = meshed(vec![text]);
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    // Two glyphs advancing six units: twelve wide, nine high, with the
    // background a unit round them, at 1/40 block a unit.
    let (low, high) = extent(&placed(&entities, 0));
    let size = [0, 1].map(|i| high[i] - low[i]);
    assert!(
        (size[0] - 13.0 * 0.025).abs() < 1e-3 && (size[1] - 10.0 * 0.025).abs() < 1e-3,
        "{size:?}"
    );
}

/// Every entity of every schematic in `LUMILIO_SCHEMATICS` (a folder) is
/// drawn, with the baked bundle's game version for the file's data version
/// and with the newest: what the studio would report undrawable, listed.
/// Reads `target/block-assets/` like the coverage check. Ignored: run with
/// `cargo test -p lumilio-schematic-render --lib schematics_in_a_folder --
/// --ignored` on a folder of real schematics.
#[test]
#[ignore = "needs a folder of schematics and the baked bundle"]
fn schematics_in_a_folder_draw_every_entity() {
    let folder = std::env::var("LUMILIO_SCHEMATICS").expect("LUMILIO_SCHEMATICS");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/block-assets");
    let blocks =
        lumilio_block_assets::Bundle::read(&std::fs::read(root.join("blocks.dat.z")).unwrap())
            .unwrap();
    let entities = lumilio_block_assets::EntityBundle::read(
        &std::fs::read(root.join("entities.dat.z")).unwrap(),
    )
    .unwrap();
    let index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("blocks.json")).unwrap()).unwrap();
    let versions: Vec<(i64, String)> = index["versions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            (
                v["data_version"].as_i64().unwrap(),
                v["id"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let mut report = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&folder)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "litematic"))
        .collect();
    files.sort();
    for path in files {
        let bytes = std::fs::read(&path).unwrap();
        let data_version = nucleation::formats::litematic::from_litematic(&bytes)
            .ok()
            .and_then(|schematic| schematic.metadata.mc_version)
            .unwrap_or(i32::MAX);
        let own = versions
            .iter()
            .rev()
            .find(|(v, _)| *v <= i64::from(data_version))
            .map_or(versions[0].1.clone(), |(_, id)| id.clone());
        let newest = versions.last().unwrap().1.clone();
        for version in [own, newest] {
            let pack = blocks.pack(&version).unwrap();
            let scene = crate::Scene::load(&bytes, &pack, entities.layers_of(&version)).unwrap();
            report.push(format!(
                "{} ({version}): {} entities, undrawable {:?}",
                path.file_name().unwrap().to_string_lossy(),
                scene.entity_count(),
                scene.undrawable_entities()
            ));
        }
    }
    let report = report.join("\n");
    eprintln!("{report}");
    assert!(!report.contains("undrawable [\""), "{report}");
}

/// A picture of one of each kind of entity T35 draws, on a stone floor,
/// with a client jar's textures: to look at, not to assert. Run with
/// `LUMILIO_PACK` (a client jar) and `LUMILIO_FRAME_PATH` (a PNG to write).
#[test]
#[ignore = "writes a picture to look at"]
fn entity_gallery() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/block-assets");
    let entities = lumilio_block_assets::EntityBundle::read(
        &std::fs::read(root.join("entities.dat.z")).unwrap(),
    )
    .unwrap();
    let pack = bake(&[AssetSource::at(std::env::var("LUMILIO_PACK").unwrap())]).unwrap();
    let version = pack.meta.game_version.clone().unwrap_or_default();
    let mut schematic = UniversalSchematic::new("gallery".into());
    schematic.fill_cuboid_str((0, 0, 0), (14, 0, 6), "minecraft:stone");
    schematic.fill_cuboid_str((0, 1, 6), (14, 3, 6), "minecraft:oak_planks");
    schematic.set_block_str(10, 1, 2, "minecraft:oak_fence");
    let mut cart = entity("hopper_minecart", (1.5, 1.0, 1.5), 0.0, 0.0);
    cart.nbt.insert(
        "Rotation".into(),
        NbtValue::List(vec![NbtValue::Float(0.0), NbtValue::Float(0.0)]),
    );
    let chest = entity("chest_minecart", (1.5, 1.0, 3.5), 90.0, 0.0);
    let mut falling = entity("falling_block", (3.5, 1.0, 1.5), 0.0, 0.0);
    falling.nbt.insert(
        "BlockState".into(),
        compound(&[("Name", NbtValue::String("minecraft:red_sand".into()))]),
    );
    let tnt = entity("tnt", (3.5, 1.0, 3.5), 0.0, 0.0);
    let arrow = entity("arrow", (5.5, 1.5, 1.5), 45.0, 20.0);
    let trident = entity("trident", (5.5, 1.5, 3.5), 0.0, -30.0);
    let crystal = entity("end_crystal", (7.5, 1.0, 2.5), 0.0, 0.0);
    let mut knot = Entity::new("minecraft:leash_knot".into(), (10.5, 1.375, 2.5));
    knot.nbt
        .insert("block_pos".into(), NbtValue::IntArray(vec![10, 1, 2]));
    let mut painting = Entity::new("minecraft:painting".into(), (12.0, 2.5, 5.96875));
    painting.nbt.insert(
        "variant".into(),
        NbtValue::String("minecraft:pigscene".into()),
    );
    painting.nbt.insert("facing".into(), NbtValue::Byte(2));
    painting
        .nbt
        .insert("block_pos".into(), NbtValue::IntArray(vec![11, 2, 5]));
    let mut fish = entity("tropical_fish", (12.5, 1.5, 2.5), 30.0, 0.0);
    fish.nbt.insert(
        "Variant".into(),
        NbtValue::Int((11 << 24) | (14 << 16) | (2 << 8) | 1),
    );
    let mut item = entity("item_display", (13.5, 1.5, 1.0), 0.0, 0.0);
    item.nbt.insert(
        "item".into(),
        compound(&[("id", NbtValue::String("minecraft:diamond_sword".into()))]),
    );
    let mut text = entity("text_display", (8.0, 3.0, 5.5), 180.0, 0.0);
    text.nbt
        .insert("text".into(), NbtValue::String(r#""Lumilio""#.into()));
    let mut orb = entity("experience_orb", (9.5, 1.2, 4.0), 0.0, 0.0);
    orb.nbt.insert("Value".into(), NbtValue::Short(40));
    let mut rabbit = entity("rabbit", (11.5, 1.0, 4.0), 200.0, 0.0);
    rabbit.nbt.insert("RabbitType".into(), NbtValue::Int(3));
    for entity in [
        cart, chest, falling, tnt, arrow, trident, crystal, knot, painting, fish, item, text, orb,
        rabbit,
    ] {
        schematic.add_entity(entity);
    }
    let mut scene =
        crate::Scene::from_schematic(schematic, &pack, entities.layers_of(&version)).unwrap();
    eprintln!("undrawable {:?}", scene.undrawable_entities());
    let view = crate::View::new().at(crate::Angle::North);
    let view = crate::View {
        pitch_deg: 28.0,
        yaw_deg: view.yaw_deg + 25.0,
        zoom: 1.7,
        ..view
    };
    let mut frame = scene.render(&view, 1800, 1000).unwrap();
    crate::scene::rgba_to_bgra(&mut frame.bgra);
    image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
        .unwrap()
        .save(std::env::var("LUMILIO_FRAME_PATH").unwrap())
        .unwrap();
}
