use super::{Cave, Cell, TORCH_LEVEL, brightness, render};
use crate::hero::raster::PixelGrid;

#[test]
fn torch_light_drops_one_level_per_open_block() {
    let cave = Cave::new(170, 72);
    let age = 30.; // Tunnel complete, every torch fully lit.
    let (torch, _) = cave.light(age);
    let &(c, r, _) = cave.torches.last().expect("torches along the tunnel");
    let at = |c: i32| torch[(r * cave.columns + c) as usize];
    assert_eq!(at(c), TORCH_LEVEL);
    assert_eq!(at(c - 1), TORCH_LEVEL - 1.);
    assert_eq!(at(c - 2), TORCH_LEVEL - 2.);
}

#[test]
fn light_waits_for_the_tunnel_to_open() {
    let cave = Cave::new(170, 72);
    let (&(c, r), _) = cave.path.split_last().expect("a tunnel path");
    let index = (r * cave.columns + c) as usize;
    assert!(matches!(cave.cell_at(c, r, 0.), Some(Cell::Stone { .. })));
    assert_eq!(cave.cell_at(c, r, 30.), Some(Cell::Air));
    let (_, lava_before) = cave.light(0.);
    let (_, lava_after) = cave.light(30.);
    let back = cave.path[cave.path.len() - 5];
    let back_index = (back.1 * cave.columns + back.0) as usize;
    assert!(lava_after[index] > 0.);
    assert_eq!(lava_before[back_index], 0.);
    assert!(lava_after[back_index] > 5.);
}

#[test]
fn mining_advances_through_ten_stages_and_places_torches() {
    let cave = Cave::new(170, 72);
    assert_eq!(cave.mining(0.), None);
    let (index, progress) = cave.mining(2.).expect("mining in progress");
    assert!((0. ..1.).contains(&progress));
    assert!(cave.mined_at(index) > 2.);
    assert!(cave.torches_lit(0.) == 1 && cave.torches_lit(30.) > 2);
}

#[test]
fn brightness_curve_is_monotonic_and_steep() {
    let levels: Vec<f32> = (0..=15).map(|l| brightness(l as f32)).collect();
    assert!(levels.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(levels[0], 0.);
    assert!((levels[15] - 1.).abs() < 1e-5);
    assert!(levels[7] < 0.25);
}

#[test]
fn rendering_is_deterministic_for_odd_sizes() {
    for (w, h) in [(170, 72), (97, 41), (200, 110)] {
        let mut a = PixelGrid::new(w, h);
        let mut b = PixelGrid::new(w, h);
        render(&mut a, 4.2);
        render(&mut b, 4.2);
        assert_eq!(a, b);
    }
}
