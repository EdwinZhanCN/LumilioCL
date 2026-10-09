//! Reproducible development tools for the maintained world generator.
mod btree;
mod prepare;
mod saves;
#[cfg(test)]
mod tests;

use crate::release::Result;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

pub(crate) fn btree(input: &str, output: &str, name: &str) -> Result {
    btree::run(Path::new(input), Path::new(output), name)
}

pub(crate) fn prepare(root: &Path, version: &str) -> Result {
    prepare::prepare(root, version).map(|_| ())
}

pub(crate) fn saved(root: &Path, version: &str) -> Result {
    saves::read(root, version)
}

pub(crate) fn golden(root: &Path, version: &str) -> Result {
    let work = prepare::path(root, version)?;
    if !work.join("data.json").exists() {
        prepare::prepare(root, version)?;
    }
    let named = prepare::named(root, &work)?;
    let classes = root.join("target/worldgen/probe");
    std::fs::create_dir_all(&classes).map_err(|e| e.to_string())?;
    let libraries = work.join("libraries/*");
    let javac = std::env::var("WORLDGEN_JAVAC").unwrap_or_else(|_| "javac".into());
    prepare::execute(
        Command::new(javac)
            .arg("-cp")
            .arg(&libraries)
            .arg("-d")
            .arg(&classes)
            .arg(root.join("crates/lumilio-xtask/src/worldgen/Probe.java")),
    )?;
    let classpath = std::env::join_paths([classes, named, libraries]).map_err(|e| e.to_string())?;
    prepare::execute(
        Command::new(prepare::java())
            .args(["-Xmx1G", "-cp"])
            .arg(classpath)
            .arg("Probe")
            .arg(work.join("probe.json"))
            .current_dir(&work),
    )?;
    import(root, version)
}

