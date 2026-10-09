use crate::*;

#[test]
fn colored_tile_position_zoom_and_bgra_readback() {
    let mut scene = match Scene::new() {
        Ok(scene) => scene,
        Err(Error::NoGpu) => return,
        Err(error) => panic!("{error:?}"),
    };
    let tile = Tile {
        id: "red".into(),
        x: 0.,
        z: 0.,
        span: 256.,
        rgba: [255, 0, 0, 255].repeat(256 * 256).into(),
    };
    let camera = Camera {
        x: 128.,
        z: 128.,
        blocks_per_pixel: 1.,
    };
    let frame = scene
        .render(
            camera,
            std::slice::from_ref(&tile),
            &[],
            Grid::default(),
            128,
            64,
        )
        .unwrap();
    assert_eq!(frame.bgra.len(), 128 * 64 * 4);
    assert!(
        frame
            .bgra
            .as_chunks::<4>()
            .0
            .iter()
            .all(|pixel| *pixel == [0, 0, 255, 255])
    );
    let frame = scene
        .render(
            Camera {
                blocks_per_pixel: 4.,
                ..camera
            },
            &[tile],
            &[],
            Grid::default(),
            128,
            64,
        )
        .unwrap();
    assert_eq!(&frame.bgra[(32 * 128 + 64) * 4..][..4], &[0, 0, 255, 255]);
    assert_ne!(&frame.bgra[..4], &[0, 0, 255, 255]);
}

/// An 80x80 icon: opaque red left half, fully transparent right half.
fn half_icon() -> std::sync::Arc<[u8]> {
    (0..80 * 80)
        .flat_map(|pixel| {
            if pixel % 80 < 40 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect()
}

fn sprite(x: f64, y: f64, opacity: f32) -> Sprite {
    Sprite {
        id: "half".into(),
        width: 80,
        height: 80,
        rgba: half_icon(),
        x,
        y,
        size: 20,
        opacity,
    }
}

fn green_tile() -> Tile {
    Tile {
        id: "green".into(),
        x: 0.,
        z: 0.,
        span: 256.,
        rgba: [0, 255, 0, 255].repeat(256 * 256).into(),
    }
}

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * frame.width + x) * 4) as usize;
    [
        frame.bgra[at],
        frame.bgra[at + 1],
        frame.bgra[at + 2],
        frame.bgra[at + 3],
    ]
}

#[test]
fn sprites_draw_over_tiles_blend_alpha_and_stay_where_asked() {
    let mut scene = match Scene::new() {
        Ok(scene) => scene,
        Err(Error::NoGpu) => return,
        Err(error) => panic!("{error:?}"),
    };
    let camera = Camera {
        x: 64.,
        z: 32.,
        blocks_per_pixel: 1.,
    };
    let render = |scene: &mut Scene, sprites: &[Sprite]| {
        scene
            .render(camera, &[green_tile()], sprites, Grid::default(), 128, 64)
            .unwrap()
    };
    let bare = render(&mut scene, &[]);
    assert_eq!(pixel(&bare, 40, 30), [0, 255, 0, 255]);
    // Centre (40, 30), 20 px wide: it covers x 30..50, y 20..40. The opaque
    // half is x 30..40 (BGRA red); the transparent half shows the green tile.
    let frame = render(&mut scene, &[sprite(40., 30., 1.)]);
    assert_eq!(
        pixel(&frame, 32, 30),
        [0, 0, 255, 255],
        "opaque half is red"
    );
    assert_eq!(
        pixel(&frame, 47, 30),
        [0, 255, 0, 255],
        "transparent half shows the tile"
    );
    assert_eq!(
        pixel(&frame, 28, 30),
        [0, 255, 0, 255],
        "left of the sprite"
    );
    assert_eq!(pixel(&frame, 32, 18), [0, 255, 0, 255], "above the sprite");
    assert_eq!(pixel(&frame, 32, 41), [0, 255, 0, 255], "below the sprite");
    // Half opacity mixes the icon with what is under it.
    let faded = pixel(&render(&mut scene, &[sprite(40., 30., 0.5)]), 32, 30);
    assert!(
        i32::from(faded[0]) <= 2 && (i32::from(faded[2]) - 128).abs() <= 3,
        "{faded:?}"
    );
    assert!((i32::from(faded[1]) - 128).abs() <= 3, "{faded:?}");
    // A sprite wholly or partly off screen is accepted and clipped.
    render(
        &mut scene,
        &[
            sprite(-500., 30., 1.),
            sprite(5., 2., 1.),
            sprite(900., 900., 1.),
        ],
    );
    // Malformed sprites are refused.
    let mut bad = sprite(10., 10., 1.);
    bad.size = 0;
    assert_eq!(
        scene
            .render(camera, &[], &[bad], Grid::default(), 128, 64)
            .unwrap_err(),
        Error::InvalidInput
    );
}

