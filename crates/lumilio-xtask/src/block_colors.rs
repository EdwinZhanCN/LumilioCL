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
/// Per biome: grass, foliage and water colour, each `0xRRGGBB`.
pub type Biomes = BTreeMap<String, [u32; 3]>;

/// Which texture of a model to show from above, most wanted first.
const FACES: [&str; 10] = [
    "top", "up", "all", "end", "side", "texture", "cross", "plant", "pattern", "particle",
];
const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

fn read_entry<R: Read + Seek>(jar: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let entry = jar.by_name(name).ok()?;
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
        // A weighted list of variants; the first stands for the block.
        let one = match value.as_array() {
            Some(list) => list.first()?,
            None => value,
        };
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
        | "sugar_cane" | "potted_fern" | "bush" => TINT_GRASS,
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

/// Blocks the game tints with one colour everywhere rather than by biome
/// (`BlockColors`: the evergreen and birch foliage colours, the lily pad's).
/// The tint is multiplied into the table's colour.
fn fixed_tint(block: &str) -> Option<[u8; 3]> {
    match block {
        "spruce_leaves" => Some([0x61, 0x99, 0x61]),
        "birch_leaves" => Some([0x80, 0xa7, 0x55]),
        "lily_pad" => Some([0x20, 0x80, 0x30]),
        _ => None,
    }
}

/// A colour in a biome file: an integer up to 1.21, `"#rrggbb"` from 26.x.
fn color_value(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|n| u32::try_from(n & 0xff_ffff).ok()),
        Value::String(text) => u32::from_str_radix(text.strip_prefix('#')?, 16).ok(),
        _ => None,
    }
}

/// The colour a 256×256 colormap gives a biome's temperature and downfall:
/// the map is indexed by temperature across and by downfall (scaled by
/// temperature) down, as the game samples it.
fn colormap(map: &image::RgbaImage, temperature: f64, downfall: f64) -> Option<u32> {
    let temperature = temperature.clamp(0.0, 1.0);
    let downfall = downfall.clamp(0.0, 1.0) * temperature;
    let x = ((1.0 - temperature) * 255.0) as u32;
    let y = ((1.0 - downfall) * 255.0) as u32;
    let pixel = map.get_pixel_checked(x, y)?;
    Some(u32::from(pixel[0]) << 16 | u32::from(pixel[1]) << 8 | u32::from(pixel[2]))
}

/// Grass, foliage and water colour of every biome the jar defines: the
/// biome's own colour where it names one, otherwise its spot on the grass or
/// foliage colormap. The swamp's grass is the drier of its two noise colours;
/// the dark forest's grass is darkened the way the game does.
pub fn build_biomes<R: Read + Seek>(jar: &mut zip::ZipArchive<R>) -> Biomes {
    let load = |jar: &mut zip::ZipArchive<R>, name: &str| {
        read_entry(
            jar,
            &format!("assets/minecraft/textures/colormap/{name}.png"),
        )
        .and_then(|png| image::load_from_memory_with_format(&png, image::ImageFormat::Png).ok())
        .map(|image| image.to_rgba8())
    };
    let (Some(grass_map), Some(foliage_map)) = (load(jar, "grass"), load(jar, "foliage")) else {
        return Biomes::new();
    };
    let names: Vec<String> = jar
        .file_names()
        .filter_map(|name| {
            name.strip_prefix("data/minecraft/worldgen/biome/")?
                .strip_suffix(".json")
                .filter(|name| !name.contains('/'))
                .map(str::to_owned)
        })
        .collect();
    let mut biomes = Biomes::new();
    for name in names {
        let Some(biome) = json(jar, &format!("data/minecraft/worldgen/biome/{name}.json")) else {
            continue;
        };
        let number = |key: &str| biome.get(key).and_then(Value::as_f64);
        let (Some(temperature), Some(downfall)) = (number("temperature"), number("downfall"))
        else {
            continue;
        };
        let effects = biome.get("effects").cloned().unwrap_or(Value::Null);
        let own = |key: &str| effects.get(key).and_then(color_value);
        let Some(mut grass) =
            own("grass_color").or_else(|| colormap(&grass_map, temperature, downfall))
        else {
            continue;
        };
        match effects.get("grass_color_modifier").and_then(Value::as_str) {
            Some("swamp") => grass = 0x6a7039,
            Some("dark_forest") => grass = ((grass & 0xfefefe) + 0x28340a) >> 1,
            _ => {}
        }
        let foliage = own("foliage_color")
            .or_else(|| colormap(&foliage_map, temperature, downfall))
            .unwrap_or(grass);
        let water = own("water_color").unwrap_or(0x3f76e4);
        biomes.insert(format!("minecraft:{name}"), [grass, foliage, water]);
    }
    biomes
}

