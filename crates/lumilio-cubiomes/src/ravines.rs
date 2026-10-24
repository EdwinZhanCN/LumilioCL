//! Ravines of the 1.18+ Overworld (the fork's `carvers.c`): where the canyon
//! carver starts and the path its ellipsoids walk. Both come from the seed
//! alone; which blocks open depends on the terrain the ravine cuts.

use crate::{Error, Version, ffi};

#[derive(Clone, Debug, PartialEq)]
pub struct Ravine {
    /// The start block, x y z.
    pub start: [i32; 3],
    /// The start column lies under an ocean at sea level, so water fills it.
    pub underwater: bool,
    /// Each step's centre, x z, along the walk.
    pub path: Vec<[i32; 2]>,
}

/// Largest area edge `ravines` accepts, in blocks.
pub const MAX_RAVINE_AREA: i32 = 1 << 14;

pub fn ravines_available(version: Version) -> bool {
    version.at_least("1.18.2")
}

/// Ravines whose start chunk lies in the block area `[x0, x1) x [z0, z1)`;
/// a ravine walks up to 112 blocks from it.
pub fn ravines(
    version: Version,
    seed: i64,
    area: [i32; 4],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<Ravine>, Error> {
    let [x0, z0, x1, z1] = area;
    if x1 <= x0
        || z1 <= z0
        || i64::from(x1) - i64::from(x0) > i64::from(MAX_RAVINE_AREA)
        || i64::from(z1) - i64::from(z0) > i64::from(MAX_RAVINE_AREA)
        || area.iter().any(|edge| edge.unsigned_abs() > 30_000_000)
    {
        return Err(Error::InvalidRange);
    }
    if !ravines_available(version) {
        return Err(Error::Unsupported);
    }
    // One start in a hundred chunks; room for every chunk to start one would
    // be wasteful, so the buffer allows ten times the expected number.
    let chunks =
        ((i64::from(x1) - i64::from(x0)) / 16 + 2) * ((i64::from(z1) - i64::from(z0)) / 16 + 2);
    let cap = (chunks / 10 + 8) as usize * (5 + 2 * 112);
    let mut cancelled = cancelled;
    let packed = ffi::ravines(version, seed, area, cap, &mut cancelled)?;
    let mut found = vec![];
    let mut at = 0;
    while at + 5 <= packed.len() {
        let steps = packed[at + 4] as usize;
        let path = packed[at + 5..at + 5 + 2 * steps]
            .as_chunks::<2>()
            .0
            .to_vec();
        found.push(Ravine {
            start: [packed[at], packed[at + 1], packed[at + 2]],
            underwater: packed[at + 3] != 0,
            path,
        });
        at += 5 + 2 * steps;
    }
    Ok(found)
}