#[test]
fn scaling_a_sprite_averages_without_dark_fringes() {
    // 4x4 with an opaque white left half and a transparent black right half.
    let icon: Vec<u8> = (0..16)
        .flat_map(|pixel| {
            if pixel % 4 < 2 {
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 0]
            }
        })
        .collect();
    let small = crate::scene::scale_to(&icon, 4, 4, 2);
    assert_eq!(&small[..4], &[255, 255, 255, 255]);
    assert_eq!(&small[4..8], &[0, 0, 0, 0]);
    // A mixed cell stays white (premultiplied average), only its alpha drops.
    let one = crate::scene::scale_to(&icon, 4, 4, 1);
    assert_eq!(&one[..3], &[255, 255, 255]);
    assert_eq!(one[3], 127);
}

#[test]
#[ignore = "headless frame-time diagnostic, not a hardware acceptance gate"]
fn headless_frame_time() {
    let mut scene = match Scene::new() {
        Ok(scene) => scene,
        Err(Error::NoGpu) => {
            eprintln!("NoGpu: measurement unavailable");
            return;
        }
        Err(error) => panic!("{error:?}"),
    };
    let tiles = vec![Tile {
        id: "measure".into(),
        x: 0.,
        z: 0.,
        span: 4096.,
        rgba: [0, 128, 0, 255].repeat(256 * 256).into(),
    }];
    let camera = Camera {
        x: 2048.,
        z: 2048.,
        blocks_per_pixel: 4.,
    };
    scene
        .render(camera, &tiles, &[], Grid::default(), 1800, 1200)
        .unwrap();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        scene
            .render(camera, &tiles, &[], Grid::default(), 1800, 1200)
            .unwrap();
    }
    eprintln!(
        "headless software/no-GPU diagnostic: {:.2} ms/frame, not representative of real hardware",
        start.elapsed().as_secs_f64() * 100.
    );
}

#[test]
#[ignore = "start-up measurement, prints a report"]
fn scene_startup_time() {
    let started = std::time::Instant::now();
    let mut scene = match Scene::new() {
        Ok(scene) => scene,
        Err(error) => {
            eprintln!("{error:?}");
            return;
        }
    };
    println!(
        "Scene::new {:.2}s on {}",
        started.elapsed().as_secs_f64(),
        scene.adapter()
    );
    for (name, took) in scene.startup() {
        println!("  {name}: {:.2}s", took.as_secs_f64());
    }
    let tile = Tile {
        id: "t".into(),
        x: 0.,
        z: 0.,
        span: 256.,
        rgba: [0, 128, 0, 255].repeat(256 * 256).into(),
    };
    for n in 0..3 {
        let began = std::time::Instant::now();
        let frame = scene
            .render(
                Camera {
                    x: 128.,
                    z: 128.,
                    blocks_per_pixel: 1.,
                },
                std::slice::from_ref(&tile),
                &[],
                Grid::default(),
                1000,
                700,
            )
            .unwrap();
        println!(
            "frame {n}: {:.2} ms {:?}",
            began.elapsed().as_secs_f64() * 1e3,
            frame.timings
        );
    }
}
