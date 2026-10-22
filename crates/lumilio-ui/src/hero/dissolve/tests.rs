use super::PageDissolve;
use crate::hero::raster::{PixelGrid, Rect, Rgb};

const ART: Rgb = Rgb::hex(0x203040);

fn art(columns: usize, rows: usize) -> PixelGrid {
    let mut grid = PixelGrid::new(columns, rows);
    grid.shade(|_, _| ART);
    grid
}

fn page_rects(rects: &[Rect], page: Rgb, inside: impl Fn(&Rect) -> bool) -> usize {
    rects
        .iter()
        .filter(|rect| rect.color == page.pack() && inside(rect))
        .count()
}

/// The live page colour is read without its opacity, and its opacity decides
/// whether the band is painted or left to the glass.
#[test]
fn the_page_is_read_from_the_live_colour() {
    let page = gpui::hsla(0.1, 0.05, 0.9, 1.);
    let solid = PageDissolve::of(page);
    let glass = PageDissolve::of(page.opacity(0.75));
    assert!(!solid.translucent);
    assert!(glass.translucent);
    assert_eq!(solid.page.pack(), glass.page.pack());
}

/// On a translucent page the world's dissolved texels are left out instead of
/// painted, so it breaks up into the glass; on a solid page all are painted.
#[test]
fn the_world_dissolves_into_the_glass_on_a_translucent_page() {
    let page = Rgb::hex(0xe4e4e1);
    let mut grid = art(8, 10);
    let solid = PageDissolve::solid(page).bottom(6);
    let glass = PageDissolve {
        translucent: true,
        ..solid
    };
    solid.apply(&mut grid);
    let band = 10 - 6;
    let in_band = |rect: &Rect| rect.row >= band;
    let solid = solid.painted(&grid);
    let glass = glass.painted(&grid);
    assert!(
        page_rects(&solid, page, in_band) > 0,
        "a solid page paints the dissolve"
    );
    assert_eq!(
        page_rects(&glass, page, in_band),
        0,
        "the glass shows through it"
    );
    assert!(
        glass.iter().any(in_band),
        "the surviving texels of the band are still painted"
    );
}

/// A cover in the corner dissolves leftward too: on glass its leading edge
/// and its last row are left out entirely, and nothing is left out beyond
/// the two bands, even a texel that happens to be the page colour.
#[test]
fn a_corner_dissolves_into_the_glass_on_both_edges() {
    let page = Rgb::hex(0xe4e4e1);
    let (columns, rows) = (40, 20);
    let mut grid = art(columns, rows);
    grid.set(columns - 1, 0, page);
    let glass = PageDissolve {
        translucent: true,
        ..PageDissolve::solid(page).bottom(6).left(36)
    };
    glass.apply(&mut grid);
    let clears = |x: usize, y: usize| glass.clears(x, y, rows, grid.get(x, y).pack());
    assert!((0..rows).all(|y| clears(0, y)), "the leading edge is glass");
    assert!(
        (0..columns).all(|x| clears(x, rows - 1)),
        "the last row is glass"
    );
    assert!(!clears(columns - 1, 0), "the corner keeps its art");
    assert!(
        (0..rows)
            .flat_map(|y| (0..columns).map(move |x| (x, y)))
            .filter(|&(x, y)| clears(x, y))
            .all(|(x, y)| x < 36 || y >= rows - 6),
        "only the bands are left out"
    );
    let solid = PageDissolve {
        translucent: false,
        ..glass
    };
    assert!(
        (0..rows).all(|y| (0..columns).all(|x| !solid.clears(x, y, rows, grid.get(x, y).pack()))),
        "a solid page paints every texel"
    );
}
