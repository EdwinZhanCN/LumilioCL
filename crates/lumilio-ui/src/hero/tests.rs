use super::{
    FrameSpec, Framing, Landmark, MAX_COLUMNS, MIN_COLUMNS, Scene, grid_layout, rasterize,
};
use crate::hero::raster::Rgb;

const DARK_PAGE: Framing = Framing {
    fade_rows: 12,
    page: Rgb::new(0.07, 0.07, 0.08),
};
const LIGHT_PAGE: Framing = Framing {
    fade_rows: 12,
    page: Rgb::new(1., 1., 1.),
};

#[test]
fn grid_layout_tiles_the_canvas_exactly_and_caps_the_cost() {
    for (width, height) in [(1040., 420.), (656., 220.), (3000., 560.), (10., 10.)] {
        let (columns, rows, tw, th) = grid_layout(width, height);
        assert!((columns as f32 * tw - width).abs() < 1e-3);
        assert!((rows as f32 * th - height).abs() < 1e-3);
        assert!((MIN_COLUMNS..=MAX_COLUMNS).contains(&(columns as f32)));
        assert!(rows >= 12);
    }
    let (_, _, tw, th) = grid_layout(1040., 420.);
    assert!((tw - th).abs() / tw < 0.05, "texels stay close to square");
}

#[test]
fn transitions_rasterise_both_scenes_at_one_resolution() {
    let frame = FrameSpec {
        scene: Scene::Portal,
        age: 0.4,
        leaving: Some((Scene::Redstone, 9.8, 0.5)),
        reveal: 0.5,
        dim: true,
        flare: 0.,
    };
    let grid = rasterize(frame, 120, 50, DARK_PAGE);
    assert_eq!((grid.width(), grid.height()), (120, 50));
}

#[test]
fn the_world_dissolves_into_the_live_page_colour() {
    let frame = FrameSpec {
        scene: Scene::Hearth(Landmark::Portal),
        age: 2.,
        leaving: None,
        reveal: 1.,
        dim: false,
        flare: 0.,
    };
    for framing in [DARK_PAGE, LIGHT_PAGE] {
        let grid = rasterize(frame, 150, 70, framing);
        assert!((0..150).all(|x| grid.get(x, 69) == framing.page));
        assert!((0..150).any(|x| grid.get(x, 40) != framing.page));
    }
}

/// Visual review aid: writes a contact sheet of every scene and a
/// transition as binary PPM files.
/// `LUMILIO_HERO_DUMP=/some/dir cargo nextest run -p lumilio-ui hero_contact_sheet --ignored`
#[test]
#[ignore = "writes images for manual review"]
fn hero_contact_sheet() {
    let Ok(dir) = std::env::var("LUMILIO_HERO_DUMP") else {
        return;
    };
    let (columns, rows, _, _) = grid_layout(1040., 440.);
    let mut frames = Vec::new();
    for scene in Scene::ALL {
        for age in [0.6, 2.4, 5., scene.still_age(), 9.2] {
            frames.push((
                format!("{scene:?}-{age}"),
                FrameSpec {
                    scene,
                    age,
                    leaving: None,
                    reveal: 1.,
                    dim: false,
                    flare: 0.,
                },
            ));
        }
    }
    for (name, landmark) in [
        ("village", Landmark::Village),
        ("portal", Landmark::Portal),
        ("mine", Landmark::Mine),
        ("lamp", Landmark::Lamp),
    ] {
        for flare in [0., 1.] {
            frames.push((
                format!("hearth-{name}-flare{flare}"),
                FrameSpec {
                    scene: Scene::Hearth(landmark),
                    age: 2.6,
                    leaving: None,
                    reveal: 1.,
                    dim: false,
                    flare,
                },
            ));
        }
    }
    for progress in [0.15, 0.4, 0.7] {
        frames.push((
            format!("transition-{progress}"),
            FrameSpec {
                scene: Scene::Caves,
                age: progress,
                leaving: Some((Scene::Dawn, 9.5, progress)),
                reveal: 1.,
                dim: false,
                flare: 0.,
            },
        ));
        frames.push((
            format!("launch-{progress}"),
            FrameSpec {
                scene: Scene::Hearth(Landmark::Portal),
                age: 5.,
                leaving: None,
                reveal: progress,
                dim: false,
                flare: 0.,
            },
        ));
    }
    frames.push((
        "playing".into(),
        FrameSpec {
            scene: Scene::Hearth(Landmark::Portal),
            age: 3.,
            leaving: None,
            reveal: 1.,
            dim: true,
            flare: 0.,
        },
    ));
    const SCALE: usize = 4;
    let mut framed = Vec::new();
    for (name, frame) in frames {
        framed.push((format!("{name}-dark"), frame, DARK_PAGE));
        if name.starts_with("hearth") || name.starts_with("Dawn") {
            framed.push((format!("{name}-light"), frame, LIGHT_PAGE));
        }
    }
    for (name, frame, framing) in framed {
        let grid = rasterize(frame, columns, rows, framing);
        let (w, h) = (grid.width() * SCALE, grid.height() * SCALE);
        let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                let (gx, gy) = (x / SCALE, y / SCALE);
                let packed = grid.get(gx, gy).pack();
                bytes.extend([(packed >> 16) as u8, (packed >> 8) as u8, packed as u8]);
            }
        }
        std::fs::write(format!("{dir}/{name}.ppm"), bytes).expect("write frame");
    }
}

