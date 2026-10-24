//! Loot the fork simulates from a chest's loot seed: the first pool of the
//! desert pyramid's table, which holds the enchanted golden apple.
//!
//! `extract` reads one release's original server jar and writes
//! `forks/cubiomes/data/<release>/loot.json`: the pyramid's place among its
//! decoration step's structures (registries list them by id), the pool's
//! rolls and entries, and for an enchanted book the maximum level of every
//! enchantment its options tag lists, in the tag's order. `table` turns every
//! release's file into `forks/cubiomes/tables/loot.h`.

use std::fmt::Write as _;
use std::io::Read;
use std::path::Path;

use serde_json::{Value, json};

use crate::release::Result;

struct Jar(zip::ZipArchive<std::fs::File>);

impl Jar {
    fn json(&mut self, name: &str) -> Result<Option<Value>> {
        let Ok(mut entry) = self.0.by_name(name) else {
            return Ok(None);
        };
        let mut bytes = vec![];
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| e.to_string())
    }

    /// A tag's members in the order the game binds them: entries in file
    /// order, a nested tag expanded where it is listed, duplicates dropped.
    fn tag(&mut self, kind: &str, name: &str, out: &mut Vec<String>) -> Result {
        let path = format!(
            "data/minecraft/tags/{kind}/{}.json",
            name.trim_start_matches("minecraft:")
        );
        let tag = self
            .json(&path)?
            .ok_or_else(|| format!("missing tag {name}"))?;
        for value in tag["values"].as_array().ok_or("invalid tag")? {
            let id = value
                .as_str()
                .or_else(|| value["id"].as_str())
                .ok_or("invalid tag entry")?;
            if let Some(nested) = id.strip_prefix('#') {
                self.tag(kind, nested, out)?;
            } else if !out.iter().any(|known| known == id) {
                out.push(id.to_owned());
            }
        }
        Ok(())
    }
}

/// An int number provider as (min, max).
fn range(value: &Value) -> Result<(i64, i64)> {
    if let Some(n) = value.as_f64() {
        return Ok((n.floor() as i64, n.floor() as i64));
    }
    match value["type"].as_str() {
        Some("minecraft:uniform") => {
            let bound = |key: &str| {
                value[key]
                    .as_f64()
                    .map(|v| v.floor() as i64)
                    .ok_or("invalid uniform bound")
            };
            Ok((bound("min")?, bound("max")?))
        }
        Some("minecraft:constant") => {
            let n = value["value"].as_f64().ok_or("invalid constant")?;
            Ok((n.floor() as i64, n.floor() as i64))
        }
        _ => Err(format!("unsupported number provider {value}")),
    }
}

fn extract_entry(jar: &mut Jar, entry: &Value) -> Result<Value> {
    if !entry["conditions"].is_null() {
        return Err("loot entries with conditions are not modelled".into());
    }
    let weight = entry["weight"].as_i64().unwrap_or(1);
    match entry["type"].as_str() {
        Some("minecraft:empty") => return Ok(json!({"weight": weight})),
        Some("minecraft:item") => {}
        other => return Err(format!("unsupported loot entry {other:?}")),
    }
    let item = entry["name"].as_str().ok_or("item entry without name")?;
    let mut out = json!({"weight": weight, "item": item});
    // 26.3 names a single function `modifier`; earlier releases list `functions`.
    let functions: Vec<&Value> = match (&entry["functions"], &entry["modifier"]) {
        (Value::Array(list), _) | (_, Value::Array(list)) => list.iter().collect(),
        (_, single @ Value::Object(_)) => vec![single],
        _ => vec![],
    };
    for function in functions {
        let kind = function["function"]
            .as_str()
            .or_else(|| function["type"].as_str())
            .ok_or("loot function without type")?;
        match kind {
            "minecraft:set_count" => {
                let (min, max) = range(&function["count"])?;
                out["count"] = json!([min, max]);
            }
            "minecraft:enchant_randomly" => {
                let options = function["options"]
                    .as_str()
                    .and_then(|o| o.strip_prefix('#'))
                    .ok_or("enchant_randomly without an options tag")?;
                let mut members = vec![];
                jar.tag("enchantment", options, &mut members)?;
                out["enchantments"] = json!(members);
                let mut levels = vec![];
                for member in &members {
                    let path = format!(
                        "data/minecraft/enchantment/{}.json",
                        member.trim_start_matches("minecraft:")
                    );
                    let enchantment = jar
                        .json(&path)?
                        .ok_or_else(|| format!("missing enchantment {member}"))?;
                    levels.push(
                        enchantment["max_level"]
                            .as_i64()
                            .ok_or("enchantment without max level")?,
                    );
                }
                out["levels"] = json!(levels);
            }
            _ => return Err(format!("loot function {kind} is not modelled")),
        }
    }
    Ok(out)
}

