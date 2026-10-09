//! `cargo xtask block-colors <version>`: the colour of every block of a game
//! version, from that version's own client jar (plan W13).
//!
//! The jar is downloaded from Mojang into the ignored `target/` and checked
//! against the sha1 in Mojang's version metadata; it is never committed. For
//! each blockstate the first model is followed through its parents to a
//! texture (the top face if there is one), and the texture's pixels are
//! averaged. The result is derived data and carries its provenance, so it can
//! be rebuilt: `SOURCE.md` next to the table records the version, the jar's
//! sha1, this command and the commit it ran at.
use crate::release::Result;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Bits of a block's flags in the table.
pub const SEE_THROUGH: u8 = 1;
pub const TINT_GRASS: u8 = 2;
pub const TINT_FOLIAGE: u8 = 4;
pub const TINT_WATER: u8 = 8;

pub type Table = BTreeMap<String, [u8; 4]>;

/// Which texture of a model to show from above, most wanted first.
const FACES: [&str; 10] = [
    "top", "up", "all", "end", "side", "texture", "cross", "plant", "pattern", "particle",
];
const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

fn read_entry<R: Read + Seek>(jar: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let mut entry = jar.by_name(name).ok()?;
    let mut bytes = Vec::new();
    entry.take(32 * 1024 * 1024).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn json<R: Read + Seek>(jar: &mut zip::ZipArchive<R>, name: &str) -> Option<Value> {
    serde_json::from_slice(&read_entry(jar, name)?).ok()
}

fn without_namespace(reference: &str) -> &str {
    reference.strip_prefix("minecraft:").unwrap_or(reference)
}

/// The first model a blockstate file names, in either of its two forms.
fn first_model(state: &Value) -> Option<String> {
    let pick = |value: &Value| {
        let one = value.as_array().map_or(value, |list| {
            &list[0..1.min(list.len())][..]
                .first()
                .unwrap_or(value)
                .clone()
        });
        one.get("model")?.as_str().map(str::to_owned)
    };
    if let Some(variants) = state.get("variants").and_then(Value::as_object) {
        return variants.values().next().and_then(pick);
    }
    let parts = state.get("multipart")?.as_array()?;
    parts.first()?.get("apply").and_then(pick)
}

/// The texture a model shows from above: the parent chain's `textures`
/// merged (a child overrides its parent), `#name` references followed.
fn texture_of<R: Read + Seek>(jar: &mut zip::ZipArchive<R>, model: &str) -> Option<String> {
    let mut textures: BTreeMap<String, String> = BTreeMap::new();
    let mut next = Some(without_namespace(model).to_owned());
    for _ in 0..16 {
        let Some(path) = next.take() else { break };
        let Some(document) = json(jar, &format!("assets/minecraft/models/{path}.json")) else {
            break;
        };
        if let Some(own) = document.get("textures").and_then(Value::as_object) {
            for (key, value) in own {
                if let Some(value) = value.as_str() {
                    textures
                        .entry(key.clone())
                        .or_insert_with(|| value.to_owned());
                }
            }
        }
        next = document
            .get("parent")
            .and_then(Value::as_str)
            .map(|parent| without_namespace(parent).to_owned());
    }
    let resolve = |mut value: String| {
        for _ in 0..8 {
            match value.strip_prefix('#') {
                Some(key) => value = textures.get(key)?.clone(),
                None => return Some(value),
            }
        }
        None
    };
    FACES
        .iter()
        .find_map(|face| textures.get(*face).cloned().and_then(&resolve))
        .or_else(|| textures.values().next().cloned().and_then(&resolve))
        .map(|texture| without_namespace(&texture).to_owned())
}

/// The mean colour of a texture's visible pixels and how much of it is
/// visible (0–1). An animation strip is read as its first frame.
fn average(png: &[u8]) -> Option<([u8; 3], f64)> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let (width, height) = image.dimensions();
    if width == 0 || height < width {
        return None;
    }
    let (mut sum, mut weight, mut pixels) = ([0.0_f64; 3], 0.0_f64, 0.0_f64);
    for (_, row, pixel) in image.enumerate_pixels() {
        if row >= width {
            break;
        }
        let alpha = f64::from(pixel[3]) / 255.0;
        for channel in 0..3 {
            sum[channel] += f64::from(pixel[channel]) * alpha;
        }
        weight += alpha;
        pixels += 1.0;
    }
    if weight == 0.0 {
        return None;
    }
    let mean = sum.map(|total| (total / weight).round() as u8);
    Some((mean, weight / pixels))
}

fn tint_of(block: &str) -> u8 {
    match block {
        "grass_block" | "grass" | "short_grass" | "tall_grass" | "fern" | "large_fern"
        | "sugar_cane" | "potted_fern" => TINT_GRASS,
        "vine"
        | "oak_leaves"
        | "jungle_leaves"
        | "acacia_leaves"
        | "dark_oak_leaves"
        | "mangrove_leaves"
        | "azalea_leaves"
        | "flowering_azalea_leaves" => TINT_FOLIAGE,
        "water" | "bubble_column" | "water_cauldron" => TINT_WATER,
        _ => 0,
    }
}

