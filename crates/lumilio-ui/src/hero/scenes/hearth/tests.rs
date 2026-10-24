use super::{FIRE_LEVEL, Landmark, Layout, fire_light, flame_height, render};
use crate::hero::raster::PixelGrid;

#[test]
fn fire_light_drops_one_level_per_block_and_flares_further() {
    let layout = Layout::new(160, 70);
    let (fx, fy) = (layout.fire_x, layout.ground - 1);
    assert_eq!(fire_light(fx, fy, &layout, 0.), FIRE_LEVEL);
    let one_block = fire_light(fx + 4, fy, &layout, 0.);
    let two_blocks = fire_light(fx + 8, fy, &layout, 0.);
    assert_eq!(one_block - two_blocks, 1.);
    assert!(fire_light(fx + 60, fy, &layout, 1.) > fire_light(fx + 60, fy, &layout, 0.));
}

#[test]
fn flaring_makes_a_taller_fire() {
    for age in [0., 1.3, 4.2] {
        assert!(flame_height(age, 1.) > flame_height(age, 0.));
    }
}

#[test]
fn landmarks_change_only_the_far_hill() {
    let render_with = |landmark| {
        let mut grid = PixelGrid::new(160, 70);
        render(&mut grid, 2., landmark, 0.);
        grid
    };
    let village = render_with(Landmark::Village);
    let portal = render_with(Landmark::Portal);
    assert_ne!(village, portal);
    let layout = Layout::new(160, 70);
    for y in 0..70 {
        for x in 0..(layout.landmark_x - 16) as usize {
            assert_eq!(village.get(x, y), portal.get(x, y), "({x}, {y})");
        }
    }
    let violet = (0..70)
        .flat_map(|y| (0..160).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let texel = portal.get(x, y);
            texel.b > 0.6 && texel.r < 0.8 && texel.g < 0.5
        })
        .count();
    assert!(violet > 4, "the distant portal glows");
}

#[test]
fn rendering_is_deterministic_at_any_size() {
    for (w, h) in [(160, 70), (48, 20), (190, 100)] {
        let mut a = PixelGrid::new(w, h);
        let mut b = PixelGrid::new(w, h);
        render(&mut a, 3.3, Landmark::Mine, 0.5);
        render(&mut b, 3.3, Landmark::Mine, 0.5);
        assert_eq!(a, b);
    }
}
