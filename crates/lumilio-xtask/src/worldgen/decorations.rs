//! Decoration features the fork places from a per-release table.
//!
//! `extract` reads one release's original data and the probe's decoration
//! order and writes `forks/cubiomes/data/<release>/decorations.json`: for each
//! tracked placed feature, its decoration step and index (the original
//! server's `FeatureSorter`), its placement modifiers and the biomes that list
//! it. `table` turns every release's file into `forks/cubiomes/tables/decorations.h`.
use crate::release::Result;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

/// Tracked placed features, in the order of the fork's `DecorationType`.
pub(super) const TRACKED: &[(&str, &str, &str)] = &[
    ("fossil_upper", "overworld", "Deco_Fossil_Upper"),
    ("fossil_lower", "overworld", "Deco_Fossil_Lower"),
    ("lake_lava_surface", "overworld", "Deco_Lava_Lake_Surface"),
    (
        "lake_lava_underground",
        "overworld",
        "Deco_Lava_Lake_Underground",
    ),
    ("monster_room", "overworld", "Deco_Monster_Room"),
    ("monster_room_deep", "overworld", "Deco_Monster_Room_Deep"),
    ("rooted_sulfur_spring", "overworld", "Deco_Sulfur_Spring"),
    ("amethyst_geode", "overworld", "Deco_Geode"),
    ("desert_well", "overworld", "Deco_Desert_Well"),
    ("end_island_decorated", "end", "Deco_End_Island"),
    ("end_gateway_return", "end", "Deco_End_Gateway"),
];

/// The generator bounds a placement context uses in each dimension.
fn bounds(data: &Value, dimension: &str) -> Result<Value> {
    let settings =
        &data[format!("data/minecraft/worldgen/noise_settings/{dimension}.json")]["noise"];
    let number = |key: &str| settings[key].as_i64().ok_or("missing noise bounds");
    Ok(json!({"min_y": number("min_y")?, "height": number("height")?}))
}

/// Biomes whose generation settings list `feature` in any step.
fn biomes(data: &Value, feature: &str) -> Result<Vec<String>> {
    let mut found = vec![];
    for (path, biome) in data.as_object().ok_or("invalid worldgen data")? {
        let Some(name) = path
            .strip_prefix("data/minecraft/worldgen/biome/")
            .and_then(|n| n.strip_suffix(".json"))
        else {
            continue;
        };
        let steps = biome["features"]
            .as_array()
            .ok_or_else(|| format!("biome {name} without feature steps"))?;
        let mut listed = false;
        for step in steps {
            let step = step
                .as_array()
                .ok_or_else(|| format!("biome {name}: feature tags are not supported"))?;
            listed |= step.iter().any(|f| f.as_str() == Some(feature));
        }
        if listed {
            found.push(name.to_owned());
        }
    }
    found.sort();
    Ok(found)
}

pub(super) fn extract(order: &Value, data: &Value) -> Result<Value> {
    let mut features = vec![];
    for &(name, dimension, _) in TRACKED {
        let id = format!("minecraft:{name}");
        let steps = order[dimension]
            .as_array()
            .ok_or("probe output without decoration order")?;
        let Some((step, index)) = steps.iter().enumerate().find_map(|(step, list)| {
            list.as_array()?
                .iter()
                .position(|f| f.as_str() == Some(&id))
                .map(|index| (step, index))
        }) else {
            continue;
        };
        let placed = &data[format!("data/minecraft/worldgen/placed_feature/{name}.json")];
        if placed.is_null() {
            return Err(format!("{name} is ordered but has no placed feature"));
        }
        features.push(json!({
            "name": name,
            "dimension": dimension,
            "step": step,
            "index": index,
            "placement": placed["placement"],
            "biomes": biomes(data, &id)?,
        }));
    }
    let mut dimensions = serde_json::Map::new();
    for dimension in ["overworld", "nether", "end"] {
        dimensions.insert(dimension.into(), bounds(data, dimension)?);
    }
    Ok(json!({"dimensions": dimensions, "features": features}))
}

/// A vertical anchor as (kind, value): 0 absolute, 1 above bottom, 2 below top.
fn anchor(value: &Value) -> Result<(i64, i64)> {
    for (kind, key) in [(0, "absolute"), (1, "above_bottom"), (2, "below_top")] {
        if let Some(v) = value[key].as_i64() {
            return Ok((kind, v));
        }
    }
    Err(format!("unsupported vertical anchor {value}"))
}

