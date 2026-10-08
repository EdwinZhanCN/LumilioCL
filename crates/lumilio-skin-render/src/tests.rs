use super::model::V3;
use super::raster::View;
use super::{Arms, Camera, Frame, Player, Texture, render};

const RED: [u8; 4] = [220, 30, 30, 255];
const BLUE: [u8; 4] = [30, 30, 220, 255];
const ORANGE: [u8; 4] = [240, 140, 20, 255];
const GREEN: [u8; 4] = [30, 200, 30, 255];
const YELLOW: [u8; 4] = [240, 230, 20, 255];
const MAGENTA: [u8; 4] = [230, 20, 230, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const SKIN_GREY: [u8; 4] = [120, 120, 120, 255];

fn paint(texture: &mut Texture, x: u32, y: u32, w: u32, h: u32, colour: [u8; 4]) {
    for row in y..y + h {
        for column in x..x + w {
            let at = ((row * texture.width + column) * 4) as usize;
            texture.rgba[at..at + 4].copy_from_slice(&colour);
        }
    }
}

/// A test skin: the first layer grey with a colour per face that matters,
/// a black marker at the face's top-left texel, and an empty second layer.
fn skin() -> Texture {
    let mut skin = Texture::new(64, 64, vec![0; 64 * 64 * 4]).unwrap();
    for (x, y, w, h) in [(0, 0, 32, 16), (0, 16, 56, 16), (16, 48, 32, 16)] {
        paint(&mut skin, x, y, w, h, SKIN_GREY);
    }
    paint(&mut skin, 8, 8, 8, 8, RED); // head front
    paint(&mut skin, 8, 8, 1, 1, BLACK); // its top-left texel
    paint(&mut skin, 24, 8, 8, 8, BLUE); // head back
    paint(&mut skin, 0, 8, 8, 8, ORANGE); // head right
    paint(&mut skin, 20, 20, 8, 12, GREEN); // body front
    skin
}

fn cape() -> Texture {
    let mut cape = Texture::new(64, 32, vec![0; 64 * 32 * 4]).unwrap();
    paint(&mut cape, 1, 1, 10, 16, MAGENTA); // outer side
    paint(&mut cape, 12, 1, 10, 16, BLACK); // the side against the back
    cape
}

const W: u32 = 240;
const H: u32 = 360;

fn camera(yaw: f32) -> Camera {
    Camera {
        yaw,
        pitch: 0.0,
        zoom: 1.0,
    }
}

fn player<'a>(skin: Option<&'a Texture>, cape: Option<&'a Texture>) -> Player<'a> {
    Player {
        skin,
        arms: Arms::Classic,
        cape,
        outer_layer: true,
    }
}

/// The RGBA colour at the pixel where `point` lands.
fn at(frame: &Frame, camera: Camera, point: V3) -> [u8; 4] {
    let (x, y, _) = View::new(camera.clamped(), frame.width, frame.height)
        .project(point)
        .expect("in front of the camera");
    let i = ((y as u32 * frame.width + x as u32) * 4) as usize;
    let b = &frame.bgra[i..i + 4];
    [b[2], b[1], b[0], b[3]]
}

/// Whether a lit colour is still recognisably `expected` (shading only
/// darkens).
fn looks_like(actual: [u8; 4], expected: [u8; 4]) -> bool {
    let strongest = |c: [u8; 4]| (0..3).max_by_key(|&i| c[i]).unwrap_or(0);
    actual[3] == 255
        && (expected[..3] == [0, 0, 0] && actual[..3].iter().all(|&c| c < 30)
            || strongest(actual) == strongest(expected) && actual[..3] != [0, 0, 0])
}

#[test]
fn the_front_shows_the_face_the_right_way_round() {
    let skin = skin();
    let camera = camera(0.0);
    let frame = render(&player(Some(&skin), None), camera, W, H);
    assert!(looks_like(at(&frame, camera, V3::new(0.0, 28.0, 4.0)), RED));
    // The marker texel is the face's top-left as the viewer sees it.
    assert!(looks_like(
        at(&frame, camera, V3::new(-3.5, 31.5, 4.0)),
        BLACK
    ));
    assert!(looks_like(at(&frame, camera, V3::new(3.5, 31.5, 4.0)), RED));
    assert!(looks_like(
        at(&frame, camera, V3::new(0.0, 18.0, 2.0)),
        GREEN
    ));
}

#[test]
fn the_back_and_the_right_side_show_their_faces() {
    let skin = skin();
    let back = camera(std::f32::consts::PI);
    let frame = render(&player(Some(&skin), None), back, W, H);
    assert!(looks_like(at(&frame, back, V3::new(0.0, 28.0, -4.0)), BLUE));
    let right = camera(-std::f32::consts::FRAC_PI_2);
    let frame = render(&player(Some(&skin), None), right, W, H);
    assert!(looks_like(
        at(&frame, right, V3::new(-4.0, 28.0, 0.0)),
        ORANGE
    ));
}

#[test]
fn a_transparent_second_layer_shows_the_first_and_a_painted_one_covers_it() {
    let mut skin = skin();
    let camera = camera(0.0);
    let face = V3::new(0.0, 28.0, 4.5);
    let frame = render(&player(Some(&skin), None), camera, W, H);
    assert!(looks_like(at(&frame, camera, face), RED));
    paint(&mut skin, 40, 8, 8, 8, YELLOW); // hat front
    let frame = render(&player(Some(&skin), None), camera, W, H);
    assert!(looks_like(at(&frame, camera, face), YELLOW));
    let hidden = Player {
        outer_layer: false,
        ..player(Some(&skin), None)
    };
    let frame = render(&hidden, camera, W, H);
    assert!(looks_like(at(&frame, camera, face), RED));
}

#[test]
fn slim_arms_are_narrower_than_classic_ones() {
    let skin = skin();
    let camera = camera(0.0);
    let drawn_in_row = |arms| {
        let frame = render(
            &Player {
                arms,
                ..player(Some(&skin), None)
            },
            camera,
            W,
            H,
        );
        let (_, y, _) = View::new(camera, W, H)
            .project(V3::new(0.0, 15.0, 2.0))
            .unwrap();
        let row = y as u32 * W * 4;
        (0..W)
            .filter(|x| frame.bgra[(row + x * 4 + 3) as usize] == 255)
            .count()
    };
    assert!(drawn_in_row(Arms::Slim) < drawn_in_row(Arms::Classic));
}

#[test]
fn the_cape_hangs_behind_and_shows_its_outer_side() {
    let (skin, cape) = (skin(), cape());
    let back = camera(std::f32::consts::PI);
    let frame = render(&player(Some(&skin), Some(&cape)), back, W, H);
    assert!(looks_like(
        at(&frame, back, V3::new(0.0, 18.0, -3.6)),
        MAGENTA
    ));
    let front = camera(0.0);
    let frame = render(&player(Some(&skin), Some(&cape)), front, W, H);
    assert!(looks_like(
        at(&frame, front, V3::new(0.0, 18.0, 2.0)),
        GREEN
    ));
}

#[test]
fn without_a_skin_the_player_is_plain_grey_on_nothing() {
    let camera = camera(0.0);
    let frame = render(&player(None, None), camera, W, H);
    let face = at(&frame, camera, V3::new(0.0, 28.0, 4.0));
    assert_eq!(face[3], 255);
    assert!(face[0] == face[1] && face[1] == face[2] && face[0] > 100);
    assert_eq!(&frame.bgra[0..4], &[0, 0, 0, 0]);
}

#[test]
fn the_camera_stays_in_range_and_an_empty_picture_is_empty() {
    let wild = Camera {
        yaw: -1.0,
        pitch: 9.0,
        zoom: 99.0,
    }
    .clamped();
    assert!((0.0..std::f32::consts::TAU).contains(&wild.yaw));
    assert_eq!(wild.pitch, Camera::MAX_PITCH);
    assert_eq!(wild.zoom, Camera::MAX_ZOOM);
    let frame = render(&player(None, None), Camera::HOME, 0, 10);
    assert!(frame.bgra.is_empty());
}

/// With `LUMILIO_SKIN_PREVIEW=<folder>`, writes a few views as PNG to look
/// at; otherwise does nothing.
#[test]
fn write_previews_when_asked() {
    let Some(folder) = std::env::var_os("LUMILIO_SKIN_PREVIEW") else {
        return;
    };
    let (skin, cape) = (skin(), cape());
    for (name, yaw, arms) in [
        ("front", 0.0, Arms::Classic),
        ("home", Camera::HOME.yaw, Arms::Classic),
        ("home-slim", Camera::HOME.yaw, Arms::Slim),
        ("back", std::f32::consts::PI, Arms::Classic),
        ("right", -std::f32::consts::FRAC_PI_2, Arms::Classic),
    ] {
        let camera = Camera {
            yaw,
            ..Camera::HOME
        };
        let frame = render(
            &Player {
                arms,
                ..player(Some(&skin), Some(&cape))
            },
            camera,
            W,
            H,
        );
        let rgba: Vec<u8> = frame
            .bgra
            .chunks(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        image::RgbaImage::from_raw(W, H, rgba)
            .unwrap()
            .save(std::path::Path::new(&folder).join(format!("{name}.png")))
            .unwrap();
    }
}
