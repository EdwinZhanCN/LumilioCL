use std::fs;

use image::ImageEncoder;
use image::codecs::png::CompressionType;

use super::{archive, baked, jar_1_13_2, jar_26_3, json, png};
use crate::{AssetSource, BakedBlockstate, Condition, Direction, bake};

/// The point of baking: each game version's spelling of the same block —
/// `sprite` texture objects, `bottom` cull faces, numeric and boolean
/// multipart values, the single-axis or Euler rotation, and moved entity
/// textures — comes out as one pack. Only where it came from differs.
#[test]
fn spellings_of_1_13_2_and_26_3_bake_alike() {
    let old = baked(jar_1_13_2());
    let new = baked(jar_26_3());

    assert_eq!(old.assets, new.assets);
    assert_ne!(old.meta, new.meta);
    assert_eq!(old.meta.data_version, Some(1631));
    assert_eq!(old.meta.game_version.as_deref(), Some("1.13.2"));
    assert!(old.meta.skipped.is_empty(), "{:?}", old.meta.skipped);

    let assets = &new.assets;
    let BakedBlockstate::Variants(variants) = &assets.blockstates["minecraft:lamp"] else {
        panic!("variants");
    };
    assert_eq!(
        variants.keys().collect::<Vec<_>>(),
        ["facing=east,lit=false", "facing=north,lit=false"]
    );
    let BakedBlockstate::Multipart(cases) = &assets.blockstates["minecraft:lamp_wire"] else {
        panic!("multipart");
    };
    assert_eq!(
        cases[1].when,
        Some(Condition::Properties([("power".into(), "0".into())].into()))
    );
    assert!(matches!(&cases[2].when, Some(Condition::Any(any)) if any.len() == 2));

    let lamp = &assets.models["minecraft:block/lamp"];
    assert!(!lamp.ambient_occlusion, "inherited from the parent");
    assert_eq!(lamp.textures["particle"], "minecraft:block/lamp_side");
    let element = &lamp.elements[0];
    assert_eq!(element.rotation.as_ref().unwrap().angles, [0.0, 22.5, 0.0]);
    let down = &element.faces[&Direction::Down];
    assert_eq!(down.cullface, Some(Direction::Down));
    assert_eq!(down.texture.as_deref(), Some("minecraft:block/lamp_side"));
    assert_eq!(element.faces[&Direction::Up].tintindex, Some(0));

    assert_eq!(
        assets.textures.keys().collect::<Vec<_>>(),
        [
            "minecraft:block/lamp_side",
            "minecraft:block/lamp_top",
            "minecraft:entity/banner/banner_base",
            "minecraft:entity/pig/pig_temperate",
        ]
    );
    assert_eq!(assets.images.len(), 4);
    let animation = assets.textures["minecraft:block/lamp_top"]
        .animation
        .as_ref()
        .unwrap();
    assert_eq!(animation.frames, Some(vec![(0, None), (1, Some(4))]));
}

/// A resource pack replaces files before anything is resolved: a changed
/// parent model reaches the child, a changed texture replaces the pixels.
#[test]
fn later_sources_override_earlier_files() {
    let directory = tempfile::tempdir().unwrap();
    let models = directory.path().join("assets/minecraft/models/block");
    fs::create_dir_all(&models).unwrap();
    fs::write(
        models.join("lamp_base.json"),
        r##"{"elements":[{"from":[0,0,0],"to":[16,16,16],
            "faces":{"north":{"texture":"#side"}}}]}"##,
    )
    .unwrap();
    let texture_pack = archive(&[(
        "assets/minecraft/textures/block/lamp_side.png",
        png(99, CompressionType::Fast),
    )]);

    let base = baked(jar_26_3());
    let layered = bake(&[
        AssetSource::ArchiveBytes(jar_26_3()),
        AssetSource::ArchiveBytes(texture_pack),
        AssetSource::at(directory.path()),
    ])
    .unwrap();

    let lamp = &layered.assets.models["minecraft:block/lamp"];
    assert!(
        lamp.ambient_occlusion,
        "the replaced parent no longer turns it off"
    );
    assert_eq!(lamp.elements[0].to, [16.0, 16.0, 16.0]);
    assert_ne!(
        layered.assets.texture_image("minecraft:block/lamp_side"),
        base.assets.texture_image("minecraft:block/lamp_side")
    );
    assert_eq!(
        layered.assets.texture_image("minecraft:block/lamp_top"),
        base.assets.texture_image("minecraft:block/lamp_top")
    );
    assert_eq!(layered.meta.sources.len(), 3);
    assert_eq!(layered.meta.sources[2].sha1, None);
}