/// An int provider as (min, max, draws): a constant draws nothing.
fn int_provider(value: &Value) -> Result<(i64, i64, bool)> {
    if let Some(n) = value.as_i64() {
        return Ok((n, n, false));
    }
    match value["type"].as_str() {
        Some("minecraft:constant") => {
            let n = value["value"].as_i64().ok_or("invalid constant")?;
            Ok((n, n, false))
        }
        Some("minecraft:uniform") => {
            // 26.x inlines the bounds; earlier releases nest them in `value`.
            let bounds = if value["min_inclusive"].is_null() {
                &value["value"]
            } else {
                value
            };
            let number = |key: &str| bounds[key].as_i64().ok_or("invalid uniform int");
            Ok((number("min_inclusive")?, number("max_inclusive")?, true))
        }
        _ => Err(format!("unsupported int provider {value}")),
    }
}

/// One placement modifier as a C `DecoOp` initialiser.
fn op(modifier: &Value) -> Result<String> {
    let kind = modifier["type"].as_str().ok_or("modifier without type")?;
    Ok(match kind {
        "minecraft:rarity_filter" => {
            let chance = modifier["chance"].as_i64().ok_or("invalid rarity")?;
            format!("{{DP_RARITY, {chance}, 0, 0, 0}}")
        }
        "minecraft:in_square" => "{DP_IN_SQUARE, 0, 0, 0, 0}".into(),
        "minecraft:count" => {
            let count = &modifier["count"];
            if count["type"].as_str() == Some("minecraft:weighted_list") {
                let entries = count["distribution"]
                    .as_array()
                    .filter(|list| (1..=2).contains(&list.len()))
                    .ok_or("weighted counts support one or two entries")?;
                let mut args = vec![];
                for entry in entries {
                    args.push(entry["data"].as_i64().ok_or("invalid weighted count")?);
                    args.push(entry["weight"].as_i64().ok_or("invalid count weight")?);
                }
                args.resize(4, 0);
                format!(
                    "{{DP_COUNT_WEIGHTED, {}, {}, {}, {}}}",
                    args[0], args[1], args[2], args[3]
                )
            } else {
                let (min, max, draws) = int_provider(count)?;
                format!("{{DP_COUNT, {min}, {max}, {}, 0}}", u8::from(draws))
            }
        }
        "minecraft:height_range" => {
            let height = &modifier["height"];
            if height["type"].as_str() != Some("minecraft:uniform") {
                return Err(format!("unsupported height provider {height}"));
            }
            let (min_kind, min) = anchor(&height["min_inclusive"])?;
            let (max_kind, max) = anchor(&height["max_inclusive"])?;
            format!("{{DP_HEIGHT_UNIFORM, {min_kind}, {min}, {max_kind}, {max}}}")
        }
        "minecraft:offset" | "minecraft:random_offset" => {
            let axes: [&Value; 3] = if kind == "minecraft:offset" {
                [&modifier["x"], &modifier["y"], &modifier["z"]]
            } else {
                [
                    &modifier["xz_spread"],
                    &modifier["y_spread"],
                    &modifier["xz_spread"],
                ]
            };
            let mut ops = vec![];
            for (axis, value) in axes.into_iter().enumerate() {
                let (min, max, draws) = int_provider(value)?;
                ops.push(format!(
                    "{{DP_OFFSET, {axis}, {min}, {max}, {}}}",
                    u8::from(draws)
                ));
            }
            ops.join(", ")
        }
        "minecraft:heightmap" => "{DP_HEIGHTMAP, 0, 0, 0, 0}".into(),
        "minecraft:biome" => "{DP_BIOME, 0, 0, 0, 0}".into(),
        // Filters that read blocks or heights and draw no random numbers.
        "minecraft:environment_scan"
        | "minecraft:surface_relative_threshold_filter"
        | "minecraft:block_predicate_filter" => "{DP_TERRAIN, 0, 0, 0, 0}".into(),
        _ => return Err(format!("unsupported placement modifier {kind}")),
    })
}

/// `"1.21.10"` → `MC_1_21_10`.
fn mc(version: &str) -> String {
    format!("MC_{}", version.replace('.', "_"))
}

fn dimension(name: &str) -> Result<&'static str> {
    Ok(match name {
        "overworld" => "DIM_OVERWORLD",
        "nether" => "DIM_NETHER",
        "end" => "DIM_END",
        _ => return Err(format!("unknown dimension {name}")),
    })
}