/// The pyramid loot of one release, or `None` before data-driven enchantments.
pub(super) fn extract(work: &Path, data: &Value) -> Result<Option<Value>> {
    let file = std::fs::File::open(work.join("game.jar")).map_err(|e| e.to_string())?;
    let mut jar = Jar(zip::ZipArchive::new(file).map_err(|e| e.to_string())?);
    let Some(table) = jar.json("data/minecraft/loot_table/chests/desert_pyramid.json")? else {
        return Ok(None);
    };
    if jar
        .json("data/minecraft/tags/enchantment/on_random_loot.json")?
        .is_none()
    {
        return Ok(None);
    }
    let mut surface: Vec<&str> = data
        .as_object()
        .ok_or("invalid worldgen data")?
        .iter()
        .filter(|(_, structure)| structure["step"].as_str() == Some("surface_structures"))
        .filter_map(|(path, _)| {
            path.strip_prefix("data/minecraft/worldgen/structure/")
                .and_then(|n| n.strip_suffix(".json"))
        })
        .collect();
    surface.sort();
    let index = surface
        .iter()
        .position(|name| *name == "desert_pyramid")
        .ok_or("desert pyramid is not a surface structure")?;
    let pool = &table["pools"][0];
    let (min, max) = range(&pool["rolls"])?;
    let mut entries = vec![];
    for entry in pool["entries"].as_array().ok_or("pool without entries")? {
        entries.push(extract_entry(&mut jar, entry)?);
    }
    Ok(Some(json!({
        "pyramid": {"step": 4, "index": index, "rolls": [min, max], "entries": entries}
    })))
}

/// The table every committed `loot.json` produces.
pub(super) fn render(root: &Path) -> Result<String> {
    table(&super::releases(root, "loot.json")?)
}

fn mc(version: &str) -> String {
    format!("MC_{}", version.replace('.', "_"))
}

/// The C table for every release with a `loot.json`, ordered by release.
pub(super) fn table(releases: &[(String, Value)]) -> Result<String> {
    let mut out = String::from(
        "/* Generated by `cargo xtask cubiomes-tables` from\n * forks/cubiomes/data/<release>/loot.json. Do not edit.\n * Derived from original Minecraft data; see forks/cubiomes/data/README.md. */\n",
    );
    let mut rows = String::new();
    for (n, (version, file)) in releases.iter().enumerate() {
        let pyramid = &file["pyramid"];
        let entries = pyramid["entries"].as_array().ok_or("pyramid entries")?;
        let mut list = vec![];
        for (i, entry) in entries.iter().enumerate() {
            let weight = entry["weight"].as_i64().ok_or("entry weight")?;
            let item = entry["item"].as_str();
            let name = item.map_or("NULL".to_owned(), |item| format!("\"{item}\""));
            if let Some(levels) = entry["levels"].as_array() {
                let levels: Vec<String> = levels.iter().map(|l| l.to_string()).collect();
                let names: Vec<String> = entry["enchantments"]
                    .as_array()
                    .ok_or("book without enchantment names")?
                    .iter()
                    .map(|e| format!("\"{}\"", e.as_str().unwrap_or_default()))
                    .collect();
                writeln!(
                    out,
                    "static const int loot_levels_{n}_{i}[] = {{ {} }};",
                    levels.join(", ")
                )
                .expect("writing to a String cannot fail");
                writeln!(
                    out,
                    "static const char *const loot_names_{n}_{i}[] = {{ {} }};",
                    names.join(", ")
                )
                .expect("writing to a String cannot fail");
                list.push(format!(
                    "{{ {weight}, LE_ENCHANT, 0, 0, loot_levels_{n}_{i}, {}, {name}, loot_names_{n}_{i} }}",
                    levels.len()
                ));
            } else if let Some(count) = entry["count"].as_array() {
                list.push(format!(
                    "{{ {weight}, LE_COUNT, {}, {}, NULL, 0, {name}, NULL }}",
                    count[0], count[1]
                ));
            } else if item == Some("minecraft:enchanted_golden_apple") {
                list.push(format!(
                    "{{ {weight}, LE_TARGET, 0, 0, NULL, 0, {name}, NULL }}"
                ));
            } else {
                list.push(format!(
                    "{{ {weight}, LE_PLAIN, 0, 0, NULL, 0, {name}, NULL }}"
                ));
            }
        }
        writeln!(
            out,
            "static const LootEntry loot_entries_{n}[] = {{ {} }};",
            list.join(", ")
        )
        .expect("writing to a String cannot fail");
        let rolls = pyramid["rolls"].as_array().ok_or("pyramid rolls")?;
        writeln!(
            rows,
            "    {{ {}, {}, {}, {}, {}, {}, loot_entries_{n} }},",
            mc(version),
            pyramid["step"],
            pyramid["index"],
            rolls[0],
            rolls[1],
            entries.len()
        )
        .expect("writing to a String cannot fail");
    }
    writeln!(
        out,
        "static const PyramidLoot PYRAMID_LOOT[] = {{\n{rows}}};"
    )
    .expect("writing to a String cannot fail");
    Ok(out)
}
