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

pub(super) fn read(root: &Path, version: &str) -> Result {
    let work = path(root, version)?;
    let ids: Value =
        serde_json::from_str(include_str!("biome-ids.json")).map_err(|e| e.to_string())?;
    let mut output = String::new();
    let mut located = String::new();
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
            let (dimension, folder) = match point["dimension"].as_str() {
                Some("overworld") => (0, ""),
                Some("nether") => (-1, "DIM-1"),
                Some("end") => (1, "DIM1"),
                _ => return Err("invalid dimension".into()),
            };
            let resource = match dimension {
                -1 => "the_nether",
                0 => "overworld",
                1 => "the_end",
                _ => unreachable!(),
            };
            let named = save.join("world/dimensions/minecraft").join(resource);
            let dimension_path = if named.is_dir() {
                named
            } else {
                save.join("world").join(folder)
            };
            let (cx, cz) = (x.div_euclid(4), z.div_euclid(4));
            let (rx, rz) = (cx.div_euclid(32), cz.div_euclid(32));
            let region = dimension_path
                .join("region")
                .join(format!("r.{rx}.{rz}.mca"));
            let chunk = lumilio_anvil::Region::open(FileSource(region), rx, rz)
                .map_err(|e| e.to_string())?
                .chunk(cx.rem_euclid(32) as usize, cz.rem_euclid(32) as usize)
                .map_err(|e| e.to_string())?
                .ok_or("unsaved chunk")?;
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
    }
    let out = root.join("crates/lumilio-cubiomes/tests/golden");
    std::fs::write(out.join(format!("{version}-saved.txt")), output).map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}-locates.txt")), located).map_err(|e| e.to_string())
}
