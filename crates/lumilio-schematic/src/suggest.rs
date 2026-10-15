//! A replacement to propose for a block the target version does not have
//! (plan schematic-studio, T37): the convert dialog offers it, and the
//! person accepts or edits it in the replacement table.
//!
//! The rules are plain name heuristics over the block IDs the target knows,
//! tried in order:
//!
//! 1. the same path in `minecraft:` (a mod's copy of a vanilla block);
//! 2. the block without a variant prefix (`deepslate_`, `waxed_`,
//!    `polished_` …): `deepslate_iron_ore` → `iron_ore`;
//! 3. a block of the same shape (`_stairs`, `_slab`, `_wall` …) whose
//!    material shares the most words, else a plain default material
//!    (`oak_`, `stone_`): `cherry_stairs` → `oak_stairs`;
//! 4. without a shape, the block sharing the most words, its last word
//!    first: `cherry_leaves` → `oak_leaves`.
//!
//! Vanilla `minecraft:` blocks win every tie. No candidate gives `None`.

use std::collections::BTreeSet;

/// Prefixes a variant of a block adds to its name, outermost first.
const VARIANT_PREFIXES: &[&str] = &[
    "waxed_",
    "exposed_",
    "weathered_",
    "oxidized_",
    "deepslate_",
    "polished_",
    "cracked_",
    "mossy_",
    "chiseled_",
    "smooth_",
    "cut_",
    "infested_",
    "stripped_",
    "potted_",
];

/// Shape suffixes, longest first so `_wall_sign` is not read as `_sign`.
const SHAPES: &[&str] = &[
    "_wall_hanging_sign",
    "_hanging_sign",
    "_pressure_plate",
    "_wall_sign",
    "_fence_gate",
    "_trapdoor",
    "_stairs",
    "_button",
    "_sapling",
    "_leaves",
    "_planks",
    "_fence",
    "_slab",
    "_wall",
    "_door",
    "_sign",
    "_log",
    "_wood",
];

/// Materials a shape falls back to when no word is shared, best first.
const PLAIN: &[&str] = &["oak", "stone", "cobblestone", "stone_brick"];

/// A replacement for `missing` among `known` (the target's block IDs), or
/// `None` when no rule finds one. `missing` itself is never proposed.
pub fn suggest<'a>(missing: &str, known: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let known: BTreeSet<&str> = known.into_iter().filter(|id| *id != missing).collect();
    let (namespace, path) = split(missing);
    let has = |path: &str| {
        let vanilla = format!("minecraft:{path}");
        if known.contains(vanilla.as_str()) {
            return Some(vanilla);
        }
        let own = format!("{namespace}:{path}");
        known.contains(own.as_str()).then_some(own)
    };
    if namespace != "minecraft"
        && let Some(found) = has(path)
    {
        return Some(found);
    }
    let mut stripped = path;
    while let Some(rest) = VARIANT_PREFIXES
        .iter()
        .find_map(|prefix| stripped.strip_prefix(prefix))
    {
        stripped = rest;
        if let Some(found) = has(stripped) {
            return Some(found);
        }
    }
    match SHAPES.iter().find(|shape| path.ends_with(*shape)) {
        Some(shape) => same_shape(path, shape, &known),
        None => most_words(path, &known),
    }
}

/// `namespace:path`, with `minecraft` for a bare path.
fn split(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

fn words(text: &str) -> BTreeSet<&str> {
    text.split('_').filter(|word| !word.is_empty()).collect()
}

/// The best block of `shape` for `path`'s material.
fn same_shape(path: &str, shape: &str, known: &BTreeSet<&str>) -> Option<String> {
    let material = words(&path[..path.len() - shape.len()]);
    known
        .iter()
        .filter_map(|id| {
            let (namespace, candidate) = split(id);
            // `_wall_sign` is not a `_sign` shape's match, nor the reverse.
            let shape_of = SHAPES.iter().find(|known| candidate.ends_with(*known))?;
            if shape_of != &shape {
                return None;
            }
            let base = &candidate[..candidate.len() - shape.len()];
            let shared = words(base).intersection(&material).count();
            // A plain material only decides when no word is shared.
            let plain = PLAIN
                .iter()
                .position(|plain| *plain == base)
                .filter(|_| shared == 0)
                .map_or(0, |at| PLAIN.len() - at);
            let rank = (
                shared,
                namespace == "minecraft",
                plain,
                std::cmp::Reverse(id.len()),
            );
            Some((rank, *id))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)))
        .map(|(_, id)| id.to_owned())
}

/// The block sharing the most words with `path`, at least one; its last
/// word counts double.
fn most_words(path: &str, known: &BTreeSet<&str>) -> Option<String> {
    let mine = words(path);
    let last = path.rsplit('_').next().unwrap_or(path);
    known
        .iter()
        .filter_map(|id| {
            let (namespace, candidate) = split(id);
            let shared = words(candidate).intersection(&mine).count();
            if shared == 0 {
                return None;
            }
            let same_last = candidate.rsplit('_').next() == Some(last);
            let rank = (
                shared + usize::from(same_last),
                namespace == "minecraft",
                std::cmp::Reverse(id.len()),
            );
            Some((rank, *id))
        })
        .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(a.1)))
        .map(|(_, id)| id.to_owned())
}
