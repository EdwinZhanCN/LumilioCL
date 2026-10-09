//! Structures, strongholds, spawn and slime chunks on top of cubiomes' finders.
use crate::{Dimension, Error, Version, ffi};

/// Structures placed on a region grid, in the order the C bridge indexes them.
/// The decorators (buried treasure, mineshafts, wells, geodes and End gateways)
/// have a one-chunk region rather than a coarse one; the End's islands are left
/// out because the map has no icon for them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(i32)]
pub enum Structure {
    DesertPyramid,
    JungleTemple,
    SwampHut,
    Igloo,
    Village,
    OceanRuin,
    Shipwreck,
    Monument,
    Mansion,
    Outpost,
    RuinedPortal,
    AncientCity,
    TrailRuins,
    TrialChambers,
    Fortress,
    Bastion,
    EndCity,
    /// Surface-projected jigsaw; positions are candidates, viability estimated.
    AbandonedCamp,
    /// One attempt per chunk, one in a hundred of them survives; a beach find.
    Treasure,
    /// A mineshaft start per chunk, so it is dense and off by default.
    Mineshaft,
    /// One attempt per chunk, one in a thousand.
    DesertWell,
    /// One attempt per chunk, about one in twenty-four.
    Geode,
    /// One attempt per chunk in the End.
    EndGateway,
}

impl Structure {
    pub const ALL: [Structure; 23] = [
        Structure::DesertPyramid,
        Structure::JungleTemple,
        Structure::SwampHut,
        Structure::Igloo,
        Structure::Village,
        Structure::OceanRuin,
        Structure::Shipwreck,
        Structure::Monument,
        Structure::Mansion,
        Structure::Outpost,
        Structure::RuinedPortal,
        Structure::AncientCity,
        Structure::TrailRuins,
        Structure::TrialChambers,
        Structure::Fortress,
        Structure::Bastion,
        Structure::EndCity,
        Structure::AbandonedCamp,
        Structure::Treasure,
        Structure::Mineshaft,
        Structure::DesertWell,
        Structure::Geode,
        Structure::EndGateway,
    ];

    /// Whether this version generates the kind in the dimension, and cubiomes
    /// can place it there.
    pub fn available(self, version: Version, dimension: Dimension) -> bool {
        ffi::structure_available(version.mc, dimension as i32, self as i32)
    }

    /// Kinds offered for a version and dimension.
    pub fn in_dimension(version: Version, dimension: Dimension) -> Vec<Structure> {
        Self::ALL
            .into_iter()
            .filter(|kind| kind.available(version, dimension))
            .collect()
    }

    /// Region candidates are exact; generation and template offsets depend
    /// on terrain height, which cubiomes does not compute.
    pub fn estimated(self, version: Version) -> bool {
        self == Structure::AbandonedCamp
            || matches!(
                self,
                Structure::DesertPyramid | Structure::JungleTemple | Structure::Mansion
            ) && version.at_least("1.18.2")
    }
}

/// A block position, X then Z.
pub type Position = [i32; 2];

/// Largest area edge `structures` accepts, in blocks.
pub const MAX_AREA: i32 = 1 << 17;

/// Largest number of region cells a query may span, so the output buffer stays
/// bounded even for the one-chunk decorators.
const MAX_CELLS: i64 = 1 << 20;

/// Viable positions of `kind` in the block area `[x0, x1) x [z0, z1)`.
pub fn structures(
    version: Version,
    seed: i64,
    dimension: Dimension,
    kind: Structure,
    area: [i32; 4],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<Position>, Error> {
    let [x0, z0, x1, z1] = area;
    if x1 <= x0
        || z1 <= z0
        || i64::from(x1) - i64::from(x0) > i64::from(MAX_AREA)
        || i64::from(z1) - i64::from(z0) > i64::from(MAX_AREA)
        || [x0, x1, z0, z1]
            .iter()
            .any(|edge| edge.unsigned_abs() > 30_000_000)
    {
        return Err(Error::InvalidRange);
    }
    if !kind.available(version, dimension) {
        return Err(Error::Unsupported);
    }
    // At most one position per region. The region edge is a kind property:
    // the coarse kinds use hundreds of blocks, the decorators a single chunk,
    // so the buffer follows the kind instead of the smallest upstream region.
    let region = i64::from(ffi::region_blocks(
        version.mc,
        dimension as i32,
        kind as i32,
    ));
    if region <= 0 {
        return Err(Error::Unsupported);
    }
    let cells = |lo: i32, hi: i32| (i64::from(hi) - i64::from(lo)) / region + 2;
    let cap = cells(x0, x1)
        .checked_mul(cells(z0, z1))
        .filter(|count| *count <= MAX_CELLS)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(Error::InvalidRange)?;
    let mut cancelled = cancelled;
    ffi::structures(
        version,
        seed,
        dimension,
        kind as i32,
        area,
        cap,
        &mut cancelled,
    )
}

/// The first `limit` strongholds in generation order (at most 128).
pub fn strongholds(
    version: Version,
    seed: i64,
    limit: usize,
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<Position>, Error> {
    let mut cancelled = cancelled;
    ffi::strongholds(version, seed, limit.min(128), &mut cancelled)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spawn {
    pub at: Position,
    /// Spawn depends on grass and terrain height; cubiomes approximates both.
    pub estimated: bool,
}

pub fn spawn(version: Version, seed: i64) -> Spawn {
    Spawn {
        at: ffi::spawn(version, seed),
        estimated: true,
    }
}

/// Slime chunks in a chunk rectangle, row-major (`true` for a slime chunk).
/// Valid for the Overworld in every supported version.
pub fn slime_chunks(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    width: i32,
    height: i32,
) -> Result<Vec<bool>, Error> {
    if !(1..=1024).contains(&width)
        || !(1..=1024).contains(&height)
        || [chunk_x, chunk_z]
            .iter()
            .any(|c| c.unsigned_abs() > 2_000_000)
    {
        return Err(Error::InvalidRange);
    }
    Ok(ffi::slime(seed, chunk_x, chunk_z, width, height))
}
