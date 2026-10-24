use super::{Circuit, MAX_POWER, Part, lever_on, render};
use crate::hero::raster::PixelGrid;

#[test]
fn dust_loses_one_level_per_block() {
    let circuit = Circuit::solve();
    assert_eq!(circuit.at((1, 4)), Some((Part::Dust, MAX_POWER)));
    assert_eq!(circuit.at((2, 4)), Some((Part::Dust, 14)));
    assert_eq!(circuit.at((2, 1)), Some((Part::Dust, 11)));
    assert_eq!(circuit.at((3, 1)), Some((Part::Dust, 10)));
    assert_eq!(circuit.at((12, 1)), Some((Part::Dust, 1)));
    assert_eq!(circuit.at((13, 1)), Some((Part::Dust, 0)));
    assert_eq!(circuit.at((15, 1)), Some((Part::Dust, 0)));
}

#[test]
fn the_repeater_restores_full_strength_and_only_its_lamp_lights() {
    let circuit = Circuit::solve();
    assert_eq!(circuit.at((4, 7)), Some((Part::Dust, 9)));
    assert_eq!(circuit.at((6, 7)), Some((Part::Dust, MAX_POWER)));
    assert_eq!(circuit.at((15, 7)), Some((Part::Dust, 6)));
    assert!(circuit.at((16, 7)).is_some_and(|(_, power)| power > 0));
    assert_eq!(circuit.at((16, 1)), Some((Part::Lamp, 0)));
}

#[test]
fn both_tracks_are_the_same_length() {
    let parts = super::board();
    let row = |r: i32| parts.iter().filter(|((_, pr), _)| *pr == r).count();
    assert_eq!(row(1), row(7));
}

#[test]
fn the_signal_travels_as_a_wave() {
    let circuit = Circuit::solve();
    let near = circuit.parts.iter().position(|p| p.0 == (3, 7)).unwrap();
    let far = circuit.parts.iter().position(|p| p.0 == (15, 7)).unwrap();
    assert!(circuit.delay[far] > circuit.delay[near] + 0.5);
    let just_on = 0.9 + circuit.delay[near] + 0.01;
    assert!(circuit.live(near, just_on) > 0);
    assert_eq!(circuit.live(far, just_on), 0);
    assert!(!lever_on(0.) && lever_on(1.) && !lever_on(5.5) && lever_on(7.2));
}

#[test]
fn rendering_is_deterministic() {
    let mut a = PixelGrid::new(170, 72);
    let mut b = PixelGrid::new(170, 72);
    render(&mut a, 3.3);
    render(&mut b, 3.3);
    assert_eq!(a, b);
}
