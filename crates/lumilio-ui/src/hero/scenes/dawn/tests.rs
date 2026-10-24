use super::{clock, daylight, horizon_row, render, sun};
use crate::hero::raster::PixelGrid;

#[test]
fn the_slide_is_a_sunrise() {
    let (w, h) = (170, 72);
    assert!(daylight(0.) < 0.1);
    assert!(daylight(9.) > 0.95);
    let (_, start_y, half) = sun(w, h, daylight(0.));
    let (_, end_y, _) = sun(w, h, daylight(9.));
    assert!(
        start_y - half > horizon_row(h),
        "sun starts below the horizon"
    );
    assert!(end_y < horizon_row(h) / 2, "sun ends high in the sky");
    assert!(clock(0.) < clock(9.));
    assert_eq!(clock(0.), "05:04");
}

#[test]
fn sky_brightens_and_rendering_is_deterministic() {
    let sky_luma = |age: f32| {
        let mut grid = PixelGrid::new(170, 72);
        render(&mut grid, age);
        let mut again = PixelGrid::new(170, 72);
        render(&mut again, age);
        assert_eq!(grid, again);
        (0..170).map(|x| grid.get(x, 2).luma()).sum::<f32>()
    };
    assert!(sky_luma(9.) > sky_luma(0.) * 2.);
}
