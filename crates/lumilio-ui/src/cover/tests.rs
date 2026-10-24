use super::*;

#[gpui::test]
fn a_game_starting_empties_the_covers(cx: &mut gpui::TestAppContext) {
    let key = (1, 0, 0, 2, 2, 0, 0, 0, false);
    let picture = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
    let cover = Arc::new(RenderImage::new([image::Frame::new(picture)]));
    CACHE.with(|cache| cache.borrow_mut().insert(key, cover));
    cx.update(release);
    cx.run_until_parked();
    assert!(CACHE.with(|cache| cache.borrow().is_empty()));
}

fn cover(seed: u32, loader: Loader) -> Vec<u32> {
    let page = Rgb::hex(0x101010);
    let dissolve = PageDissolve::solid(page).bottom(6);
    let grid = render(seed, loader, WorldHint::Overworld, 40, 20, dissolve);
    (0..20)
        .flat_map(|y| (0..40).map(move |x| (x, y)))
        .map(|(x, y)| grid.get(x, y).pack())
        .collect()
}

#[test]
fn covers_are_deterministic_per_seed() {
    assert_eq!(cover(3, Loader::Fabric), cover(3, Loader::Fabric));
    assert_ne!(cover(3, Loader::Fabric), cover(4, Loader::Fabric));
    assert_ne!(cover(3, Loader::Fabric), cover(3, Loader::Forge));
}

#[test]
fn rasterizing_maps_whole_blocks_and_swaps_to_bgra() {
    let mut grid = PixelGrid::new(2, 1);
    grid.set(0, 0, Rgb::hex(0x102030));
    grid.set(1, 0, Rgb::hex(0xa0b0c0));
    let bytes = rasterize(&grid, PageDissolve::solid(Rgb::BLACK), 4, 2);
    assert_eq!(bytes.len(), 4 * 2 * 4);
    // Blue, green, red, opaque: the first texel covers two device pixels.
    assert_eq!(&bytes[0..4], &[0x30, 0x20, 0x10, 0xff]);
    assert_eq!(&bytes[4..8], &[0x30, 0x20, 0x10, 0xff]);
    assert_eq!(&bytes[8..12], &[0xc0, 0xb0, 0xa0, 0xff]);
    assert_eq!(&bytes[28..32], &[0xc0, 0xb0, 0xa0, 0xff]);
}

#[test]
fn the_last_row_is_exactly_the_page() {
    let page = Rgb::hex(0x101010);
    let dissolve = PageDissolve::solid(page).bottom(6);
    let grid = render(9, Loader::Quilt, WorldHint::Nether, 40, 20, dissolve);
    for x in 0..40 {
        assert_eq!(grid.get(x, 19).pack(), page.pack());
    }
}

#[test]
fn a_corner_cover_also_ends_on_the_page_at_its_leading_edge() {
    let page = Rgb::hex(0x101010);
    let dissolve = PageDissolve::solid(page).bottom(6).left(36);
    let grid = render(9, Loader::Quilt, WorldHint::Nether, 40, 20, dissolve);
    for y in 0..20 {
        assert_eq!(grid.get(0, y).pack(), page.pack());
    }
}

/// The Instance header's cover on a translucent page: the texels that
/// dissolved into the page are transparent, so the glass shows through
/// instead of a block of the page colour; the corner keeps its art. On a
/// solid page every texel is painted.
#[test]
fn a_corner_cover_dissolves_into_the_glass() {
    let alphas = |page: Hsla| {
        let dissolve = PageDissolve::of(page).bottom(6).left(36);
        let grid = render(9, Loader::Fabric, WorldHint::Overworld, 40, 20, dissolve);
        rasterize(&grid, dissolve, 40, 20)
            .chunks(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>()
    };
    let page = gpui::hsla(0.17, 0.04, 0.89, 1.);
    assert!(alphas(page).iter().all(|&alpha| alpha == 0xff));
    let glass = alphas(page.opacity(0.75));
    assert!((0..20).all(|y| glass[y * 40] == 0), "the leading edge");
    assert!((0..40).all(|x| glass[19 * 40 + x] == 0), "the last row");
    assert!(
        (0..14).all(|y| (36..40).all(|x| glass[y * 40 + x] == 0xff)),
        "the corner keeps its art"
    );
}
