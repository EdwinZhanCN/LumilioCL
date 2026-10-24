use super::{IGNITE_AT, Layout, charge, render};
use crate::hero::raster::PixelGrid;

#[test]
fn the_frame_is_four_by_five_with_a_two_by_three_interior() {
    for (w, h) in [(170, 72), (120, 90), (200, 50)] {
        let layout = Layout::new(w, h);
        let b = layout.block;
        let (_, fy, fw, fh) = layout.frame;
        assert_eq!((fw, fh), (4 * b, 5 * b));
        let (_, _, iw, ih) = layout.inner();
        assert_eq!((iw, ih), (2 * b, 3 * b));
        assert_eq!(fy + fh, layout.floor, "the frame stands on the floor");
        assert!(fy >= 0);
    }
}

#[test]
fn the_portal_lights_after_the_spark() {
    assert_eq!(charge(0.), 0.);
    assert_eq!(charge(IGNITE_AT), 0.);
    assert_eq!(charge(IGNITE_AT + 2.), 1.);

    let (w, h) = (170, 72);
    let layout = Layout::new(w, h);
    let (ix, iy, iw, ih) = layout.inner();
    let centre = ((ix + iw / 2) as usize, (iy + ih / 2) as usize);
    let mut before = PixelGrid::new(w as usize, h as usize);
    render(&mut before, 0.5);
    let mut after = PixelGrid::new(w as usize, h as usize);
    render(&mut after, 5.);
    let violet = |grid: &PixelGrid| {
        let texel = grid.get(centre.0, centre.1);
        texel.b > texel.g && texel.b > 0.3
    };
    assert!(!violet(&before));
    assert!(violet(&after));
}
