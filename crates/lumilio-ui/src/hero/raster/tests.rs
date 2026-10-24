use super::{PixelGrid, Rgb, Run};

#[test]
fn hex_round_trips_through_pack() {
    for value in [0x000000, 0xffffff, 0x7d4a2e, 0x3a0f7a] {
        assert_eq!(Rgb::hex(value).pack(), value);
    }
    assert_eq!(Rgb::new(2., -1., 0.5).pack(), 0xff0080);
}

#[test]
fn runs_merge_identical_neighbours_and_cover_every_texel() {
    let mut grid = PixelGrid::new(6, 2);
    grid.fill_rect(0, 0, 3, 1, Rgb::hex(0xff0000));
    grid.fill_rect(3, 0, 3, 1, Rgb::hex(0x00ff00));
    grid.fill_rect(0, 1, 6, 1, Rgb::hex(0x0000ff));
    assert_eq!(
        grid.runs(),
        vec![
            Run {
                row: 0,
                start: 0,
                len: 3,
                color: 0xff0000
            },
            Run {
                row: 0,
                start: 3,
                len: 3,
                color: 0x00ff00
            },
            Run {
                row: 1,
                start: 0,
                len: 6,
                color: 0x0000ff
            },
        ]
    );
}

#[test]
fn rects_merge_repeated_rows_and_still_cover_every_texel() {
    let mut flat = PixelGrid::new(30, 20);
    flat.shade(|_, _| Rgb::hex(0x336699));
    assert_eq!(flat.rects().len(), 1);

    let mut grid = PixelGrid::new(40, 24);
    grid.shade(|x, y| {
        if y < 10 {
            Rgb::hex(0x223344)
        } else if (x + y) % 3 == 0 {
            Rgb::hex(0xffffff)
        } else {
            Rgb::hex(0x000000)
        }
    });
    let rects = grid.rects();
    let mut covered = vec![false; 40 * 24];
    for rect in &rects {
        for y in rect.row..rect.row + rect.rows {
            for x in rect.start..rect.start + rect.len {
                assert!(!covered[y * 40 + x], "rects never overlap");
                covered[y * 40 + x] = true;
                assert_eq!(grid.get(x, y).pack(), rect.color);
            }
        }
    }
    for y in 0..24 {
        for x in 0..40 {
            assert!(covered[y * 40 + x], "every texel is painted");
        }
    }
    assert!(rects.len() < grid.runs().len());
}

#[test]
fn extended_rows_repeat_the_last_row_and_the_fade_ends_on_the_page() {
    let mut grid = PixelGrid::new(8, 4);
    grid.shade(|x, y| Rgb::new(x as f32 / 8., y as f32 / 4., 0.5));
    grid.extend_to(10);
    assert_eq!(grid.height(), 10);
    for x in 0..8 {
        assert_eq!(grid.get(x, 9), grid.get(x, 3));
    }
    let page = Rgb::hex(0xf7f7f8);
    grid.fade_bottom(6, page);
    assert!(
        (0..8).all(|x| grid.get(x, 9) == page),
        "last row is the page"
    );
    let dissolved = |y| (0..8).filter(|&x| grid.get(x, y) == page).count();
    assert!(dissolved(4) < dissolved(7), "density rises toward the page");
    assert_ne!(grid.get(0, 3), page, "rows above the band are untouched");
}

#[test]
fn the_left_fade_ends_on_the_page_and_thins_toward_the_right() {
    let mut grid = PixelGrid::new(10, 8);
    grid.shade(|x, y| Rgb::new(x as f32 / 10., y as f32 / 8., 0.5));
    let page = Rgb::hex(0xf7f7f8);
    let right = grid.get(9, 3);
    grid.fade_left(6, page);
    assert!(
        (0..8).all(|y| grid.get(0, y) == page),
        "first column is the page"
    );
    assert_eq!(grid.get(9, 3), right, "columns past the band are untouched");
}

#[test]
fn out_of_bounds_sprite_writes_are_ignored() {
    let mut grid = PixelGrid::new(4, 4);
    grid.put(-1, 2, Rgb::hex(0xffffff));
    grid.put(4, 0, Rgb::hex(0xffffff));
    grid.blend(2, 9, Rgb::hex(0xffffff), 1.);
    assert!(grid.runs().iter().all(|run| run.color == 0));
}

#[test]
fn scrim_darkens_the_copy_corner_only() {
    let mut grid = PixelGrid::new(80, 40);
    grid.shade(|_, _| Rgb::hex(0x808080));
    grid.apply_scrim(0.7);
    assert!(grid.get(2, 38).luma() < grid.get(2, 2).luma());
    assert_eq!(grid.get(78, 2), Rgb::hex(0x808080));
}
