//! Which item a block shows as in an inventory (plan schematic-studio, T27,
//! D29): the item the studio draws a block's icon from. Most blocks are
//! their own item; the rest (`wall_torch` → `torch`, crops → seeds) live in
//! `data/block_items.json`, because the game keeps that pairing in code,
//! not in its resources. The same file names the flat items the game tints
//! with a biome colour (grass, vines).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Deserialize)]
struct Table {
    /// Block name (one `*` matches any run) → item name, `*` filled in.
    items: BTreeMap<String, String>,
    /// Block name → the colormap its flat item is tinted with.
    tints: BTreeMap<String, String>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/block_items.json"))
            .expect("data/block_items.json is valid")
    })
}

/// `name` matched against `pattern` with at most one `*`: what the `*`
/// covered, or `None` when it does not match.
fn capture<'a>(pattern: &str, name: &'a str) -> Option<&'a str> {
    match pattern.split_once('*') {
        None => (pattern == name).then_some(""),
        Some((prefix, suffix)) => name
            .strip_prefix(prefix)?
            .strip_suffix(suffix)
            .filter(|middle| !middle.is_empty()),
    }
}

/// The item model id block `id` is shown with: `minecraft:wall_torch` →
/// `minecraft:item/torch`. Block-state properties are ignored.
#[must_use]
pub fn icon_item(id: &str) -> String {
    let id = id.split('[').next().unwrap_or(id);
    let (namespace, name) = id.split_once(':').unwrap_or(("minecraft", id));
    let items = &table().items;
    let item = items
        .get(name)
        .cloned()
        .or_else(|| {
            items.iter().find_map(|(pattern, item)| {
                let middle = capture(pattern, name).filter(|_| pattern.contains('*'))?;
                Some(item.replace('*', middle))
            })
        })
        .unwrap_or_else(|| name.to_owned());
    format!("{namespace}:item/{item}")
}

/// The colormap (`grass`, `foliage`) that tints block `id`'s flat item.
#[must_use]
pub fn icon_tint(id: &str) -> Option<&'static str> {
    let id = id.split('[').next().unwrap_or(id);
    let name = id.split_once(':').map_or(id, |(_, name)| name);
    table().tints.get(name).map(String::as_str)
}

/// Every item name the table points at, for the lint against a real pack.
#[must_use]
pub fn table_items() -> Vec<String> {
    table()
        .items
        .iter()
        .filter(|(pattern, _)| !pattern.contains('*'))
        .map(|(_, item)| item.clone())
        .collect()
}
