//! Worn armor: which texture an armor item draws with, and pose angles.
//!
//! Material names and texture paths follow schematic-mesher's original
//! `armor_stand.rs` (AGPL-3.0-only, Schem-at); the armor geometry is the
//! game's own layers chosen by the binding table (LumilioCL local change,
//! plan schematic-studio T7).

/// The armor slots in the order they are drawn, with the property naming
/// each slot's item.
pub const SLOTS: [&str; 4] = ["helmet", "chestplate", "leggings", "boots"];

/// The texture an armor `item` (`minecraft:iron_helmet`) draws with in `slot`.
pub fn texture(item: &str, slot: &str) -> Option<String> {
    let name = item.strip_prefix("minecraft:").unwrap_or(item);
    let material = [
        ("leather_", "leather"),
        ("chainmail_", "chainmail"),
        ("iron_", "iron"),
        ("golden_", "gold"),
        ("diamond_", "diamond"),
        ("netherite_", "netherite"),
        ("copper_", "copper"),
        ("turtle_", "turtle_scute"),
    ]
    .into_iter()
    .find_map(|(prefix, material)| name.starts_with(prefix).then_some(material))?;
    let sheet = if slot == "leggings" {
        "humanoid_leggings"
    } else {
        "humanoid"
    };
    Some(format!("entity/equipment/{sheet}/{material}"))
}

/// A pose property, `"x,y,z"` in degrees.
pub fn parse_angles(text: &str) -> [f32; 3] {
    let mut angles = [0.0; 3];
    for (slot, part) in angles.iter_mut().zip(text.split(',')) {
        *slot = part.trim().parse().unwrap_or(0.0);
    }
    angles
}
