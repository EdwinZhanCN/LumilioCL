//! Read biomes from actual vanilla Anvil chunks, independently of the oracle.
use super::prepare::{json, path};
use crate::release::Result;
use lumilio_nbt::Tag;
use serde_json::Value;
use std::fmt::Write as _;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

struct FileSource(PathBuf);
impl lumilio_anvil::Source for FileSource {
    fn read(&self, offset: u64, len: usize) -> std::result::Result<Vec<u8>, lumilio_anvil::Error> {
        let read = || -> std::io::Result<Vec<u8>> {
            let mut file = std::fs::File::open(&self.0)?;
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = vec![];
            file.take(len as u64).read_to_end(&mut bytes)?;
            Ok(bytes)
        };
        read().map_err(|e| lumilio_anvil::Error::Source(e.to_string()))
    }
    fn external(&self, name: &str) -> std::result::Result<Option<Vec<u8>>, lumilio_anvil::Error> {
        let path = self.0.with_file_name(name);
        if !path.exists() {
            return Ok(None);
        }
        std::fs::read(path)
            .map(Some)
            .map_err(|e| lumilio_anvil::Error::Source(e.to_string()))
    }
}

fn biome(root: &Tag, x: i32, y: i32, z: i32) -> Result<&str> {
    let Some(Tag::List(sections)) = root.get("sections") else {
        return Err("missing chunk sections".into());
    };
    let section = sections
        .iter()
        .find(|s| s.get("Y").and_then(Tag::as_i64) == Some(i64::from(y.div_euclid(4))))
        .ok_or("missing biome section")?;
    let Some(Tag::List(palette)) = section.at(&["biomes", "palette"]) else {
        return Err("missing biome palette".into());
    };
    let index = if palette.len() == 1 {
        0
    } else {
        let Some(Tag::LongArray(words)) = section.at(&["biomes", "data"]) else {
            return Err("missing biome indices".into());
        };
        let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(1) as usize;
        let index = (y.rem_euclid(4) * 16 + z.rem_euclid(4) * 4 + x.rem_euclid(4)) as usize;
        let per_word = 64 / bits;
        let word = *words.get(index / per_word).ok_or("truncated biome data")? as u64;
        ((word >> ((index % per_word) * bits)) & ((1 << bits) - 1)) as usize
    };
    palette
        .get(index)
        .and_then(Tag::as_str)
        .ok_or_else(|| "invalid biome index".into())
}

/// The saved NBT of chunk (cx, cz) in a dimension (-1, 0, 1) of a world.
fn chunk(save: &Path, dimension: i32, cx: i32, cz: i32) -> Result<Tag> {
    let (resource, folder) = match dimension {
        -1 => ("the_nether", "DIM-1"),
        0 => ("overworld", ""),
        1 => ("the_end", "DIM1"),
        _ => return Err("invalid dimension".into()),
    };
    let named = save.join("world/dimensions/minecraft").join(resource);
    let dimension_path = if named.is_dir() {
        named
    } else {
        save.join("world").join(folder)
    };
    let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
    let region = dimension_path
        .join("region")
        .join(format!("r.{rx}.{rz}.mca"));
    lumilio_anvil::Region::open(FileSource(region), rx, rz)
        .map_err(|e| e.to_string())?
        .chunk(cx.rem_euclid(32) as usize, cz.rem_euclid(32) as usize)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "unsaved chunk".into())
}

/// Saved starts whose variant `start_variant` reads.
const VARIANT_STRUCTURES: [&str; 3] = [
    "minecraft:bastion_remnant",
    "minecraft:igloo",
    "minecraft:end_city",
];

/// Every string anywhere under `tag`, depth first.
fn strings<'a>(tag: &'a Tag, out: &mut Vec<&'a str>) {
    match tag {
        Tag::String(text) => out.push(text),
        Tag::List(items) => items.iter().for_each(|item| strings(item, out)),
        Tag::Compound(entries) => entries.iter().for_each(|(_, item)| strings(item, out)),
        _ => {}
    }
}