/// A jar holding both spellings of a moved texture keeps the new file.
#[test]
fn an_old_texture_name_gives_way_to_the_new_one() {
    let pack = baked(archive(&[
        (
            "assets/minecraft/textures/entity/pig/pig.png",
            png(1, CompressionType::Fast),
        ),
        (
            "assets/minecraft/textures/entity/pig/pig_temperate.png",
            png(2, CompressionType::Fast),
        ),
    ]));
    let only = baked(archive(&[(
        "assets/minecraft/textures/entity/pig/pig_temperate.png",
        png(2, CompressionType::Fast),
    )]));
    assert_eq!(pack.assets, only.assets);
}

/// One broken file does not lose a version's other blocks; it is named.
#[test]
fn unreadable_entries_are_skipped_and_named() {
    let pack = baked(archive(&[
        (
            "assets/minecraft/blockstates/broken.json",
            json("{not json"),
        ),
        (
            "assets/minecraft/blockstates/orphan.json",
            json(r#"{"variants":{"":{"model":"block/nowhere"}}}"#),
        ),
        (
            "assets/minecraft/blockstates/looped.json",
            json(r#"{"variants":{"":{"model":"block/a"}}}"#),
        ),
        (
            "assets/minecraft/models/block/a.json",
            json(r#"{"parent":"block/b"}"#),
        ),
        (
            "assets/minecraft/models/block/b.json",
            json(r#"{"parent":"block/a"}"#),
        ),
        (
            "assets/minecraft/textures/block/bad.png",
            b"not a png".to_vec(),
        ),
    ]));
    let skipped = pack.meta.skipped.join("\n");
    for name in [
        "minecraft:broken",
        "minecraft:block/nowhere",
        "minecraft:block/a",
        "minecraft:block/bad",
    ] {
        assert!(skipped.contains(name), "{name} in {skipped}");
    }
    assert!(pack.assets.blockstates.contains_key("minecraft:orphan"));
}

#[test]
fn a_file_that_is_not_an_archive_is_a_readable_error() {
    let error = bake(&[AssetSource::ArchiveBytes(b"plain text".to_vec())]).unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("not a readable resource archive"),
        "{error}"
    );
}

/// Each colormap is sampled where the game reads plains (temperature 0.8,
/// downfall 0.4 → column 50, row 173), and the samples survive a write.
#[test]
fn colormaps_are_sampled_where_plains_reads_them() {
    let map = |hit: [u8; 3]| {
        let mut pixels = vec![0u8; 256 * 256 * 4];
        let at = (173 * 256 + 50) * 4;
        pixels[at..at + 4].copy_from_slice(&[hit[0], hit[1], hit[2], 255]);
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&pixels, 256, 256, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    };
    let pack = baked(archive(&[
        (
            "assets/minecraft/textures/colormap/grass.png",
            map([1, 2, 3]),
        ),
        (
            "assets/minecraft/textures/colormap/foliage.png",
            map([4, 5, 6]),
        ),
        (
            "assets/minecraft/textures/colormap/odd.png",
            png(3, CompressionType::Fast),
        ),
    ]));
    assert_eq!(
        pack.assets.tint_samples,
        [("foliage".into(), [4, 5, 6]), ("grass".into(), [1, 2, 3])].into(),
        "a map that is not 256×256 has no sample"
    );
    let bytes = pack.to_bytes().unwrap();
    let back = crate::BakedPack::from_bytes(&bytes).unwrap();
    assert_eq!(back, pack);
    assert_eq!(back.to_bytes().unwrap(), bytes);
}

/// Item models are kept though no blockstate names them, with their display
/// transforms inherited per context and their layer textures.
#[test]
fn item_models_keep_their_display_transforms() {
    let pack = baked(archive(&[
        (
            "assets/minecraft/models/item/generated.json",
            json(
                r#"{"display":{"fixed":{"rotation":[0,180,0],"scale":[0.5,0.5,0.5]},
                              "ground":{"translation":[0,2,0]}}}"#,
            ),
        ),
        (
            "assets/minecraft/models/item/apple.json",
            json(
                r#"{"parent":"minecraft:item/generated","textures":{"layer0":"minecraft:item/apple"},
                    "display":{"ground":{"scale":[0.25,0.25,0.25]}}}"#,
            ),
        ),
        (
            "assets/minecraft/textures/item/apple.png",
            png(3, CompressionType::Fast),
        ),
    ]));
    let apple = &pack.assets.models["minecraft:item/apple"];
    assert_eq!(apple.textures["layer0"], "minecraft:item/apple");
    assert_eq!(apple.display["fixed"].rotation, [0.0, 180.0, 0.0]);
    assert_eq!(apple.display["fixed"].scale, [0.5; 3]);
    assert_eq!(apple.display["ground"].scale, [0.25; 3], "the child wins");
    assert_eq!(apple.display["ground"].translation, [0.0; 3]);
    assert!(pack.assets.textures.contains_key("minecraft:item/apple"));
}

