//! The "chunk loading" transition between two slides.
//!
//! The incoming scene streams in 16×16-texel chunks, ring by ring outward
//! from the centre chunk, the way a world loads around a player. While a chunk
//! is loading its texels dissolve in and its border is outlined like the debug
//! chunk-border overlay; the rest of the screen still shows the old scene.

use super::noise::{hash2, smoothstep};
use super::raster::{PixelGrid, Rgb};

pub const CHUNK: usize = 16;
/// Share of the transition over which chunk start times are spread; each
/// chunk then spends the remainder loading.
const STAGGER: f32 = 0.7;
const BORDER: Rgb = Rgb::hex(0x9fe8ff);

/// Load rank of every chunk, row-major. Rank 0 is the centre chunk; chunks in
/// the same ring load clockwise starting from the top.
pub fn chunk_ranks(columns: usize, rows: usize) -> Vec<usize> {
    let (cx, cy) = ((columns as f32 - 1.) / 2., (rows as f32 - 1.) / 2.);
    let mut order: Vec<(usize, f32, f32)> = (0..columns * rows)
        .map(|index| {
            let dx = (index % columns) as f32 - cx;
            let dy = (index / columns) as f32 - cy;
            let ring = dx.abs().max(dy.abs());
            // Clockwise from 12 o'clock.
            let angle = dx.atan2(-dy).rem_euclid(std::f32::consts::TAU);
            (index, ring, angle)
        })
        .collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)));

    let mut ranks = vec![0; columns * rows];
    for (rank, (index, _, _)) in order.into_iter().enumerate() {
        ranks[index] = rank;
    }
    ranks
}

/// Loading phase in `[0, 1]` for a chunk of the given rank.
pub fn chunk_phase(rank: usize, chunks: usize, progress: f32) -> f32 {
    let start = if chunks <= 1 {
        0.
    } else {
        rank as f32 / (chunks - 1) as f32 * STAGGER
    };
    ((progress - start) / (1. - STAGGER)).clamp(0., 1.)
}

/// The not-yet-loaded world: near-black, with a faint marker at every chunk
/// corner so the grid that is about to fill is legible.
pub fn void(width: usize, height: usize) -> PixelGrid {
    let mut grid = PixelGrid::new(width, height);
    grid.shade(|x, y| {
        let depth = y as f32 / height.max(1) as f32;
        let base = Rgb::hex(0x0c0e14).lerp(Rgb::hex(0x141824), depth);
        if x % CHUNK == 0 && y % CHUNK == 0 {
            Rgb::hex(0x2a3244)
        } else {
            base
        }
    });
    grid
}

/// Chunks fully loaded at this progress, and the total, for a texel grid.
pub fn loaded_chunks(width: usize, height: usize, progress: f32) -> (usize, usize) {
    let columns = width.div_ceil(CHUNK);
    let rows = height.div_ceil(CHUNK);
    let chunks = columns * rows;
    let loaded = (0..chunks)
        .filter(|&rank| chunk_phase(rank, chunks, progress) >= 1.)
        .count();
    (loaded, chunks)
}

/// Composites `from` → `to` into `to` at the given transition progress.
pub fn composite(from: &PixelGrid, to: &mut PixelGrid, progress: f32) {
    debug_assert_eq!(
        (from.width(), from.height()),
        (to.width(), to.height()),
        "both scenes rasterise at the same resolution"
    );
    let columns = to.width().div_ceil(CHUNK);
    let rows = to.height().div_ceil(CHUNK);
    let ranks = chunk_ranks(columns, rows);
    let chunks = ranks.len();

    for y in 0..to.height() {
        for x in 0..to.width() {
            let chunk = (y / CHUNK) * columns + x / CHUNK;
            let phase = chunk_phase(ranks[chunk], chunks, progress);
            if phase >= 1. {
                continue;
            }
            let old = from.get(x, y);
            if phase <= 0. {
                to.set(x, y, old);
                continue;
            }
            let (lx, ly) = (x % CHUNK, y % CHUNK);
            let edge = lx == 0 || ly == 0 || lx == CHUNK - 1 || ly == CHUNK - 1;
            let arrived = hash2(x as i32, y as i32, 0xc4a1) < smoothstep(0., 0.85, phase);
            let texel = if arrived {
                to.get(x, y)
            } else {
                old.scale(1. - 0.55 * phase)
            };
            let outline = (1. - phase) * 0.55;
            to.set(
                x,
                y,
                if edge {
                    texel.lerp(BORDER, outline)
                } else {
                    texel
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
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
        let colors: std::collections::HashSet<_> =
            middle.runs().iter().map(|run| run.color).collect();
        assert!(colors.contains(&0xa0b0c0) && colors.len() > 2);
    }
}