/// Builds the table from a client jar's blockstates, models and textures.
pub fn build_table<R: Read + Seek>(jar: &mut zip::ZipArchive<R>) -> Table {
    let states: Vec<String> = jar
        .file_names()
        .filter_map(|name| {
            name.strip_prefix("assets/minecraft/blockstates/")?
                .strip_suffix(".json")
                .map(str::to_owned)
        })
        .collect();
    let mut table = Table::new();
    for block in states {
        let Some(state) = json(jar, &format!("assets/minecraft/blockstates/{block}.json")) else {
            continue;
        };
        let Some(texture) = first_model(&state).and_then(|model| texture_of(jar, &model)) else {
            continue;
        };
        let Some(png) = read_entry(jar, &format!("assets/minecraft/textures/{texture}.png")) else {
            continue;
        };
        let Some((rgb, visible)) = average(&png) else {
            continue;
        };
        let mut flags = tint_of(&block);
        if visible < 0.5 {
            flags |= SEE_THROUGH;
        }
        table.insert(
            format!("minecraft:{block}"),
            [rgb[0], rgb[1], rgb[2], flags],
        );
    }
    // Fluids have no blockstate file; their textures are named here.
    for (block, texture, flags) in [
        ("water", "block/water_still", TINT_WATER),
        ("lava", "block/lava_still", 0),
    ] {
        if let Some((rgb, _)) = read_entry(jar, &format!("assets/minecraft/textures/{texture}.png"))
            .and_then(|png| average(&png))
        {
            table.insert(
                format!("minecraft:{block}"),
                [rgb[0], rgb[1], rgb[2], flags],
            );
        }
    }
    table
}

/// The table as JSON, one block per line so a regenerated table diffs cleanly.
pub fn render(table: &Table) -> String {
    let mut out = String::from("{\n  \"blocks\": {\n");
    let last = table.len().saturating_sub(1);
    for (at, (block, [r, g, b, flags])) in table.iter().enumerate() {
        let comma = if at == last { "" } else { "," };
        let _ = writeln!(out, "    \"{block}\": [{r}, {g}, {b}, {flags}]{comma}");
    }
    out.push_str("  }\n}\n");
    out
}

/// `SOURCE.md` with the section for `version` written or replaced, the others
/// kept, sections in version order of first appearance.
pub fn source_section(
    existing: &str,
    version: &str,
    sha1: &str,
    commit: &str,
    blocks: usize,
) -> String {
    let heading = format!("## {version}\n");
    let section = format!(
        "{heading}\n- Client jar sha1: `{sha1}` (from Mojang's version metadata)\n- Command: `cargo xtask block-colors {version}`\n- Tool commit: `{commit}`\n- Blocks: {blocks}\n- Colour: the mean of the visible pixels of the texture a block shows from above (`top`, then `up`, `all`, `end`, `side`); flags 1 = see-through, 2 = grass tint, 4 = foliage tint, 8 = water tint\n\n"
    );
    let mut out = if existing.trim().is_empty() {
        String::from(
            "# Block colour tables\n\nDerived from the original game's textures; regenerate with the command in each section. The jar itself is not kept in the repository.\n\n",
        )
    } else {
        existing.to_owned()
    };
    if let Some(start) = out.find(&heading) {
        let end = out[start + heading.len()..]
            .find("\n## ")
            .map_or(out.len(), |at| start + heading.len() + at + 1);
        out.replace_range(start..end, &section);
    } else {
        out.push_str(&section);
    }
    out
}

fn curl(url: &str, to: &Path) -> Result {
    let status = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--output",
        ])
        .arg(to)
        .arg(url)
        .status()
        .map_err(|error| format!("cannot run curl: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("download failed: {url}"))
}

fn sha1_of(path: &Path) -> Result<String> {
    use sha1::{Digest, Sha1};
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(Sha1::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn commit(root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |text| text.trim().to_owned())
}

/// Downloads `version`'s client jar, builds its table and writes it under
/// `crates/lumilio-plugin-world-explorer/data/block-colors/`.
pub fn run(version: &str, root: &Path, target: &Path) -> Result {
    let work: PathBuf = target.join("block-colors").join(version);
    std::fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let manifest_file = work.join("manifest.json");
    curl(MANIFEST, &manifest_file)?;
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&manifest_file).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let url = manifest["versions"]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["id"] == version))
        .and_then(|entry| entry["url"].as_str())
        .ok_or_else(|| format!("Mojang lists no version {version}"))?
        .to_owned();
    let meta_file = work.join("version.json");
    curl(&url, &meta_file)?;
    let meta: Value =
        serde_json::from_slice(&std::fs::read(&meta_file).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let client = &meta["downloads"]["client"];
    let (jar_url, expected) = (
        client["url"].as_str().ok_or("no client download")?,
        client["sha1"].as_str().ok_or("no client sha1")?,
    );
    let jar_file = work.join("client.jar");
    if sha1_of(&jar_file).ok().as_deref() != Some(expected) {
        curl(jar_url, &jar_file)?;
    }
    let sha1 = sha1_of(&jar_file)?;
    if sha1 != expected {
        return Err(format!(
            "client jar sha1 {sha1} differs from Mojang's {expected}"
        ));
    }
    let file = std::fs::File::open(&jar_file).map_err(|error| error.to_string())?;
    let mut jar = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let table = build_table(&mut jar);
    if table.len() < 100 {
        return Err(format!(
            "only {} blocks found; the jar layout may have changed",
            table.len()
        ));
    }
    let data = root.join("crates/lumilio-plugin-world-explorer/data/block-colors");
    std::fs::create_dir_all(&data).map_err(|error| error.to_string())?;
    std::fs::write(data.join(format!("{version}.json")), render(&table))
        .map_err(|error| error.to_string())?;
    let source = data.join("SOURCE.md");
    let existing = std::fs::read_to_string(&source).unwrap_or_default();
    std::fs::write(
        &source,
        source_section(&existing, version, &sha1, &commit(root), table.len()),
    )
    .map_err(|error| error.to_string())?;
    println!("{version}: {} blocks", table.len());
    Ok(())
}
