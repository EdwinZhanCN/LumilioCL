use super::*;
use std::io::Write;

fn client(path: &Path, entries: &[(&str, u32, [u8; 4])]) {
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, height, color) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        let image = image::RgbaImage::from_pixel(64, *height, image::Rgba(*color));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        zip.write_all(png.get_ref()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn uuid_uses_java_hash_and_selects_both_modern_arm_models() {
    let root = tempfile::tempdir().unwrap();
    let jar = root.path().join("client.jar");
    client(
        &jar,
        &[
            (
                "assets/minecraft/textures/entity/player/slim/ari.png",
                64,
                [1, 2, 3, 255],
            ),
            (
                "assets/minecraft/textures/entity/player/wide/alex.png",
                64,
                [4, 5, 6, 255],
            ),
            (
                "assets/minecraft/textures/entity/player/wide/zuri.png",
                64,
                [7, 8, 9, 255],
            ),
        ],
    );
    for (hex, hash, model, color) in [
        (
            "00000000000000000000000000000001",
            1,
            SkinModel::Slim,
            [1, 2, 3, 255],
        ),
        (
            "00000000000000000000000000000009",
            9,
            SkinModel::Wide,
            [4, 5, 6, 255],
        ),
        (
            "000000000000000000000000ffffffff",
            -1,
            SkinModel::Wide,
            [7, 8, 9, 255],
        ),
    ] {
        let id = ProfileId::parse(hex).unwrap();
        assert_eq!(uuid_hash(id), hash);
        let look = from_client(&jar, id);
        let look = look.unwrap();
        assert_eq!(look.model, model);
        assert_eq!(look.skin.unwrap().rgba[..4], color);
    }
}

#[test]
fn legacy_alex_steve_and_missing_artwork_are_read_from_the_jar() {
    let root = tempfile::tempdir().unwrap();
    let jar = root.path().join("legacy.jar");
    let odd = ProfileId::parse("00000000000000000000000000000001").unwrap();
    let even = ProfileId::parse("00000000000000000000000000000002").unwrap();
    assert!(from_client(&jar, even).is_none());
    client(
        &jar,
        &[(
            "assets/minecraft/textures/entity/steve.png",
            32,
            [4, 5, 6, 255],
        )],
    );
    let look = from_client(&jar, odd).unwrap();
    assert_eq!(look.model, SkinModel::Wide);
    assert_eq!(look.skin.unwrap().height, 64);
    client(
        &jar,
        &[
            (
                "assets/minecraft/textures/entity/alex.png",
                64,
                [1, 2, 3, 255],
            ),
            (
                "assets/minecraft/textures/entity/steve.png",
                64,
                [4, 5, 6, 255],
            ),
        ],
    );
    assert_eq!(from_client(&jar, odd).unwrap().model, SkinModel::Slim);
    assert_eq!(from_client(&jar, even).unwrap().model, SkinModel::Wide);
    std::fs::write(&jar, b"broken archive").unwrap();
    assert!(from_client(&jar, even).is_none());
}
