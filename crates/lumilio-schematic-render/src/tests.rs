use std::io::Write;

use nucleation::UniversalSchematic;

use crate::scene::{game_data_version, rgba_to_bgra};
use crate::{Scene, SceneError, View};

/// A pack with one block, `minecraft:stone`, a flat red cube.
fn tiny_pack() -> Vec<u8> {
    cube_pack("stone", None)
}

/// A pack whose only block, `minecraft:<block>`, is a flat red cube. With a data version it
/// carries a client JAR's `version.json`.
fn cube_pack(block: &str, data_version: Option<i32>) -> Vec<u8> {
    let mut png = Vec::new();
    let red = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 0, 0, 255]));
    image::DynamicImage::ImageRgba8(red)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    let blockstate = format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{block}"}}}}}}"#);
    let model = format!(
        r##"{{"textures":{{"all":"minecraft:block/{block}"}},"elements":[{{"from":[0,0,0],"to":[16,16,16],"faces":{{"down":{{"texture":"#all","cullface":"down"}},"up":{{"texture":"#all","cullface":"up"}},"north":{{"texture":"#all","cullface":"north"}},"south":{{"texture":"#all","cullface":"south"}},"west":{{"texture":"#all","cullface":"west"}},"east":{{"texture":"#all","cullface":"east"}}}}}}]}}"##
    );
    let version = data_version.map(|v| format!(r#"{{"id":"test","world_version":{v}}}"#));
    let mut files: Vec<(String, &[u8])> = vec![
        (
            "pack.mcmeta".into(),
            br#"{"pack":{"pack_format":34,"description":""}}"#,
        ),
        (
            format!("assets/minecraft/blockstates/{block}.json"),
            blockstate.as_bytes(),
        ),
        (
            format!("assets/minecraft/models/block/{block}.json"),
            model.as_bytes(),
        ),
        (format!("assets/minecraft/textures/block/{block}.png"), &png),
    ];
    if let Some(version) = &version {
        files.push(("version.json".into(), version.as_bytes()));
    }
    for (name, bytes) in files {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_client_jar_names_its_data_version() {
    assert_eq!(
        game_data_version(&cube_pack("stone", Some(5023))),
        Some(5023)
    );
    assert_eq!(game_data_version(&tiny_pack()), None);
    assert_eq!(game_data_version(b"not a zip"), None);
}

/// `minecraft:chain` became `minecraft:iron_chain` in data version 4541. A schematic saved
/// before the rename finds the new model in a newer game, and an unconverted one is reported.
#[test]
fn renamed_blocks_are_converted_or_reported() {
    let mut schematic = UniversalSchematic::new("old".into());
    schematic.set_block_str(0, 0, 0, "minecraft:chain[axis=y,waterlogged=false]");
    schematic.metadata.mc_version = Some(4400);
    schematic.metadata.source_data_version = Some(4400);
    let litematic = nucleation::formats::litematic::to_litematic(&schematic).unwrap();

    for (pack, expected) in [
        (cube_pack("iron_chain", Some(5023)), &[][..]),
        (
            cube_pack("iron_chain", None),
            &["minecraft:chain".to_string()][..],
        ),
    ] {
        match Scene::load(&litematic, &pack) {
            Ok(scene) => assert_eq!(scene.undrawable_blocks(), expected),
            Err(SceneError::NoGpu) => {
                eprintln!("skipped: no graphics adapter on this machine");
                return;
            }
            Err(e) => panic!("{e}"),
        }
    }
}

#[test]
fn orbit_turns_and_clamps_pitch() {
    let view = View::new().orbit(10.0, 1000.0);
    assert_eq!(view.pitch_deg, View::MAX_PITCH);
    assert_eq!(view.yaw_deg, 30.0);
    let wrapped = View::new().orbit(100.0, 0.0);
    assert!((0.0..360.0).contains(&wrapped.yaw_deg));
    assert_eq!(wrapped.yaw_deg, 300.0);
}

#[test]
fn zoom_stays_within_its_range() {
    assert_eq!(View::new().zoomed(1000.0).zoom, View::MAX_ZOOM);
    assert_eq!(View::new().zoomed(0.0001).zoom, View::MIN_ZOOM);
}

#[test]
fn first_person_moves_by_heading_without_orbit_fitting_or_pitch_affecting_wasd() {
    let view = View {
        yaw_deg: 0.,
        pitch_deg: 80.,
        position: Some([0., 1.62, 3.]),
        ..View::new()
    };
    assert_eq!(view.moved(0., 0., 1., 2.).position, Some([0., 1.62, 1.]));
    assert_eq!(view.moved(1., 0., 0., 2.).position, Some([2., 1.62, 3.]));
    let rotated = View {
        yaw_deg: 90.,
        ..view
    }
    .moved(0., 0., 1., 2.)
    .position
    .unwrap();
    assert!((rotated[0] + 2.).abs() < 1e-5);
    assert!((rotated[2] - 3.).abs() < 1e-5);
    assert_eq!(view.moved(0., 1., 0., 2.).position, Some([0., 3.62, 3.]));
    assert_eq!(View::new().moved(1., 1., 1., 10.), View::new());
}

#[test]
fn first_person_diagonal_speed_matches_straight_speed_and_mouse_look_clamps() {
    let view = View {
        yaw_deg: 0.,
        position: Some([0.; 3]),
        ..View::new()
    };
    let diagonal = view.moved(1., 1., 1., 5.).position.unwrap();
    let distance = diagonal.iter().map(|axis| axis * axis).sum::<f32>().sqrt();
    assert!((distance - 5.).abs() < 1e-5);
    assert_eq!(view.look(15., 1000.).pitch_deg, View::MAX_PITCH);
    assert_eq!(view.look(15., 0.).yaw_deg, 345.);
}

#[test]
fn first_person_projection_uses_the_eye_and_ignores_model_bounds_and_orbit_zoom() {
    use nucleation::rendering::{
        CameraConfig,
        camera::{compute_view_proj_reversed, project_point},
    };
    let camera = CameraConfig {
        yaw_deg: 0.,
        pitch_deg: 0.,
        position: Some([10., 2., 5.]),
        fov_deg: 70.,
        ..CameraConfig::default()
    };
    let (matrix, _) = compute_view_proj_reversed([0.; 3], [20.; 3], 1., &camera);
    let changed_orbit = CameraConfig {
        zoom: 8.,
        target: Some([50.; 3]),
        ..camera
    };
    let (other, _) = compute_view_proj_reversed([-100.; 3], [100.; 3], 1., &changed_orbit);
    assert_eq!(matrix, other);
    let (x, y) = project_point(&matrix, [10., 2., 4.], 600, 600).unwrap();
    assert!((x - 300.).abs() < 1e-4 && (y - 300.).abs() < 1e-4);
    assert!(project_point(&matrix, [10., 2., 6.], 600, 600).is_none());
}

#[test]
fn red_and_blue_trade_places() {
    let mut pixels = [1, 2, 3, 4, 5, 6, 7, 8];
    rgba_to_bgra(&mut pixels);
    assert_eq!(pixels, [3, 2, 1, 4, 7, 6, 5, 8]);
}

#[test]
fn a_bad_pack_is_rejected_before_adapter_creation() {
    let mut schematic = UniversalSchematic::new("test".into());
    schematic.fill_cuboid_str((0, 0, 0), (1, 1, 1), "minecraft:stone");
    assert!(matches!(
        Scene::from_schematic(&schematic, b"not a zip"),
        Err(SceneError::Pack(_))
    ));
}

#[test]
fn a_file_that_is_not_a_schematic_is_a_parse_error() {
    assert!(matches!(
        Scene::load(b"not a schematic", &tiny_pack()),
        Err(SceneError::Parse(_))
    ));
}

#[test]
fn a_cube_renders_red_pixels_in_the_middle() {
    let mut schematic = UniversalSchematic::new("test".into());
    schematic.fill_cuboid_str((0, 0, 0), (3, 3, 3), "minecraft:stone");
    let mut scene = match Scene::from_schematic(&schematic, &tiny_pack()) {
        Ok(scene) => scene,
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            return;
        }
        Err(e) => panic!("{e}"),
    };
    let frame = scene.render(&View::new(), 64, 48).unwrap();
    assert_eq!((frame.width, frame.height), (64, 48));
    assert_eq!(frame.bgra.len(), 64 * 48 * 4);
    let centre = ((24 * 64) + 32) * 4;
    let px = &frame.bgra[centre..centre + 4];
    // BGRA: red cube, lit but still dominated by red.
    assert!(px[2] > px[0] && px[2] > px[1], "centre pixel {px:?}");
    // A second size goes through the same scene.
    let bigger = scene.render(&View::new().zoomed(2.0), 128, 96).unwrap();
    assert_eq!(bigger.bgra.len(), 128 * 96 * 4);
}

#[test]
fn render_errors_preserve_the_no_adapter_case() {
    assert_eq!(
        SceneError::from_render(nucleation::rendering::RenderError::NoGpuAdapter),
        SceneError::NoGpu
    );
}

#[test]
#[ignore = "release performance measurement; requires a GPU"]
fn preview_timings() {
    use std::time::Instant;
    let started = Instant::now();
    let mut scene = if let Ok(path) = std::env::var("LUMILIO_SCHEMATIC") {
        let schematic = std::fs::read(path).unwrap();
        let pack =
            std::fs::read(std::env::var("LUMILIO_PACK").expect("resource pack path")).unwrap();
        Scene::load(&schematic, &pack).unwrap()
    } else {
        let mut schematic = UniversalSchematic::new("timing fixture".into());
        schematic.fill_cuboid_str((0, 0, 0), (15, 15, 15), "minecraft:stone");
        Scene::from_schematic(&schematic, &tiny_pack()).unwrap()
    };
    eprintln!(
        "load (inputs/parse/mesh/GPU combined): {:?}",
        started.elapsed()
    );
    if let Ok(path) = std::env::var("LUMILIO_FRAME_PATH") {
        let mut frame = scene.render(&View::new(), 1800, 1200).unwrap();
        rgba_to_bgra(&mut frame.bgra);
        image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra)
            .unwrap()
            .save(path)
            .unwrap();
    }
    scene.render(&View::new(), 1800, 1200).unwrap();
    let started = Instant::now();
    for n in 0..100 {
        std::hint::black_box(
            scene
                .render(&View::new().orbit(n as f32, 0.), 1800, 1200)
                .unwrap(),
        );
    }
    eprintln!("frame with BGRA: {:?}", started.elapsed() / 100);
    let mut pixels = vec![127u8; 1800 * 1200 * 4];
    let started = Instant::now();
    for _ in 0..100 {
        rgba_to_bgra(std::hint::black_box(&mut pixels));
    }
    eprintln!("BGRA only: {:?}", started.elapsed() / 100);
}
