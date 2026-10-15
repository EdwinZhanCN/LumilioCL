mod bake;
mod block_items;
mod container;
mod names;
mod over;

use std::io::{Cursor, Write};

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use crate::{AssetSource, BakedPack};

/// An archive with `files` in it.
fn archive(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A 2×2 texture of `seed`'s colours, PNG-encoded with `compression`: two
/// versions that encode the same pixels differently still share an image.
fn png(seed: u8, compression: CompressionType) -> Vec<u8> {
    let pixels: Vec<u8> = (0..16).map(|i| seed.wrapping_mul(i + 1)).collect();
    let mut bytes = Vec::new();
    PngEncoder::new_with_quality(&mut bytes, compression, FilterType::Adaptive)
        .write_image(&pixels, 2, 2, ExtendedColorType::Rgba8)
        .unwrap();
    bytes
}

fn json(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

/// A lamp post the way 1.13.2 writes it.
fn jar_1_13_2() -> Vec<u8> {
    let fast = CompressionType::Fast;
    archive(&[
        (
            "version.json",
            json(r#"{"id":"1.13.2","world_version":1631}"#),
        ),
        (
            "net/minecraft/client/Main.class",
            b"\xca\xfe\xba\xbe".to_vec(),
        ),
        (
            "assets/minecraft/blockstates/lamp.json",
            json(
                r#"{"variants":{
                    "facing=north,lit=false":{"model":"block/lamp"},
                    "facing=east,lit=false":{"model":"block/lamp","y":90,"uvlock":true}}}"#,
            ),
        ),
        (
            "assets/minecraft/blockstates/lamp_wire.json",
            json(
                r#"{"multipart":[
                    {"apply":{"model":"block/lamp"}},
                    {"when":{"power":"0"},"apply":{"model":"minecraft:block/lamp","x":180}},
                    {"when":{"OR":[{"lit":"true"},{"north":"side|up"}]},
                     "apply":[{"model":"block/lamp","weight":2}]}]}"#,
            ),
        ),
        (
            "assets/minecraft/models/block/lamp_base.json",
            json(
                r##"{"ambientocclusion":false,"textures":{"particle":"#side"},
                    "elements":[{"from":[0,0,0],"to":[16,8,16],
                      "rotation":{"origin":[8,8,8],"axis":"y","angle":22.5},
                      "faces":{"down":{"texture":"#side","cullface":"down"},
                               "up":{"texture":"#top","uv":[0,0,16,16],"tintindex":0}}}]}"##,
            ),
        ),
        (
            "assets/minecraft/models/block/lamp.json",
            json(
                r#"{"parent":"block/lamp_base",
                    "textures":{"side":"block/lamp_side","top":"block/lamp_top"}}"#,
            ),
        ),
        (
            "assets/minecraft/textures/block/lamp_side.png",
            png(3, fast),
        ),
        ("assets/minecraft/textures/block/lamp_top.png", png(5, fast)),
        (
            "assets/minecraft/textures/block/lamp_top.png.mcmeta",
            json(r#"{"animation":{"frametime":2,"frames":[0,{"index":1,"time":4}]}}"#),
        ),
        (
            "assets/minecraft/textures/entity/banner_base.png",
            png(7, fast),
        ),
        (
            "assets/minecraft/textures/entity/pig/pig.png",
            png(11, fast),
        ),
        ("assets/minecraft/textures/gui/widgets.png", png(13, fast)),
    ])
}

/// The same lamp post the way 26.3 writes it.
fn jar_26_3() -> Vec<u8> {
    let best = CompressionType::Best;
    archive(&[
        (
            "version.json",
            json(r#"{"id":"26.3","world_version":4790}"#),
        ),
        (
            "assets/minecraft/blockstates/lamp.json",
            json(
                r#"{"variants":{
                    "lit=false,facing=north":[{"model":"minecraft:block/lamp"}],
                    "lit=false,facing=east":{"model":"minecraft:block/lamp","y":90,"uvlock":true}}}"#,
            ),
        ),
        (
            "assets/minecraft/blockstates/lamp_wire.json",
            json(
                r#"{"multipart":[
                    {"apply":[{"model":"minecraft:block/lamp"}]},
                    {"when":{"power":0},"apply":{"model":"minecraft:block/lamp","x":180}},
                    {"when":{"OR":[{"lit":true},{"north":"side|up"}]},
                     "apply":{"model":"minecraft:block/lamp","weight":2}}]}"#,
            ),
        ),
        (
            "assets/minecraft/models/block/lamp_base.json",
            json(
                r##"{"ambientocclusion":false,"textures":{"particle":"#side"},
                    "elements":[{"from":[0,0,0],"to":[16,8,16],
                      "rotation":{"origin":[8,8,8],"y":22.5},
                      "faces":{"down":{"texture":"#side","cullface":"bottom"},
                               "up":{"texture":"#top","uv":[0,0,16,16],"tintindex":0}}}]}"##,
            ),
        ),
        (
            "assets/minecraft/models/block/lamp.json",
            json(
                r#"{"parent":"minecraft:block/lamp_base",
                    "textures":{"side":{"sprite":"minecraft:block/lamp_side","force_translucent":true},
                                "top":"minecraft:block/lamp_top"}}"#,
            ),
        ),
        (
            "assets/minecraft/textures/block/lamp_side.png",
            png(3, best),
        ),
        ("assets/minecraft/textures/block/lamp_top.png", png(5, best)),
        (
            "assets/minecraft/textures/block/lamp_top.png.mcmeta",
            json(r#"{"animation":{"frames":[0,{"index":1,"time":4}],"frametime":2}}"#),
        ),
        (
            "assets/minecraft/textures/entity/banner/banner_base.png",
            png(7, best),
        ),
        (
            "assets/minecraft/textures/entity/pig/pig_temperate.png",
            png(11, best),
        ),
        ("assets/minecraft/items/lamp.json", json(r#"{"model":{}}"#)),
    ])
}

fn baked(jar: Vec<u8>) -> BakedPack {
    crate::bake(&[AssetSource::ArchiveBytes(jar)]).unwrap()
}
