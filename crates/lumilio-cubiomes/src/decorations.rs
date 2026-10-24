//! Decoration features: per chunk, the decoration random runs a placed
//! feature's placement modifiers (the fork's `decorations.c`, from per-release
//! tables of the original game data).

use crate::{Dimension, Error, Version, ffi};

/// Placed features the fork tables, in its `DecorationType` order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(i32)]
pub enum Decoration {
    /// `fossil_upper`: bones with coal, under deserts and swamps.
    Fossil,
    /// `fossil_lower`: deep bones with diamonds, under the same biomes.
    DiamondFossil,
    /// `lake_lava_surface`: a lava lake on the surface.
    LavaLake,
    /// `lake_lava_underground`.
    UndergroundLavaLake,
    /// `monster_room`.
    MonsterRoom,
    /// `monster_room_deep`.
    DeepMonsterRoom,
    /// `rooted_sulfur_spring` (26.2+).
    SulfurSpring,
    /// `amethyst_geode`.
    Geode,
    /// `desert_well`.
    DesertWell,
    /// `end_island_decorated`.
    EndIsland,
    /// `end_gateway_return`.
    EndGateway,
}

impl Decoration {
    pub const ALL: [Decoration; 11] = [
        Decoration::Fossil,
        Decoration::DiamondFossil,
        Decoration::LavaLake,
        Decoration::UndergroundLavaLake,
        Decoration::MonsterRoom,
        Decoration::DeepMonsterRoom,
        Decoration::SulfurSpring,
        Decoration::Geode,
        Decoration::DesertWell,
        Decoration::EndIsland,
        Decoration::EndGateway,
    ];

    /// Whether the release's table places this feature in the dimension.
    /// Releases without original-server tables have none.
    pub fn available(self, version: Version, dimension: Dimension) -> bool {
        ffi::decoration_available(version.mc, dimension as i32, self as i32)
    }
}

/// One candidate that passed the modifiers the fork evaluates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placed {
    /// x, y, z; y is meaningless when `surface` is set.
    pub at: [i32; 3],
    /// Placed on a surface height that is not computed.
    pub surface: bool,
    /// A block or height filter, not evaluated, decides placement.
    pub terrain: bool,
    /// An earlier candidate of the chunk was placed first and drew an unknown
    /// number of random numbers, so this position is not known.
    pub after: bool,
}

/// Largest area edge `decorations` accepts, in blocks.
pub const MAX_DECORATION_AREA: i32 = 1 << 14;

/// Candidates of `kind` whose column lies in `[x0, x1) x [z0, z1)`, with the
/// biome filter applied.
pub fn decorations(
    version: Version,
    seed: i64,
    dimension: Dimension,
    kind: Decoration,
    area: [i32; 4],
    cancelled: impl FnMut() -> bool,
) -> Result<Vec<Placed>, Error> {
    let [x0, z0, x1, z1] = area;
    if x1 <= x0
        || z1 <= z0
        || i64::from(x1) - i64::from(x0) > i64::from(MAX_DECORATION_AREA)
        || i64::from(z1) - i64::from(z0) > i64::from(MAX_DECORATION_AREA)
        || [x0, x1, z0, z1]
            .iter()
            .any(|edge| edge.unsigned_abs() > 30_000_000)
    {
        return Err(Error::InvalidRange);
    }
    if !kind.available(version, dimension) {
        return Err(Error::Unsupported);
    }
    let chunks = |lo: i32, hi: i32| (i64::from(hi) - i64::from(lo)) / 16 + 2;
    // Every modelled feature yields at most 16 candidates per chunk.
    let cap = (chunks(x0, x1) * chunks(z0, z1) * 16) as usize;
    let mut cancelled = cancelled;
    ffi::decorations(
        version,
        seed,
        dimension,
        kind as i32,
        area,
        cap,
        &mut cancelled,
    )
    .map(|found| {
        found
            .into_iter()
            .map(|[x, y, z, flags]| Placed {
                at: [x, y, z],
                surface: flags & 1 != 0,
                terrain: flags & 2 != 0,
                after: flags & 4 != 0,
            })
            .collect()
    })
}