/// The data version a client jar was built for, from its `version.json`.
pub fn data_version<R: Read + Seek>(jar: &mut zip::ZipArchive<R>) -> Option<i64> {
    json(jar, "version.json")?.get("world_version")?.as_i64()
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
        let Some((mut rgb, visible)) = average(&png) else {
            continue;
        };
        if let Some(tint) = fixed_tint(&block) {
            for (channel, tint) in rgb.iter_mut().zip(tint) {
                *channel = (u16::from(*channel) * u16::from(tint) / 255) as u8;
            }
        }
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
/// `version` and `data_version` say which game the table was built from; the
/// launcher picks a world's table by data version. Biome colours are written
/// as `[grass, foliage, water]`, each `0xRRGGBB` as a number.
pub fn render(table: &Table, biomes: &Biomes, version: &str, data_version: i64) -> String {
    let mut out = format!(
        "{{\n  \"version\": {},\n  \"data_version\": {data_version},\n  \"blocks\": {{\n",
        Value::from(version)
    );
    let last = table.len().saturating_sub(1);
    for (at, (block, [r, g, b, flags])) in table.iter().enumerate() {
        let comma = if at == last { "" } else { "," };
        let _ = writeln!(out, "    \"{block}\": [{r}, {g}, {b}, {flags}]{comma}");
    }
    out.push_str("  },\n  \"biomes\": {\n");
    let last = biomes.len().saturating_sub(1);
    for (at, (biome, [grass, foliage, water])) in biomes.iter().enumerate() {
        let comma = if at == last { "" } else { "," };
        let _ = writeln!(out, "    \"{biome}\": [{grass}, {foliage}, {water}]{comma}");
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
        "{heading}\n- Client jar sha1: `{sha1}` (from Mojang's version metadata)\n- Command: `cargo xtask block-colors {version}`\n- Tool commit: `{commit}`\n- Blocks: {blocks}\n- Colour: the mean of the visible pixels of the texture a block shows from above (`top`, then `up`, `all`, `end`, `side`); spruce and birch leaves and lily pads have the game's fixed tint multiplied in; flags 1 = see-through, 2 = grass tint, 4 = foliage tint, 8 = water tint; biomes give grass, foliage and water colour from the jar's biome files and colormaps\n\n"
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

/// The commit the tool runs at, marked `-dirty` when the tool's own sources
/// differ from it (the table would then not be reproducible from the commit).
fn commit(root: &Path) -> String {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
    };
    let head = git(&["rev-parse", "HEAD"])
        .map_or_else(|| "unknown".to_owned(), |text| text.trim().to_owned());
    match git(&["status", "--porcelain", "--", "crates/lumilio-xtask"]) {
        Some(changes) if !changes.trim().is_empty() => format!("{head}-dirty"),
        _ => head,
    }
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
    let biomes = build_biomes(&mut jar);
    if biomes.len() < 10 {
        return Err(format!(
            "only {} biomes found; the jar layout may have changed",
            biomes.len()
        ));
    }
    let data_version =
        data_version(&mut jar).ok_or("the client jar's version.json has no world_version")?;
    if table.len() < 100 {
        return Err(format!(
            "only {} blocks found; the jar layout may have changed",
            table.len()
        ));
    }
    let data = root.join("crates/lumilio-plugin-world-explorer/data/block-colors");
    std::fs::create_dir_all(&data).map_err(|error| error.to_string())?;
    std::fs::write(
        data.join(format!("{version}.json")),
        render(&table, &biomes, version, data_version),
    )
    .map_err(|error| error.to_string())?;
    let source = data.join("SOURCE.md");
    let existing = std::fs::read_to_string(&source).unwrap_or_default();
    std::fs::write(
        &source,
        source_section(&existing, version, &sha1, &commit(root), table.len()),
    )
    .map_err(|error| error.to_string())?;
    println!("{version}: {} blocks, {} biomes", table.len(), biomes.len());
    Ok(())
}
