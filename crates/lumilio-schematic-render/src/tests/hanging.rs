//! Entities hung on a block (plan schematic-studio T33, D32): item frames as
//! the game saves them, in each of the six facings, lie against the block
//! they hang on, and the item in them sits on the frame, turned by its
//! `ItemRotation`. Meshed only: no GPU.

use std::collections::HashMap;
use std::io::Write;

use lumilio_block_assets::{AssetSource, BakedPack, EntityLayers, bake};
use nucleation::meshing::{MeshConfig, ResourcePackSource};
use nucleation::{Entity, NbtValue, UniversalSchematic};

const FACINGS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const EPS: f32 = 1e-3;

fn png(image: &image::RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image.clone())
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

/// The frame as the game's template model lays it out (a back panel and four
/// sticks on the +z side of the cell), and a flat item whose texture is
/// clear but for a blob in its top right quarter.
fn frame_pack() -> BakedPack {
    let face = |texture: &str| format!(r##"{{"texture":"#{texture}"}}"##);
    let element = |from: [f32; 3], to: [f32; 3], texture: &str| {
        let faces: Vec<String> = ["down", "up", "north", "south", "west", "east"]
            .iter()
            .map(|side| format!(r#""{side}":{}"#, face(texture)))
            .collect();
        format!(
            r#"{{"from":{from:?},"to":{to:?},"faces":{{{}}}}}"#,
            faces.join(",")
        )
    };
    let elements = [
        element([3., 3., 15.5], [13., 13., 16.], "back"),
        element([2., 2., 15.], [14., 3., 16.], "wood"),
        element([2., 13., 15.], [14., 14., 16.], "wood"),
        element([2., 3., 15.], [3., 13., 16.], "wood"),
        element([13., 3., 15.], [14., 13., 16.], "wood"),
    ];
    let frame = format!(
        r#"{{"textures":{{"wood":"minecraft:block/birch_planks","back":"minecraft:block/item_frame"}},"elements":[{}]}}"#,
        elements.join(",")
    );
    let solid = png(&image::RgbaImage::from_pixel(
        16,
        16,
        image::Rgba([200, 160, 90, 255]),
    ));
    let blob = png(&image::RgbaImage::from_fn(16, 16, |x, y| {
        if (10..14).contains(&x) && (2..6).contains(&y) {
            image::Rgba([255, 220, 0, 255])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    }));
    let files: Vec<(&str, Vec<u8>)> = vec![
        (
            "assets/minecraft/blockstates/item_frame.json",
            br#"{"variants":{"map=false":{"model":"minecraft:block/item_frame"},"map=true":{"model":"minecraft:block/item_frame"}}}"#.to_vec(),
        ),
        ("assets/minecraft/models/block/item_frame.json", frame.into_bytes()),
        (
            "assets/minecraft/models/item/generated.json",
            br#"{"parent":"builtin/generated","display":{"fixed":{"rotation":[0,180,0],"scale":[1,1,1]}}}"#.to_vec(),
        ),
        (
            "assets/minecraft/models/item/gold_ingot.json",
            br#"{"parent":"minecraft:item/generated","textures":{"layer0":"minecraft:item/gold_ingot"}}"#.to_vec(),
        ),
        ("assets/minecraft/textures/block/birch_planks.png", solid.clone()),
        ("assets/minecraft/textures/block/item_frame.png", solid),
        ("assets/minecraft/textures/item/gold_ingot.png", blob),
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

fn normal(facing: &str) -> [f64; 3] {
    match facing {
        "down" => [0., -1., 0.],
        "up" => [0., 1., 0.],
        "north" => [0., 0., -1.],
        "south" => [0., 0., 1.],
        "west" => [-1., 0., 0.],
        _ => [1., 0., 0.],
    }
}

/// An item frame hung in block (0, 0, 0), saved as the game saves one: its
/// centre 15/32 of a block back from the block's centre against its facing,
/// `Facing` by the six-way id, the block as `TileX`/`TileY`/`TileZ`, and the
/// yaw and pitch its facing gives.
fn frame(facing: &str, item: Option<i8>, invisible: bool) -> Entity {
    let n = normal(facing);
    let position = (
        0.5 - n[0] * 15. / 32.,
        0.5 - n[1] * 15. / 32.,
        0.5 - n[2] * 15. / 32.,
    );
    let mut entity = Entity::new("minecraft:item_frame".to_owned(), position);
    let id = FACINGS.iter().position(|f| *f == facing).unwrap() as i8;
    let (yaw, pitch) = match facing {
        "up" => (0., -90.),
        "down" => (0., 90.),
        "west" => (90., 0.),
        "north" => (180., 0.),
        "east" => (270., 0.),
        _ => (0., 0.),
    };
    let nbt = &mut entity.nbt;
    nbt.insert("Facing".into(), NbtValue::Byte(id));
    for key in ["TileX", "TileY", "TileZ"] {
        nbt.insert(key.into(), NbtValue::Int(0));
    }
    nbt.insert(
        "Rotation".into(),
        NbtValue::List(vec![NbtValue::Float(yaw), NbtValue::Float(pitch)]),
    );
    nbt.insert("Invisible".into(), NbtValue::Byte(i8::from(invisible)));
    if let Some(turn) = item {
        let mut stack = HashMap::new();
        stack.insert(
            "id".to_owned(),
            NbtValue::String("minecraft:gold_ingot".into()),
        );
        stack.insert("count".to_owned(), NbtValue::Int(1));
        nbt.insert("Item".into(), NbtValue::Compound(stack));
        nbt.insert("ItemRotation".into(), NbtValue::Byte(turn));
    }
    entity
}

/// Every vertex of the meshed entity where it is drawn (positions and
/// normals), in the mesher's space: block 0 spans −0.5..0.5.
fn drawn(pack: &BakedPack, entity: Entity) -> Vec<([f32; 3], [f32; 3])> {
    let mut schematic = UniversalSchematic::new("frame".into());
    schematic.add_entity(entity);
    let source = ResourcePackSource::from_baked(pack, EntityLayers::new());
    let (_, entities) = schematic
        .to_mesh_with_entity_instances(&source, &MeshConfig::default())
        .unwrap();
    assert!(entities.undrawable.is_empty(), "{:?}", entities.undrawable);
    let mut points = Vec::new();
    for instance in &entities.instances {
        let mesh = &entities.models[instance.model].mesh;
        for layer in [&mesh.opaque, &mesh.cutout, &mesh.transparent] {
            for (position, normal) in layer.positions.iter().zip(&layer.normals) {
                let world = nucleation::meshing::transform_point(&instance.transform, *position);
                points.push((world, *normal));
            }
        }
    }
    points
}

fn axis(facing: &str) -> (usize, f32) {
    let n = normal(facing);
    let axis = (0..3).find(|&a| n[a] != 0.).unwrap();
    (axis, n[axis] as f32)
}

/// The face of block 0 the entity hangs against, along its facing's axis.
fn wall(facing: &str) -> f32 {
    -0.5 * axis(facing).1
}

#[test]
fn a_frame_lies_against_the_block_it_hangs_on_in_every_facing() {
    let pack = frame_pack();
    for facing in FACINGS {
        let points = drawn(&pack, frame(facing, None, false));
        assert!(!points.is_empty(), "{facing}: the frame is drawn");
        let (along, sign) = axis(facing);
        let wall = wall(facing);
        for (point, _) in &points {
            // From the wall into the cell, no deeper than the frame's 1/16.
            let depth = (point[along] - wall) * sign;
            assert!(
                (-EPS..=1. / 16. + EPS).contains(&depth),
                "{facing}: {point:?} is {depth} from the wall"
            );
            for other in (0..3).filter(|&a| a != along) {
                assert!(
                    point[other].abs() <= 6. / 16. + EPS,
                    "{facing}: {point:?} within the frame's 12/16 square"
                );
            }
        }
    }
}

#[test]
fn an_item_lies_on_its_frame_and_an_invisible_frame_shows_only_the_item() {
    let pack = frame_pack();
    for facing in FACINGS {
        let (along, sign) = axis(facing);
        let wall = wall(facing);
        // An invisible frame: the item alone, flat on the wall.
        let item = drawn(&pack, frame(facing, Some(0), true));
        assert!(!item.is_empty(), "{facing}: the item is drawn");
        for (point, _) in &item {
            let depth = (point[along] - wall) * sign;
            assert!(
                depth.abs() <= 1. / 32. + EPS,
                "{facing}: {point:?} at {depth}"
            );
        }
        // In a frame, in front of its back panel (1/32 from the wall).
        let framed = drawn(&pack, frame(facing, Some(0), false));
        assert!(
            framed.len() > item.len(),
            "{facing}: the frame and the item"
        );
        let deepest = framed
            .iter()
            .map(|(point, _)| (point[along] - wall) * sign)
            .fold(0f32, f32::max);
        assert!(
            deepest > 1. / 16. + EPS,
            "{facing}: the item stands in front of the frame ({deepest})"
        );
    }
}

/// The item's outline (its edge faces, which follow the opaque pixels) as
/// seen from in front of a south-facing frame: the viewer's right is +x.
fn blob_centre(pack: &BakedPack, turn: i8) -> [f32; 2] {
    let points: Vec<[f32; 3]> = drawn(pack, frame("south", Some(turn), true))
        .into_iter()
        .filter(|(_, normal)| normal[2].abs() < 0.5)
        .map(|(point, _)| point)
        .collect();
    assert!(!points.is_empty(), "the blob has edges");
    let n = points.len() as f32;
    [
        points.iter().map(|p| p[0]).sum::<f32>() / n,
        points.iter().map(|p| p[1]).sum::<f32>() / n,
    ]
}

#[test]
fn an_item_turns_clockwise_by_its_rotation_and_is_not_mirrored() {
    let pack = frame_pack();
    // Unturned, the texture's top right is the viewer's top right.
    let [x, y] = blob_centre(&pack, 0);
    assert!(x > 0.05 && y > 0.05, "top right: {x}, {y}");
    // Two eighths clockwise: bottom right.
    let [x, y] = blob_centre(&pack, 2);
    assert!(x > 0.05 && y < -0.05, "bottom right: {x}, {y}");
}
