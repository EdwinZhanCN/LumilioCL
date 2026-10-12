//! Structure chests whose loot the seed alone decides (the fork's `loot.c`).
use crate::{Position, Version, ffi};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Chest {
    /// The chest's column; its height follows the ground.
    pub at: Position,
    pub enchanted_golden_apples: u8,
}

/// Whether the release has desert pyramid loot data (1.21 and later, where
/// enchantments are data-driven).
pub fn desert_pyramid_loot_available(version: Version) -> bool {
    ffi::pyramid_loot_available(version.mc)
}

/// The four chests of the desert pyramid a structure query found at `at`,
/// with what their loot holds. Empty for releases without loot data (before
/// 1.21). Whether the pyramid itself generates is the structure layer's
/// estimate.
pub fn desert_pyramid_chests(version: Version, seed: i64, at: Position) -> Vec<Chest> {
    ffi::pyramid_chests(version, seed, at)
        .into_iter()
        .map(|(at, _, apples)| Chest {
            at,
            enchanted_golden_apples: apples.clamp(0, 255) as u8,
        })
        .collect()
}