#[test]
fn a_chosen_picture_ends_exactly_in_the_page_colour_and_breaks_up_on_the_way() {
    use super::wallpaper::{is_page, veil};
    let (columns, rows) = (64, 13);
    let page_cells = |band: usize| (0..columns).filter(|x| is_page(*x, band, rows)).count();
    assert_eq!(page_cells(rows - 1), columns, "the last row is all page");
    assert!(page_cells(0) < columns / 4, "the top row is mostly picture");
    // Ordered dither picks a different threshold row by row, so single rows
    // are not monotonic; the band as a whole thickens toward the page.
    let upper: usize = (0..rows / 2).map(page_cells).sum();
    let lower: usize = (rows / 2..rows).map(page_cells).sum();
    assert!(
        upper * 2 < lower,
        "{upper} page texels above, {lower} below"
    );
    assert!((0..rows).any(|band| (1..columns).contains(&page_cells(band))));
    assert!((0..rows).all(|band| veil(band, rows) <= 0.65 + f32::EPSILON));
}

/// On a translucent page a picture fades out through nested layers: stacked,
/// they give every strip of the band its eased opacity, from almost the whole
/// picture at the top to nothing of it at the bottom.
#[test]
fn a_picture_fades_into_the_glass_strip_by_strip() {
    let strips = 44;
    let layers = super::wallpaper::fade_layers(strips);
    assert_eq!(layers.len(), strips);
    let shown: Vec<f32> = (0..strips)
        .map(|j| 1. - layers[j..].iter().map(|alpha| 1. - alpha).product::<f32>())
        .collect();
    for (j, shown) in shown.iter().enumerate() {
        let t = (j as f32 + 0.5) / strips as f32;
        let wanted = 1. - super::noise::smoothstep(0., 1., t);
        assert!(
            (shown - wanted).abs() < 1e-4,
            "strip {j}: {shown} vs {wanted}"
        );
    }
    assert!(
        shown.windows(2).all(|pair| pair[0] > pair[1]),
        "it only fades"
    );
    assert!(shown[0] > 0.95 && shown[strips - 1] < 0.05);
}

/// On a translucent page the world's dissolved texels are left out instead of
/// painted, so it breaks up into the glass; on a solid page all are painted.
#[test]
fn the_world_dissolves_into_the_glass_on_a_translucent_page() {
    let page = Rgb::hex(0xe4e4e1);
    let mut grid = super::raster::PixelGrid::new(8, 10);
    for y in 0..10 {
        for x in 0..8 {
            grid.set(x, y, Rgb::hex(0x203040));
        }
    }
    grid.fade_bottom(6, page);
    let band = 10 - 6;
    let solid = super::painted(&grid, None);
    let glass = super::painted(&grid, Some((band, page.pack())));
    let page_in_band = |rects: &[super::raster::Rect]| {
        rects
            .iter()
            .filter(|rect| rect.row >= band && rect.color == page.pack())
            .count()
    };
    assert!(page_in_band(&solid) > 0, "a solid page paints the dissolve");
    assert_eq!(page_in_band(&glass), 0, "the glass shows through it");
    assert!(
        glass.iter().any(|rect| rect.row >= band),
        "the surviving texels of the band are still painted"
    );
}
