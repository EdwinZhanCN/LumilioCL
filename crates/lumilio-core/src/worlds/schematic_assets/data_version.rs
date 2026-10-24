//! The data version a schematic was saved with, which decides the game
//! version whose assets draw it (D3).

use crate::nbt;

/// Where each format keeps it: Litematica, Sponge v2 and vanilla structure
/// files at the root, Sponge v3 under `Schematic`.
const PATHS: [&[&str]; 3] = [
    &["MinecraftDataVersion"],
    &["DataVersion"],
    &["Schematic", "DataVersion"],
];

/// `None` for formats without one (MCEdit `.schematic`) and unreadable files.
#[must_use]
pub fn data_version(schematic: &[u8]) -> Option<u32> {
    let root = nbt::parse_maybe_gzip(schematic).ok()?;
    PATHS
        .iter()
        .find_map(|path| root.at(path)?.as_i64())
        .and_then(|version| u32::try_from(version).ok())
        .filter(|version| *version > 0)
}
