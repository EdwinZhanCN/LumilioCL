//! Large ore veins of the 1.18+ Overworld (the fork's `veins.c`): where the
//! vein toggle noise runs copper veins (Y 0–50) and iron veins (Y −60 to −8).

use crate::{Error, Version, ffi};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(i32)]
pub enum Vein {
    Copper = 1,
    Iron = 2,
}

impl Vein {
    /// Large veins began in 1.18.
    pub fn available(version: Version) -> bool {
        version.at_least("1.18.2")
    }
}

/// Largest grid edge `vein_zones` accepts, in cells.
pub const MAX_VEIN_CELLS: i32 = 1024;

/// How strongly each `cell`-block square from `origin` lies in a `vein` zone,
/// row-major over `size` = [width, height]: `None` outside any zone, else how
/// far the square's strongest noise corner passes the threshold. `cell` and
/// `origin` are multiples of 4, the spacing of the corners the game
/// interpolates between; a vein can be narrower than a square.
pub fn vein_zones(
    version: Version,
    seed: i64,
    vein: Vein,
    origin: [i32; 2],
    cell: i32,
    size: [i32; 2],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<Option<f32>>, Error> {
    if !Vein::available(version) {
        return Err(Error::Unsupported);
    }
    let [w, h] = size;
    // Squares align with the 4-block noise corners.
    if !(4..=4096).contains(&cell)
        || cell % 4 != 0
        || origin.iter().any(|edge| edge.rem_euclid(4) != 0)
        || !(1..=MAX_VEIN_CELLS).contains(&w)
        || !(1..=MAX_VEIN_CELLS).contains(&h)
        || origin.iter().zip([w, h]).any(|(start, n)| {
            let end = i64::from(*start) + i64::from(n) * i64::from(cell);
            start.unsigned_abs() > 30_000_000 || end.abs() > 30_000_000
        })
    {
        return Err(Error::InvalidRange);
    }
    let mut cancelled = cancelled;
    ffi::vein_zones(
        version,
        seed,
        vein as i32,
        origin,
        cell,
        size,
        &mut cancelled,
    )
    .map(|cells| cells.into_iter().map(|v| (v >= 0.).then_some(v)).collect())
}