/// The start variant recorded in a saved structure start, in the codes of
/// `lumilio_cubiomes::Structure::variant`: the bastion's start piece, or 1
/// for an igloo with a basement or an End City with a ship.
fn start_variant(chunk: &Tag, structure: &str) -> Result<(i32, i32)> {
    let starts = chunk
        .at(&["structures", "starts"])
        .ok_or("chunk without structure starts")?;
    let (kind, names): (i32, &[&str]) = match structure {
        "minecraft:bastion_remnant" => (15, &["minecraft:bastion_remnant"]),
        "minecraft:igloo" => (3, &["minecraft:igloo"]),
        "minecraft:end_city" => (16, &["minecraft:end_city", "minecraft:endcity"]),
        _ => return Err(format!("unknown structure {structure}")),
    };
    let start = names
        .iter()
        .find_map(|name| starts.get(name))
        .ok_or_else(|| format!("no saved {structure} start"))?;
    let Some(Tag::List(children)) = start.get("Children") else {
        return Err(format!("{structure} start without pieces"));
    };
    let mut text = vec![];
    let variant = match kind {
        15 => {
            strings(children.first().ok_or("bastion without pieces")?, &mut text);
            let pieces = [
                "bastion/units/air_base",
                "bastion/hoglin_stable/air_base",
                "bastion/treasure/big_air_full",
                "bastion/bridge/starting_pieces/entrance_base",
            ];
            let found = pieces
                .iter()
                .position(|piece| text.iter().any(|t| t.ends_with(piece)))
                .ok_or("unknown bastion start piece")?;
            found as i32
        }
        3 => {
            children.iter().for_each(|piece| strings(piece, &mut text));
            i32::from(text.iter().any(|t| t.ends_with("igloo/bottom")))
        }
        _ => {
            children.iter().for_each(|piece| strings(piece, &mut text));
            i32::from(text.iter().any(|t| t.ends_with("ship")))
        }
    };
    Ok((kind, variant))
}

/// One stack of a chest: item id, count, and an enchanted book's stored
/// enchantment and level.
type Stack = (String, i64, Option<(String, i64)>);

/// A chest's items from `/data get` output (SNBT): each stack's id and count,
/// and an enchanted book's stored enchantment and level (1.21–1.21.4 nest
/// them under `levels`).
fn chest_items(text: &str) -> Vec<Stack> {
    let mut stacks = vec![];
    let (mut depth, mut start) = (0, None);
    for (i, c) in text.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0
                    && let Some(s) = start.take()
                {
                    stacks.push(&text[s..=i]);
                }
            }
            _ => {}
        }
    }
    let quoted = |stack: &str, key: &str| -> Option<String> {
        let rest = &stack[stack.find(key)? + key.len()..];
        let rest = &rest[rest.find('"')? + 1..];
        Some(rest[..rest.find('"')?].to_owned())
    };
    let number = |stack: &str, key: &str| -> Option<i64> {
        let rest = &stack[stack.rfind(key)? + key.len()..];
        let digits: String = rest
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits.parse().ok()
    };
    stacks
        .into_iter()
        .filter_map(|stack| {
            let id = quoted(stack, " id:")?;
            let count = number(stack, "count:")?;
            let enchantment = stack.find("stored_enchantments").and_then(|at| {
                let rest = &stack[at..];
                let rest = rest.strip_prefix("stored_enchantments\": {")?;
                let rest = rest.strip_prefix("levels: {").unwrap_or(rest);
                let name = rest.strip_prefix('"')?;
                let end = name.find('"')?;
                // `"id": level`: the digits right after the quoted id.
                let digits: String = name[end + 1..]
                    .trim_start_matches([':', ' '])
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                Some((name[..end].to_owned(), digits.parse().ok()?))
            });
            Some((id, count, enchantment))
        })
        .collect()
}

/// Blocks the area goldens record, by what generates them alone.
#[derive(Default)]
struct AreaFinds {
    /// `seed x y z kind`: raw ore blocks (1 copper, 2 iron), which only veins place.
    ores: String,
    /// `seed dimension x y z entity`: spawners and their mob.
    spawners: String,
    /// `seed dimension x0 y0 z0 x1 y1 z1 blocks`: bone block clusters (fossils).
    fossils: String,
}