/// The C table for every release with a `decorations.json`, ordered by release.
pub(super) fn table(releases: &[(String, Value)], ids: &Value) -> Result<String> {
    let mut ops: BTreeMap<String, usize> = BTreeMap::new();
    let mut sets: BTreeMap<[u64; 4], usize> = BTreeMap::new();
    let mut rows = String::new();
    for (version, file) in releases {
        let bounds = &file["dimensions"];
        for feature in file["features"].as_array().ok_or("invalid decorations")? {
            let name = feature["name"].as_str().ok_or("feature without name")?;
            let &(_, _, enumerator) = TRACKED
                .iter()
                .find(|(tracked, ..)| *tracked == name)
                .ok_or_else(|| format!("untracked feature {name}"))?;
            let dim = feature["dimension"].as_str().ok_or("feature dimension")?;
            let mut list = vec![];
            for modifier in feature["placement"].as_array().ok_or("placement list")? {
                list.push(op(modifier).map_err(|e| format!("{version} {name}: {e}"))?);
            }
            list.push("{DP_END, 0, 0, 0, 0}".into());
            let program = list.join(", ");
            let next = ops.len();
            let program = *ops.entry(program).or_insert(next);
            let mut set = [0_u64; 4];
            for biome in feature["biomes"].as_array().ok_or("biome list")? {
                let biome = biome.as_str().ok_or("biome name")?;
                let id = ids[biome]
                    .as_u64()
                    .filter(|id| *id < 256)
                    .ok_or_else(|| format!("unknown biome {biome}"))?;
                set[(id / 64) as usize] |= 1 << (id % 64);
            }
            let next = sets.len();
            let set = *sets.entry(set).or_insert(next);
            let number = |key: &str| feature[key].as_i64().ok_or("feature order");
            let salt = number("step")? * 10000 + number("index")?;
            let bound = |key: &str| bounds[dim][key].as_i64().ok_or("dimension bounds");
            writeln!(
                rows,
                "    {{ {}, {}, {enumerator}, {salt}, {}, {}, deco_ops_{program}, deco_biomes_{set} }},",
                mc(version),
                dimension(dim)?,
                bound("min_y")?,
                bound("height")?,
            )
            .unwrap();
        }
    }
    let mut out = String::from(
        "/* Generated by `cargo xtask cubiomes-decorations` from\n * forks/cubiomes/data/<release>/decorations.json. Do not edit.\n * Derived from original Minecraft data; see forks/cubiomes/data/README.md. */\n",
    );
    let mut ordered: Vec<_> = ops.into_iter().collect();
    ordered.sort_by_key(|(_, index)| *index);
    for (program, index) in ordered {
        writeln!(
            out,
            "static const DecoOp deco_ops_{index}[] = {{ {program} }};"
        )
        .unwrap();
    }
    let mut ordered: Vec<_> = sets.into_iter().collect();
    ordered.sort_by_key(|(_, index)| *index);
    for (set, index) in ordered {
        writeln!(
            out,
            "static const uint64_t deco_biomes_{index}[4] = {{ 0x{:016x}ULL, 0x{:016x}ULL, 0x{:016x}ULL, 0x{:016x}ULL }};",
            set[0], set[1], set[2], set[3]
        )
        .unwrap();
    }
    writeln!(out, "static const DecoEntry DECORATIONS[] = {{\n{rows}}};").unwrap();
    Ok(out)
}

/// Rewrites `forks/cubiomes/tables/decorations.h` from the committed data.
pub(super) fn generate(root: &Path) -> Result {
    std::fs::write(
        root.join("forks/cubiomes/tables/decorations.h"),
        render(root)?,
    )
    .map_err(|e| e.to_string())
}

/// The table every committed `decorations.json` produces.
pub(super) fn render(root: &Path) -> Result<String> {
    let data = root.join("forks/cubiomes/data");
    let mut releases = vec![];
    for entry in std::fs::read_dir(&data).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let file = path.join("decorations.json");
        if !file.exists() {
            continue;
        }
        let version = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("invalid release directory")?
            .to_owned();
        releases.push((version, super::prepare::json(&file)?));
    }
    releases.sort_by_key(|(version, _)| {
        version
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or(0))
            .collect::<Vec<_>>()
    });
    let ids: Value =
        serde_json::from_str(include_str!("biome-ids.json")).map_err(|e| e.to_string())?;
    table(&releases, &ids)
}
