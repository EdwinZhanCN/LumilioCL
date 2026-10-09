//! Structures, strongholds, spawn and slime chunks on top of cubiomes' finders.
use crate::{Dimension, Error, Version, ffi};

/// Structures placed on a region grid, in the order the C bridge indexes them.
/// Treasure, mineshafts, wells, geodes and the End's gateways and islands are
/// decorators with no region grid and are not offered.
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
}

impl Structure {
    pub const ALL: [Structure; 17] = [
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

    /// Positions are right but viability is approximated: since 1.18 these
    /// depend on terrain height, which cubiomes does not compute.
    pub fn estimated(self, version: Version) -> bool {
        matches!(
            self,
            Structure::DesertPyramid | Structure::JungleTemple | Structure::Mansion
        ) && version.at_least("1.18.2")
    }
}

/// A block position, X then Z.
pub type Position = [i32; 2];

/// Largest area edge `structures` accepts, in blocks.
pub const MAX_AREA: i32 = 1 << 17;

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
    // At most one position per region; the smallest region any kind uses is
    // 16 chunks, and the area can straddle one more on each side.
    const MIN_REGION_BLOCKS: i64 = 16 * 16;
    let cells = |lo: i32, hi: i32| (i64::from(hi) - i64::from(lo)) / MIN_REGION_BLOCKS + 2;
    let cap = usize::try_from(cells(x0, x1) * cells(z0, z1)).map_err(|_| Error::InvalidRange)?;
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
    /// Before 1.18 the spawn depends on which block is grass, which cubiomes
    /// can only estimate from biomes.
    pub estimated: bool,
}

pub fn spawn(version: Version, seed: i64) -> Spawn {
    Spawn {
        at: ffi::spawn(version, seed),
        estimated: !version.at_least("1.18.2"),
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