pub(crate) fn import(root: &Path, version: &str) -> Result {
    let work = prepare::path(root, version)?;
    let probe = prepare::json(&work.join("probe.json"))?;
    let ids: Value =
        serde_json::from_str(include_str!("biome-ids.json")).map_err(|e| e.to_string())?;
    let data = root.join("forks/cubiomes/data").join(version);
    std::fs::create_dir_all(&data).map_err(|e| e.to_string())?;
    let file = std::fs::File::create(data.join("trees.json.gz")).map_err(|e| e.to_string())?;
    let mut gzip = flate2::write::GzEncoder::new(file, flate2::Compression::best());
    gzip.write_all(
        serde_json::to_string(&serde_json::json!({"trees": probe["trees"]}))
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    gzip.finish().map_err(|e| e.to_string())?;
    let mut golden = String::new();
    for sample in probe["samples"]
        .as_array()
        .ok_or("missing oracle samples")?
    {
        let name = sample["biome"]
            .as_str()
            .and_then(|n| n.strip_prefix("minecraft:"))
            .ok_or("invalid biome name")?;
        let id = ids[name]
            .as_u64()
            .ok_or_else(|| format!("unknown biome {name}"))?;
        let dim = match sample["dimension"].as_str() {
            Some("overworld") => 0,
            Some("nether") => -1,
            Some("end") => 1,
            _ => return Err("invalid dimension".into()),
        };
        writeln!(
            golden,
            "{} {dim} {} {} {} {id}",
            sample["seed"], sample["x"], sample["y"], sample["z"]
        )
        .unwrap();
    }
    let out = root.join("crates/lumilio-cubiomes/tests/golden");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{version}.txt")), golden).map_err(|e| e.to_string())?;
    let mut placements = String::new();
    for row in probe["placements"]
        .as_array()
        .ok_or("missing structure placement samples")?
    {
        writeln!(
            placements,
            "{} {} {} {} {} {}",
            row["seed"], row["kind"], row["rx"], row["rz"], row["x"], row["z"]
        )
        .unwrap();
    }
    std::fs::write(out.join(format!("{version}-structures.txt")), placements)
        .map_err(|e| e.to_string())?;
    let inputs = prepare::json(&work.join("data.json"))?;
    let mut rules = String::new();
    for (rule, tag) in [
        (-1, "stronghold_biased_to"),
        (8, "has_structure/woodland_mansion"),
        (13, "has_structure/trial_chambers"),
        (17, "has_structure/abandoned_camp"),
    ] {
        let mut allowed = std::collections::BTreeSet::new();
        for (path, value) in inputs.as_object().ok_or("invalid worldgen data")? {
            let matches = if rule == 17 {
                path.starts_with("data/minecraft/tags/worldgen/biome/has_structure/abandoned_camp_")
            } else {
                *path == format!("data/minecraft/tags/worldgen/biome/{tag}.json")
            };
            if matches {
                for name in value["values"].as_array().ok_or("invalid biome tag")? {
                    allowed.insert(name.as_str().ok_or("invalid biome tag entry")?.to_owned());
                }
            }
        }
        for (name, id) in ids.as_object().ok_or("invalid biome ids")? {
            if inputs
                .get(format!("data/minecraft/worldgen/biome/{name}.json"))
                .is_some()
            {
                writeln!(
                    rules,
                    "{rule} {id} {}",
                    u8::from(allowed.contains(&format!("minecraft:{name}")))
                )
                .unwrap();
            }
        }
    }
    std::fs::write(out.join(format!("{version}-rules.txt")), rules).map_err(|e| e.to_string())?;
    let metadata = prepare::json(&work.join("version.json"))?;
    let source = format!(
        "# Minecraft Java {version} generation data\n\nServer SHA1: `{}`.\nOriginal server: {}\nRelease metadata: {}\n\nRebuild using Java 25 (set WORLDGEN_JAVA and WORLDGEN_JAVAC when needed):\n\n```sh\ncargo xtask worldgen-prepare {version}\ncargo xtask worldgen-golden {version}\npython3 crates/lumilio-xtask/src/worldgen/save_golden.py {version}\ncargo xtask worldgen-save-read {version}\n```\n\n`trees.json.gz` is numeric parameter-tree data exported from the original\nserver runtime by `Probe.java`, including original child order and bounds.\nThe matching golden file contains 3 seeds × 3 dimensions × 4 heights × 96\npositions, in quart coordinates (scale 4), sampled by that same original\nserver runtime's BiomeSource/RandomState. Regional structure candidates and\nchanged biome tags are recorded separately. Saved-chunk biome samples and\nvanilla `/locate` results are produced by the last two commands. These\nchecks are independent of cubiomes and do not replace human F3 acceptance.\n\nGame jars, mappings, and renamed bytecode remain in ignored target/worldgen.\nNo Mojang source or game assets are committed. Biome ids are LumilioCL's\nids from the MIT cubiomes baseline plus sulfur_caves=187 and dappled_forest=188.\n",
        metadata["downloads"]["server"]["sha1"]
            .as_str()
            .ok_or("missing server hash")?,
        metadata["downloads"]["server"]["url"]
            .as_str()
            .ok_or("missing server URL")?,
        prepare::json(&work.join("manifest.json"))?["versions"]
            .as_array()
            .and_then(|versions| versions.iter().find(|entry| entry["id"] == version))
            .and_then(|entry| entry["url"].as_str())
            .ok_or("missing release metadata URL")?
    );
    std::fs::write(data.join("SOURCE.md"), source).map_err(|e| e.to_string())
}

pub(crate) fn diff(root: &Path, old: &str, new: &str) -> Result {
    let mut documents = vec![];
    for version in [old, new] {
        let work = prepare::path(root, version)?;
        if !work.join("data.json").exists() {
            prepare::prepare(root, version)?;
        }
        let mut data = prepare::json(&work.join("data.json"))?;
        let map = data.as_object_mut().ok_or("invalid worldgen data")?;
        for dimension in ["overworld", "nether"] {
            let report = prepare::json(&work.join(format!(
                "generated/reports/biome_parameters/minecraft/{dimension}.json"
            )))?;
            map.insert(format!("reports/biome_parameters/{dimension}.json"), report);
        }
        documents.push(data);
    }
    let a = documents[0].as_object().unwrap();
    let b = documents[1].as_object().unwrap();
    let keys: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).collect();
    let changed: Vec<_> = keys
        .into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .collect();
    let mut output = serde_json::json!({"old":old,"new":new,"changed":changed});
    for (key, value) in [("old_sha1", &documents[0]), ("new_sha1", &documents[1])] {
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        output[key] = Sha1::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
            .into();
    }
    let out = root.join("forks/cubiomes/data/diffs");
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    prepare::write_json(&out.join(format!("{old}-{new}.json")), &output)?;
    println!("{old} → {new}: {} changed resources", changed.len());
    Ok(())
}

pub(super) fn read_tree(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if path.extension().is_some_and(|e| e == "gz") {
        let mut out = vec![];
        flate2::read::GzDecoder::new(bytes.as_slice())
            .take(16 * 1024 * 1024)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&out).map_err(|e| e.to_string())
    } else {
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }
}
