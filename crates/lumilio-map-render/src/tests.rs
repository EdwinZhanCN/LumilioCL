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
        rgba: [255, 0, 0, 255].repeat(256 * 256),
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
            Grid::default(),
            128,
            64,
        )
        .unwrap();
    assert_eq!(&frame.bgra[(32 * 128 + 64) * 4..][..4], &[0, 0, 255, 255]);
    assert_ne!(&frame.bgra[..4], &[0, 0, 255, 255]);
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
        rgba: [0, 128, 0, 255].repeat(256 * 256),
    }];
    let camera = Camera {
        x: 2048.,
        z: 2048.,
        blocks_per_pixel: 4.,
    };
    scene
        .render(camera, &tiles, Grid::default(), 1800, 1200)
        .unwrap();
    let start = std::time::Instant::now();
    for _ in 0..10 {
        scene
            .render(camera, &tiles, Grid::default(), 1800, 1200)
            .unwrap();
    }
    eprintln!(
        "headless software/no-GPU diagnostic: {:.2} ms/frame, not representative of real hardware",
        start.elapsed().as_secs_f64() * 100.
    );
}