/// A `width`×`height` PNG of one colour.
fn sized_png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &vec![200; (width * height * 4) as usize],
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    bytes
}

/// T35: paintings take their size and picture from the game's
/// `painting_variant` data; before 1.21 there is none, and each vanilla
/// picture's own size at 16 pixels a block says it (the back is no
/// painting). A mod adds its variants; a resource pack's larger pictures
/// change no size.
#[test]
fn paintings_take_their_size_from_the_data_or_the_vanilla_picture() {
    let painting = |name: &str, width: u32, height: u32| {
        (
            format!("assets/minecraft/textures/painting/{name}.png"),
            sized_png(width, height),
        )
    };
    let files = |entries: Vec<(String, Vec<u8>)>| {
        archive(
            &entries
                .iter()
                .map(|(name, bytes)| (name.as_str(), bytes.clone()))
                .collect::<Vec<_>>(),
        )
    };
    let size = |pack: &crate::BakedPack, id: &str| {
        pack.assets
            .paintings
            .get(id)
            .map(|p| (p.width, p.height, p.texture.clone()))
    };

    let with_data = baked(files(vec![
        painting("kebab", 16, 16),
        (
            "data/minecraft/painting_variant/kebab.json".into(),
            json(r#"{"asset_id":"minecraft:kebab","width":1,"height":1}"#),
        ),
        (
            "data/minecraft/painting_variant/wide.json".into(),
            json(r#"{"asset_id":"minecraft:wide_art","width":2,"height":1}"#),
        ),
    ]));
    assert_eq!(
        size(&with_data, "minecraft:kebab"),
        Some((1, 1, "minecraft:painting/kebab".into()))
    );
    assert_eq!(
        size(&with_data, "minecraft:wide"),
        Some((2, 1, "minecraft:painting/wide_art".into())),
        "the data names the picture"
    );

    let base = baked(files(vec![
        painting("kebab", 16, 16),
        painting("wanderer", 16, 32),
        painting("back", 16, 16),
    ]));
    assert_eq!(
        size(&base, "minecraft:wanderer").map(|s| (s.0, s.1)),
        Some((1, 2))
    );
    assert_eq!(
        size(&base, "minecraft:kebab").map(|s| (s.0, s.1)),
        Some((1, 1))
    );
    assert!(
        size(&base, "minecraft:back").is_none(),
        "the back is no painting"
    );

    let resource_pack = crate::bake_over(
        &base,
        &[AssetSource::ArchiveBytes(files(vec![painting(
            "kebab", 32, 32,
        )]))],
    )
    .unwrap();
    assert!(
        resource_pack.assets.paintings.is_empty(),
        "a sharper picture is no new size"
    );
    let a_mod = crate::bake_over(
        &base,
        &[AssetSource::ArchiveBytes(files(vec![(
            "data/gallery/painting_variant/sunset.json".into(),
            json(r#"{"asset_id":"gallery:sunset","width":4,"height":3}"#),
        )]))],
    )
    .unwrap();
    assert_eq!(
        size(&a_mod, "gallery:sunset"),
        Some((4, 3, "gallery:painting/sunset".into()))
    );
}

/// T35: 26.x renamed the rabbit textures (`rabbit/brown` → `rabbit/
/// rabbit_brown`); an older jar's are baked under the new names, so one
/// row draws every version.
#[test]
fn old_rabbit_textures_take_their_new_names() {
    let pack = baked(archive(&[(
        "assets/minecraft/textures/entity/rabbit/brown.png",
        png(3, CompressionType::Fast),
    )]));
    assert!(
        pack.assets
            .textures
            .contains_key("minecraft:entity/rabbit/rabbit_brown")
    );
    assert!(
        !pack
            .assets
            .textures
            .contains_key("minecraft:entity/rabbit/brown")
    );
}