/// Scans every fully generated area of a saved world.
fn read_areas(save: &Path, seed: i64, finds: &mut AreaFinds) -> Result {
    let areas = save.join("areas.json");
    for area in json(&areas)?.as_array().ok_or("invalid area list")? {
        let number = |key: &str| {
            area[key]
                .as_i64()
                .and_then(|v| i32::try_from(v).ok())
                .ok_or("invalid area bounds")
        };
        let dim = match area["dimension"].as_str() {
            Some("overworld") => 0,
            Some("the_nether") => -1,
            Some("the_end") => 1,
            _ => return Err("unknown area dimension".into()),
        };
        let mut bones = std::collections::BTreeSet::new();
        for cx in number("cx0")?..=number("cx1")? {
            for cz in number("cz0")?..=number("cz1")? {
                let root = chunk(save, dim, cx, cz)?;
                let parsed = lumilio_anvil::Chunk::from_nbt(&root).map_err(|e| format!("{e:?}"))?;
                for section in &parsed.sections {
                    if section.is_empty() {
                        continue;
                    }
                    for y in 0..16 {
                        for z in 0..16 {
                            for x in 0..16 {
                                let block = section.block(x, y, z);
                                let at = [
                                    cx * 16 + x as i32,
                                    section.y * 16 + y as i32,
                                    cz * 16 + z as i32,
                                ];
                                match block {
                                    "minecraft:raw_copper_block" | "minecraft:raw_iron_block" => {
                                        let kind = if block.contains("copper") { 1 } else { 2 };
                                        writeln!(
                                            finds.ores,
                                            "{seed} {} {} {} {kind}",
                                            at[0], at[1], at[2]
                                        )
                                        .unwrap();
                                    }
                                    "minecraft:bone_block" => {
                                        bones.insert(at);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                if let Some(Tag::List(entities)) = root.get("block_entities") {
                    for entity in entities {
                        if entity.get("id").and_then(Tag::as_str) != Some("minecraft:mob_spawner") {
                            continue;
                        }
                        let at = ["x", "y", "z"]
                            .map(|key| entity.get(key).and_then(Tag::as_i64).unwrap_or(0));
                        let mob = entity
                            .at(&["SpawnData", "entity", "id"])
                            .and_then(Tag::as_str)
                            .unwrap_or("?");
                        writeln!(
                            finds.spawners,
                            "{seed} {dim} {} {} {} {mob}",
                            at[0], at[1], at[2]
                        )
                        .unwrap();
                    }
                }
            }
        }
        // Bone blocks within two blocks of each other belong to one fossil.
        while let Some(&first) = bones.iter().next() {
            bones.remove(&first);
            let mut cluster = vec![first];
            let mut next = 0;
            while next < cluster.len() {
                let [x, y, z] = cluster[next];
                next += 1;
                for dx in -2..=2 {
                    for dy in -2..=2 {
                        for dz in -2..=2 {
                            if bones.remove(&[x + dx, y + dy, z + dz]) {
                                cluster.push([x + dx, y + dy, z + dz]);
                            }
                        }
                    }
                }
            }
            let low = |i: usize| cluster.iter().map(|p| p[i]).min().unwrap();
            let high = |i: usize| cluster.iter().map(|p| p[i]).max().unwrap();
            writeln!(
                finds.fossils,
                "{seed} {dim} {} {} {} {} {} {} {}",
                low(0),
                low(1),
                low(2),
                high(0),
                high(1),
                high(2),
                cluster.len()
            )
            .unwrap();
        }
    }
    Ok(())
}

pub(super) fn read(root: &Path, version: &str) -> Result {
    let work = path(root, version)?;
    let ids: Value =
        serde_json::from_str(include_str!("biome-ids.json")).map_err(|e| e.to_string())?;
    let mut output = String::new();
    let mut located = String::new();
    let mut variants = String::new();
    let mut fossils = String::new();
    let mut chests = String::new();
    let mut loot = String::new();
    let mut stacks = String::new();
    let mut finds = AreaFinds::default();
    for seed in [262_i64, 9_876_543_210, -123_456_789_012_345] {
        let save = work.join("saves").join(seed.to_string());
        let level = lumilio_nbt::parse_maybe_gzip(
            &std::fs::read(save.join("world/level.dat")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let spawn = match level.at(&["Data", "spawn", "pos"]) {
            Some(Tag::IntArray(pos)) if pos.len() == 3 => [pos[0], pos[2]],
            _ => ["SpawnX", "SpawnZ"]
                .map(|key| {
                    level
                        .at(&["Data", key])
                        .and_then(Tag::as_i64)
                        .and_then(|v| i32::try_from(v).ok())
                })
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or("missing world spawn")?
                .try_into()
                .map_err(|_| "invalid world spawn")?,
        };
        writeln!(located, "{seed} 0 -2 {} {}", spawn[0], spawn[1]).unwrap();
        let coordinates = json(&save.join("coordinates.json"))?;
        for point in coordinates.as_array().ok_or("invalid saved coordinates")? {
            let number = |key: &str| {
                point[key]
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or("invalid coordinate")
            };
            let (x, y, z) = (number("x")?, number("y")?, number("z")?);
            let dimension = match point["dimension"].as_str() {
                Some("overworld") => 0,
                Some("nether") => -1,
                Some("end") => 1,
                _ => return Err("invalid dimension".into()),
            };
            let chunk = chunk(&save, dimension, x.div_euclid(4), z.div_euclid(4))?;
            let name = biome(&chunk, x, y, z)?;
            let id = ids[name.strip_prefix("minecraft:").unwrap_or(name)]
                .as_u64()
                .ok_or("unknown saved biome")?;
            writeln!(output, "{seed} {dimension} {x} {y} {z} {id}").unwrap();
        }
        for row in json(&save.join("locates.json"))?
            .as_array()
            .ok_or("invalid locate results")?
        {
            let kind = match row["kind"].as_str() {
                Some("Stronghold") => -1,
                Some("Village") => 4,
                Some("Fortress") => 14,
                Some("EndCity") => 16,
                Some("AbandonedCamp") => 17,
                _ => return Err("unknown located structure".into()),
            };
            let dim = match row["dimension"].as_str() {
                Some("overworld") => 0,
                Some("the_nether") => -1,
                Some("the_end") => 1,
                _ => return Err("unknown located dimension".into()),
            };
            writeln!(located, "{seed} {dim} {kind} {} {}", row["x"], row["z"]).unwrap();
        }
        read_areas(&save, seed, &mut finds)?;
        // Filled by `save_golden.py --chests` on a copy of the world.
        let filled = save.join("chests.json");
        if filled.exists() {
            for chest in json(&filled)?.as_array().ok_or("invalid chest list")? {
                let items = chest["items"].as_str().ok_or("chest without items")?;
                let apples = items.matches("minecraft:enchanted_golden_apple").count();
                let at = format!("{} {} {}", chest["x"], chest["y"], chest["z"]);
                writeln!(loot, "{seed} {at} {apples}").unwrap();
                // Totals per item (chests split stacks across slots), and
                // each enchanted book on its own row.
                let mut totals = std::collections::BTreeMap::new();
                for (id, count, enchantment) in chest_items(items) {
                    match enchantment {
                        Some((name, level)) => {
                            writeln!(stacks, "{seed} {at} {id}:{name} {level}").unwrap();
                        }
                        None => *totals.entry(id).or_insert(0) += count,
                    }
                }
                for (id, count) in totals {
                    writeln!(stacks, "{seed} {at} {id} {count}").unwrap();
                }
            }
        }
        for row in json(&save.join("starts.json"))?
            .as_array()
            .ok_or("invalid structure start list")?
        {
            let structure = row["structure"].as_str().ok_or("invalid structure name")?;
            if structure == "minecraft:desert_pyramid" {
                let number = |key: &str| {
                    row[key]
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or("invalid structure start position")
                };
                let (cx, cz) = (number("x")?.div_euclid(16), number("z")?.div_euclid(16));
                // Its four chests sit 8 to 12 blocks into the start chunk.
                let root = chunk(&save, 0, cx, cz)?;
                if let Some(Tag::List(entities)) = root.get("block_entities") {
                    for entity in entities {
                        let table = entity.get("LootTable").and_then(Tag::as_str);
                        if table != Some("minecraft:chests/desert_pyramid") {
                            continue;
                        }
                        let at = ["x", "y", "z"]
                            .map(|key| entity.get(key).and_then(Tag::as_i64).unwrap_or(0));
                        let loot = entity
                            .get("LootTableSeed")
                            .and_then(Tag::as_i64)
                            .ok_or("pyramid chest without a loot seed")?;
                        writeln!(
                            chests,
                            "{seed} {cx} {cz} {} {} {} {loot}",
                            at[0], at[1], at[2]
                        )
                        .unwrap();
                    }
                }
                continue;
            }
            if structure == "minecraft:nether_fossil" {
                let number = |key: &str| {
                    row[key]
                        .as_i64()
                        .and_then(|v| i32::try_from(v).ok())
                        .ok_or("invalid structure start position")
                };
                let chunk = chunk(
                    &save,
                    -1,
                    number("x")?.div_euclid(16),
                    number("z")?.div_euclid(16),
                )?;
                let piece = chunk
                    .at(&["structures", "starts", structure, "Children"])
                    .and_then(|children| match children {
                        Tag::List(pieces) => pieces.first(),
                        _ => None,
                    })
                    .ok_or("nether fossil start without pieces")?;
                let at = ["TPX", "TPY", "TPZ"]
                    .map(|key| piece.get(key).and_then(Tag::as_i64).unwrap_or(i64::MIN));
                if at.contains(&i64::MIN) {
                    return Err("nether fossil piece without template position".into());
                }
                writeln!(fossils, "{seed} {} {} {}", at[0], at[2], at[1]).unwrap();
                continue;
            }
            if !VARIANT_STRUCTURES.contains(&structure) {
                continue;
            }
            let number = |key: &str| {
                row[key]
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or("invalid structure start position")
            };
            let (x, z) = (number("x")?, number("z")?);
            let dim = match row["dimension"].as_str() {
                Some("overworld") => 0,
                Some("the_nether") => -1,
                Some("the_end") => 1,
                _ => return Err("unknown structure start dimension".into()),
            };
            let chunk = chunk(&save, dim, x.div_euclid(16), z.div_euclid(16))?;
            let (kind, variant) = start_variant(&chunk, structure)
                .map_err(|e| format!("{seed} {structure} at {x} {z}: {e}"))?;
            writeln!(variants, "{seed} {dim} {kind} {x} {z} {variant}").unwrap();
        }
    }
    let out = root.join("crates/lumilio-cubiomes/tests/golden");
    for (name, rows) in [
        ("ores", &finds.ores),
        ("spawners", &finds.spawners),
        ("bones", &finds.fossils),
    ] {
        std::fs::write(out.join(format!("{version}-{name}.txt")), rows)
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(out.join(format!("{version}-saved.txt")), output).map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}-variants.txt")), variants)
        .map_err(|e| e.to_string())?;
    // Rows `seed chunk_x chunk_z x y z loot_seed`: unopened desert pyramid chests.
    std::fs::write(out.join(format!("{version}-pyramid-chests.txt")), chests)
        .map_err(|e| e.to_string())?;
    // Rows `seed x y z apples`: enchanted golden apples in the filled chests.
    // Rows `seed x y z id total`, `seed x y z minecraft:enchanted_book:<id> level`:
    // everything those chests hold.
    if !loot.is_empty() {
        std::fs::write(out.join(format!("{version}-pyramid-loot.txt")), loot)
            .map_err(|e| e.to_string())?;
        std::fs::write(out.join(format!("{version}-pyramid-items.txt")), stacks)
            .map_err(|e| e.to_string())?;
    }
    // Rows `seed x z y`: the saved nether fossil piece's template position.
    std::fs::write(out.join(format!("{version}-nether-fossils.txt")), fossils)
        .map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}-locates.txt")), located).map_err(|e| e.to_string())
}
