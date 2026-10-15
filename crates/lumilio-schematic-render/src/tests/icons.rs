//! Block icons (plan schematic-studio T27): flat items from their texture,
//! models drawn alone, one renderer per batch.

use std::io::Write;

use lumilio_block_assets::{AssetSource, BakedPack, EntityLayers, bake};

use crate::{Frame, SceneError, icons, item_icons};

fn png(rgba: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(16, 16, |x, _| {
        // The left half clear, as a torch's texture is mostly clear.
        if x < 8 {
            image::Rgba([0, 0, 0, 0])
        } else {
            image::Rgba(rgba)
        }
    }))
    .write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    bytes
}

fn full_png(rgba: [u8; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(16, 16, image::Rgba(rgba)))
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

/// Red stone and blue dirt as full cubes with their block items, and a
/// wall torch whose item is the flat `torch`.
pub(super) fn pack() -> BakedPack {
    let cube = |block: &str| {
        format!(
            r##"{{"textures":{{"all":"minecraft:block/{block}"}},"elements":[{{"from":[0,0,0],"to":[16,16,16],"faces":{{"down":{{"texture":"#all"}},"up":{{"texture":"#all"}},"north":{{"texture":"#all"}},"south":{{"texture":"#all"}},"west":{{"texture":"#all"}},"east":{{"texture":"#all"}}}}}}]}}"##
        )
    };
    let state =
        |model: &str| format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{model}"}}}}}}"#);
    let files: Vec<(String, Vec<u8>)> = vec![
        ("assets/minecraft/blockstates/stone.json".into(), state("stone").into_bytes()),
        ("assets/minecraft/blockstates/dirt.json".into(), state("dirt").into_bytes()),
        ("assets/minecraft/models/block/stone.json".into(), cube("stone").into_bytes()),
        ("assets/minecraft/models/block/dirt.json".into(), cube("dirt").into_bytes()),
        // A lower half block, for outlines that follow a block's shape.
        ("assets/minecraft/blockstates/dirt_slab.json".into(), state("dirt_slab").into_bytes()),
        (
            "assets/minecraft/models/block/dirt_slab.json".into(),
            cube("dirt").replace(r#""to":[16,16,16]"#, r#""to":[16,8,16]"#).into_bytes(),
        ),
        (
            "assets/minecraft/models/item/stone.json".into(),
            br#"{"parent":"minecraft:block/stone"}"#.to_vec(),
        ),
        (
            "assets/minecraft/models/item/generated.json".into(),
            br#"{"parent":"builtin/generated"}"#.to_vec(),
        ),
        (
            "assets/minecraft/models/item/torch.json".into(),
            br#"{"parent":"minecraft:item/generated","textures":{"layer0":"minecraft:block/torch"}}"#
                .to_vec(),
        ),
        ("assets/minecraft/textures/block/stone.png".into(), full_png([255, 0, 0, 255])),
        ("assets/minecraft/textures/block/dirt.png".into(), full_png([0, 0, 255, 255])),
        ("assets/minecraft/textures/block/torch.png".into(), png([255, 200, 0, 255])),
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

/// BGRA pixels that are mostly `channel` (0 blue, 2 red) and opaque.
pub(super) fn count(frame: &Frame, channel: usize) -> usize {
    frame
        .bgra
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[3] > 0 && px[channel] > 128 && px[2 - channel] < 64)
        .count()
}

#[test]
fn a_flat_item_is_its_texture_without_a_renderer() {
    let states = ["minecraft:wall_torch[facing=east]".to_owned()];
    let icons = icons(&pack(), EntityLayers::new(), &states, 32).unwrap();
    let frame = icons[0].as_ref().expect("the torch item is flat");
    assert_eq!((frame.width, frame.height), (32, 32));
    // The texture's left half is clear, its right half amber: scaled ×2
    // without smoothing.
    let pixel = |x: u32, y: u32| {
        let at = ((y * 32 + x) * 4) as usize;
        <[u8; 4]>::try_from(&frame.bgra[at..at + 4]).unwrap()
    };
    assert_eq!(pixel(15, 5), [0, 0, 0, 0]);
    assert_eq!(pixel(16, 5), [0, 200, 255, 255]);
}

#[test]
fn a_batch_of_icons_draws_each_block_alone() {
    let states = [
        "minecraft:stone".to_owned(),
        "minecraft:air".to_owned(),
        "minecraft:dirt".to_owned(),
    ];
    let icons = match icons(&pack(), EntityLayers::new(), &states, 48) {
        Ok(icons) => icons,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    assert!(icons[1].is_none(), "air draws nothing");
    let (stone, dirt) = (icons[0].as_ref().unwrap(), icons[2].as_ref().unwrap());
    assert!(count(stone, 2) > 200, "the stone icon is red");
    assert_eq!(count(stone, 0), 0, "and has no dirt in it");
    assert!(count(dirt, 0) > 200, "the dirt icon is blue");
    assert_eq!(count(dirt, 2), 0, "and has no stone in it");
    // The corners stay clear: the block is framed inside the icon.
    assert_eq!(stone.bgra[3], 0);
    assert_eq!(stone.bgra[stone.bgra.len() - 1], 0);
}

/// T32: a slot's item is its flat texture or its block, and an item the pack
/// has neither for has no icon at all, never the missing-texture block.
#[test]
fn an_item_is_its_texture_or_its_block_and_nothing_else() {
    let items = [
        "minecraft:torch".to_owned(),
        "minecraft:stone".to_owned(),
        "minecraft:shield".to_owned(),
    ];
    let icons = match item_icons(&pack(), EntityLayers::new(), &items, 32) {
        Ok(icons) => icons,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(error) => panic!("{error}"),
    };
    let torch = icons[0].as_ref().expect("the torch is flat");
    assert_eq!(
        &torch.bgra[(5 * 32 + 16) * 4..(5 * 32 + 16) * 4 + 4],
        [0, 200, 255, 255]
    );
    assert!(count(icons[1].as_ref().expect("stone is its block"), 2) > 100);
    assert!(icons[2].is_none(), "nothing to draw a shield from");
}
