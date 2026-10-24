use super::{CHUNK, chunk_phase, chunk_ranks, composite};
use crate::hero::raster::{PixelGrid, Rgb};

#[test]
fn ranks_are_a_permutation_starting_at_the_centre() {
    let ranks = chunk_ranks(5, 3);
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (0..15).collect::<Vec<_>>());
    assert_eq!(ranks[7], 0, "centre chunk loads first");
    // Every ring-1 chunk loads before any ring-2 chunk.
    let ring = |index: usize| {
        let dx = (index % 5) as i32 - 2;
        let dy = (index / 5) as i32 - 1;
        dx.abs().max(dy.abs())
    };
    for a in 0..15 {
        for b in 0..15 {
            if ring(a) < ring(b) {
                assert!(ranks[a] < ranks[b]);
            }
        }
    }
}

#[test]
fn phases_start_staggered_and_all_finish() {
    assert_eq!(chunk_phase(0, 10, 0.), 0.);
    assert!(chunk_phase(0, 10, 0.2) > chunk_phase(9, 10, 0.2));
    for rank in 0..10 {
        assert_eq!(chunk_phase(rank, 10, 1.), 1.);
    }
    assert_eq!(chunk_phase(0, 1, 1.), 1.);
}

#[test]
fn loaded_chunk_count_grows_with_progress() {
    let (none, total) = super::loaded_chunks(CHUNK * 4, CHUNK * 3, 0.);
    assert_eq!((none, total), (0, 12));
    let (half, _) = super::loaded_chunks(CHUNK * 4, CHUNK * 3, 0.6);
    assert!(half > 0 && half < 12);
    assert_eq!(super::loaded_chunks(CHUNK * 4, CHUNK * 3, 1.), (12, 12));
    let void = super::void(CHUNK * 2, CHUNK);
    assert_ne!(void.get(0, 0), void.get(1, 0), "chunk corners are marked");
}

#[test]
fn composite_shows_old_scene_at_start_and_new_scene_at_end() {
    let (width, height) = (CHUNK * 3 + 5, CHUNK * 2);
    let mut from = PixelGrid::new(width, height);
    from.shade(|_, _| Rgb::hex(0x102030));
    let fresh = |_: usize, _: usize| Rgb::hex(0xa0b0c0);

    let mut start = PixelGrid::new(width, height);
    start.shade(fresh);
    composite(&from, &mut start, 0.);
    assert!(start.runs().iter().all(|run| run.color == 0x102030));

    let mut end = PixelGrid::new(width, height);
    end.shade(fresh);
    composite(&from, &mut end, 1.);
    assert!(end.runs().iter().all(|run| run.color == 0xa0b0c0));

    let mut middle = PixelGrid::new(width, height);
    middle.shade(fresh);
    composite(&from, &mut middle, 0.5);
    let colors: std::collections::HashSet<_> = middle.runs().iter().map(|run| run.color).collect();
    assert!(colors.contains(&0xa0b0c0) && colors.len() > 2);
}
