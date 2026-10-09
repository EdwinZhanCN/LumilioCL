//! Safe boundary around the pinned cubiomes source snapshot.
mod ffi;
mod finders;
pub use finders::{
    MAX_AREA, Position, Spawn, Structure, slime_chunks, spawn, strongholds, structures,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Unsupported,
    InvalidRange,
    Generation,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Dimension {
    Nether = -1,
    Overworld = 0,
    End = 1,
}

/// Explicit release identity; arbitrary C enum values cannot enter the API.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    mc: i32,
}

/// Exact release identities implemented by the maintained MCVersion enum.
/// No prefix, nearest-release, snapshot or MC_NEWEST fallback.
pub const SUPPORTED_VERSIONS: &[&str] = &[
    "1.0.0", "1.1", "1.2.5", "1.3.2", "1.4.7", "1.5.2", "1.6.4", "1.7.10", "1.8.9", "1.9.4",
    "1.10.2", "1.11.2", "1.12.2", "1.13.2", "1.14.4", "1.15.2", "1.16.1", "1.16.5", "1.17.1",
    "1.18.2", "1.19.2", "1.19.4", "1.20.6", "1.21.1", "1.21.3", "1.21.4", "1.21.5", "1.21.6",
    "1.21.7", "1.21.8", "1.21.9", "1.21.10", "1.21.11", "26.1", "26.1.1", "26.1.2", "26.2", "26.3",
];

impl Version {
    pub fn from_name(name: &str) -> Result<Self, Error> {
        SUPPORTED_VERSIONS
            .iter()
            .position(|candidate| *candidate == name)
            .map(|index| Self {
                mc: index as i32 + 3,
            })
            .ok_or(Error::Unsupported)
    }

    /// A stable ordinal: larger is a later release.
    pub fn index(self) -> i32 {
        self.mc
    }

    /// Whether this is `name` or a later supported release.
    pub fn at_least(self, name: &str) -> bool {
        SUPPORTED_VERSIONS
            .iter()
            .position(|candidate| *candidate == name)
            .is_some_and(|index| self.mc >= index as i32 + 3)
    }

    /// Exact data-version identities, used only when level.dat lacks a name.
    pub fn from_data_version(data_version: i32) -> Result<Self, Error> {
        let name = match data_version {
            2586 => "1.16.5",
            2975 => "1.18.2",
            4189 => "1.21.4",
            4325 => "1.21.5",
            4435 => "1.21.6",
            4438 => "1.21.7",
            4440 => "1.21.8",
            4554 => "1.21.9",
            4556 => "1.21.10",
            4671 => "1.21.11",
            4786 => "26.1",
            4788 => "26.1.1",
            4790 => "26.1.2",
            4903 => "26.2",
            5023 => "26.3",
            _ => return Err(Error::Unsupported),
        };
        Self::from_name(name)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Range {
    pub scale: i32,
    pub x: i32,
    pub z: i32,
    pub width: i32,
    pub height: i32,
}

pub fn biomes(
    version: Version,
    seed: i64,
    dimension: Dimension,
    range: Range,
) -> Result<Vec<i32>, Error> {
    biomes_until(version, seed, dimension, range, || false)
}

/// Like [`biomes`], polling `cancelled` between bands of rows; a true answer
/// stops generation with [`Error::Cancelled`].
pub fn biomes_until(
    version: Version,
    seed: i64,
    dimension: Dimension,
    range: Range,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<i32>, Error> {
    if ![1, 4, 16, 64, 256].contains(&range.scale)
        || !(1..=256).contains(&range.width)
        || !(1..=256).contains(&range.height)
    {
        return Err(Error::InvalidRange);
    }
    for (start, size) in [(range.x, range.width), (range.z, range.height)] {
        // Conservative world border, plus headroom for C's sampling halos.
        let first = i64::from(start) * i64::from(range.scale);
        let end = (i64::from(start) + i64::from(size)) * i64::from(range.scale);
        if first < -30_000_000 || end > 30_000_000 {
            return Err(Error::InvalidRange);
        }
    }
    ffi::generate(version, seed, dimension, range, &mut cancelled)
}

pub fn biome_colors() -> [[u8; 3]; 256] {
    ffi::colors()
}

/// A build/link probe, before the generation API is added.
pub fn probe() -> i32 {
    ffi::probe()
}

#[cfg(test)]
mod tests;
