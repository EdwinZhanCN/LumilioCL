//! Coarser tiles built from finer ones, for base maps drawn only at level 0
//! (plan W4): a tile at level `n` is its 4×4 children at level `n - 1`, each
//! shrunk to a quarter of its width.
use lumilio_plugin_api::ImageData;
use lumilio_plugin_api::map::{MAX_UNKNOWN, TILE_PIXELS, TileReply};
use std::collections::BTreeSet;

/// Children per side of a tile one level coarser.
pub const SPLIT: i32 = 4;

/// The tile made of `children`, row by row from the north-west (`z * 4 + x`).
/// A pixel shows the mean colour of the child pixels under it that have data,
/// and has data if any of them does; a tile with no data anywhere is
/// [`TileReply::Empty`]. Block names a child had no colour for are kept.
pub fn compose(children: &[TileReply]) -> TileReply {
    let side = TILE_PIXELS as usize;
    let step = side / SPLIT as usize;
    let mut rgba = vec![0u8; side * side * 4];
    let mut coverage = vec![0u8; side * side];
    let mut unknown = BTreeSet::new();
    let mut complete = true;
    for (index, child) in children.iter().enumerate().take((SPLIT * SPLIT) as usize) {
        let (left, top) = (index % SPLIT as usize * step, index / SPLIT as usize * step);
        let (image, covered): (&ImageData, Option<&[u8]>) = match child {
            TileReply::Empty => {
                complete = false;
                continue;
            }
            TileReply::Image(image) => (image, None),
            TileReply::Partial {
                image,
                coverage,
                unknown: names,
            } => {
                unknown.extend(names.iter().cloned());
                (image, Some(coverage))
            }
        };
        for y in 0..step {
            for x in 0..step {
                let (mut sum, mut count) = ([0u32; 3], 0u32);
                for dy in 0..SPLIT as usize {
                    for dx in 0..SPLIT as usize {
                        let at = (y * SPLIT as usize + dy) * side + x * SPLIT as usize + dx;
                        if covered.is_some_and(|covered| covered[at] == 0) {
                            continue;
                        }
                        for (channel, total) in sum.iter_mut().enumerate() {
                            *total += u32::from(image.rgba[at * 4 + channel]);
                        }
                        count += 1;
                    }
                }
                let out = (top + y) * side + left + x;
                if count == 0 {
                    complete = false;
                    continue;
                }
                if count < (SPLIT * SPLIT) as u32 {
                    complete = false;
                }
                for (channel, total) in sum.iter().enumerate() {
                    rgba[out * 4 + channel] = (total / count) as u8;
                }
                rgba[out * 4 + 3] = 255;
                coverage[out] = 255;
            }
        }
    }
    if coverage.iter().all(|covered| *covered == 0) {
        return TileReply::Empty;
    }
    let image = ImageData {
        width: TILE_PIXELS,
        height: TILE_PIXELS,
        rgba,
    };
    if complete && unknown.is_empty() {
        return TileReply::Image(image);
    }
    TileReply::Partial {
        image,
        coverage,
        unknown: unknown.into_iter().take(MAX_UNKNOWN).collect(),
    }
}
