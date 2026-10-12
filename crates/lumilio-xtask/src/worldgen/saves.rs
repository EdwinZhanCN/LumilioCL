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

pub(super) fn read(root: &Path, version: &str) -> Result {
    let work = path(root, version)?;
    let ids: Value =
        serde_json::from_str(include_str!("biome-ids.json")).map_err(|e| e.to_string())?;
    let mut output = String::new();
    let mut located = String::new();
    let mut variants = String::new();
    let mut fossils = String::new();
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
        for row in json(&save.join("starts.json"))?
            .as_array()
            .ok_or("invalid structure start list")?
        {
            let structure = row["structure"].as_str().ok_or("invalid structure name")?;
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
    std::fs::write(out.join(format!("{version}-saved.txt")), output).map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}-variants.txt")), variants)
        .map_err(|e| e.to_string())?;
    // Rows `seed x z y`: the saved nether fossil piece's template position.
    std::fs::write(out.join(format!("{version}-nether-fossils.txt")), fossils)
        .map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}-locates.txt")), located).map_err(|e| e.to_string())
}
