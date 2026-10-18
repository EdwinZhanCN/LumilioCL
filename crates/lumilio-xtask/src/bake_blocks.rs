//! `cargo xtask bake-blocks`: the baked block assets of every release from
//! 1.13.2 to the latest, in one `blocks.dat.z` (plan schematic-studio, T2/D2).
//!
//! Mojang's version manifest names the releases; each client jar is
//! downloaded into the ignored `target/block-assets/` (kept as a cache and
//! skipped when its sha1 already matches), checked against the sha1 in
//! Mojang's version metadata and baked with `lumilio_block_assets::bake`.
//! The packs go into one bundle keyed by version id, so an image several
//! versions share is stored once. Beside it: `blocks.json`, the index the
//! launcher downloads first, `SOURCE.md`, the provenance record, and
//! `NOTICE`, the license note published with the bundle (D2). Nothing
//! here is committed. Entity geometry goes beside it in `entities.dat.z`
//! and the same index (T6, see `entity_dump.rs`).
//!
//! The game's names for blocks, items and entities go in `names.dat.z` (T26,
//! D28): `en_us` from each client jar, `zh_cn` from each release's asset
//! index (both sha1-checked), merged newest first.
//!
//! While baking, adjacent versions are compared: an `entity/` texture that
//! disappears while a new `entity/` texture with the same pixels appears is a
//! move the rename table (`crates/lumilio-block-assets/data/texture_renames.json`)
//! does not cover yet, and is reported.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use lumilio_block_assets::{
    AssetSource, BUNDLE_FORMAT, BakedPack, BakedTexture, BlockNames, Bundle, ContentHash,
    ENTITY_FORMAT, EntityBundle, NAME_LANGUAGES, NAMES_FORMAT, bake, names_in_lang_file,
    write_bundle,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::block_colors::{MANIFEST, commit, curl, sha1_of};
use crate::entity_dump::{ENTITIES, EntityBake, EntityJar, FIRST_DUMPED, bake_entities};
use crate::release::Result;

/// The oldest release baked: the first after the flattening whose blockstate
/// and model formats the baker reads.
pub const FIRST: &str = "1.13.2";
pub const BUNDLE: &str = "blocks.dat.z";
pub const INDEX: &str = "blocks.json";
/// T26: the names of blocks, items and entities, per language.
pub const NAMES: &str = "names.dat.z";
/// Where Mojang serves the objects an asset index lists.
const RESOURCES: &str = "https://resources.download.minecraft.net/";
/// D2: the license note published beside the bundle.
pub const NOTICE: &str = "NOTICE";
/// D2: above this size the bundle is split into one file per version.
pub const SLICE_THRESHOLD: u64 = 40 * 1024 * 1024;
const ENTITY: &str = "minecraft:entity/";
/// Folders whose changes make the bake differ from the commit's.
const TOOL_SOURCES: [&str; 3] = [
    "crates/lumilio-xtask",
    "crates/lumilio-block-assets",
    "tools/entity-dump",
];

/// A release as Mojang's manifest lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRelease {
    pub id: String,
    /// The version metadata (the file naming the client jar).
    pub url: String,
    /// SHA-1 of the version metadata.
    pub sha1: String,
}

/// One version in the index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BakedVersion {
    pub id: String,
    pub data_version: u32,
    pub jar_sha1: String,
}

/// The baked `names.dat.z` and how many names each language has.
pub struct NameBake {
    pub bytes: Vec<u8>,
    pub counts: Vec<(String, usize)>,
}

/// The releases from `first` to the newest, oldest first. Snapshots,
/// pre-releases and old alphas and betas are left out; the order is Mojang's
/// release time, not the order of the list.
pub fn releases_since(manifest: &Value, first: &str) -> Result<Vec<GameRelease>> {
    let list = manifest["versions"]
        .as_array()
        .ok_or("the version manifest lists no versions")?;
    let mut releases: Vec<(&str, GameRelease)> = list
        .iter()
        .filter(|entry| entry["type"] == "release")
        .map(|entry| {
            let field = |key: &str| {
                entry[key]
                    .as_str()
                    .ok_or_else(|| format!("a release in the version manifest has no {key}"))
            };
            Ok((
                field("releaseTime")?,
                GameRelease {
                    id: field("id")?.to_owned(),
                    url: field("url")?.to_owned(),
                    sha1: field("sha1")?.to_owned(),
                },
            ))
        })
        .collect::<Result<_>>()?;
    releases.sort_by(|a, b| a.0.cmp(b.0));
    let start = releases
        .iter()
        .position(|(_, release)| release.id == first)
        .ok_or_else(|| format!("the version manifest lists no release {first}"))?;
    Ok(releases
        .into_iter()
        .skip(start)
        .map(|(_, release)| release)
        .collect())
}

/// `entity/` textures that `older` has and `newer` lacks, paired with the
/// textures `newer` adds that have the same pixels: moves the rename table
/// does not settle yet. Both maps hold canonical names (as baked), so a move
/// the table covers is not reported, and one it maps to the wrong name is.
pub fn moved_textures(
    older: &BTreeMap<String, BakedTexture>,
    newer: &BTreeMap<String, BakedTexture>,
) -> Vec<(String, String)> {
    let entity = |id: &&String| id.starts_with(ENTITY);
    let mut added: BTreeMap<ContentHash, Vec<&str>> = BTreeMap::new();
    for (id, texture) in newer {
        if entity(&id) && !older.contains_key(id) {
            added.entry(texture.image).or_default().push(id);
        }
    }
    let mut moves = Vec::new();
    for (id, texture) in older {
        if !entity(&id) || newer.contains_key(id) {
            continue;
        }
        for new in added.get(&texture.image).into_iter().flatten() {
            moves.push((id.clone(), (*new).to_owned()));
        }
    }
    moves
}

/// `blocks.json`: what the launcher reads before it downloads the bundle.
pub fn render_index(
    versions: &[BakedVersion],
    bundle: &[u8],
    tool_commit: &str,
    entities: Option<&EntityBake>,
    names: Option<&NameBake>,
) -> String {
    let mut index = json!({
        "format": BUNDLE_FORMAT,
        "file": BUNDLE,
        "size": bundle.len(),
        "sha256": sha256_hex(bundle),
        "tool_commit": tool_commit,
        "versions": versions
            .iter()
            .map(|version| json!({
                "id": version.id,
                "data_version": version.data_version,
                "jar_sha1": version.jar_sha1,
            }))
            .collect::<Vec<_>>(),
    });
    if let Some(entities) = entities {
        index["entities"] = json!({
            "format": ENTITY_FORMAT,
            "file": ENTITIES,
            "size": entities.bytes.len(),
            "sha256": sha256_hex(&entities.bytes),
            "layers": entities.layers,
            "versions": entities
                .versions
                .iter()
                .map(|(id, from)| match from {
                    Some(from) => json!({ "id": id, "geometry_from": from }),
                    None => json!({ "id": id }),
                })
                .collect::<Vec<_>>(),
        });
    }
    if let Some(names) = names {
        index["names"] = json!({
            "format": NAMES_FORMAT,
            "file": NAMES,
            "size": names.bytes.len(),
            "sha256": sha256_hex(&names.bytes),
            "languages": names
                .counts
                .iter()
                .map(|(language, count)| (language.clone(), json!(count)))
                .collect::<serde_json::Map<_, _>>(),
        });
    }
    let mut text = serde_json::to_string_pretty(&index).unwrap_or_default();
    text.push('\n');
    text
}

/// `SOURCE.md` beside the bundle: where every pack came from and how to
/// rebuild it.
pub fn render_source(
    versions: &[BakedVersion],
    bundle: &[u8],
    images: usize,
    tool_commit: &str,
    entities: Option<&EntityBake>,
    names: Option<&NameBake>,
) -> String {
    let mut out = format!(
        "# Baked block assets\n\nDerived from Mojang's client jars; regenerate with the command below. Neither the jars nor the bundle are kept in the repository.\n\n- Command: `cargo xtask bake-blocks`\n- Tool commit: `{tool_commit}`\n- Bundle: `{BUNDLE}`, container format {BUNDLE_FORMAT}, {} bytes, sha256 `{}`\n- Versions: {}\n- Distinct images: {images}\n- Contents: blockstates, the models they use with parents applied, block, colormap and `entity/` textures as RGBA8 deduplicated by pixels\n\n| Version | Data version | Client jar sha1 (from Mojang's version metadata) |\n| --- | --- | --- |\n",
        bundle.len(),
        sha256_hex(bundle),
        versions.len(),
    );
    for version in versions {
        let _ = writeln!(
            out,
            "| {} | {} | `{}` |",
            version.id, version.data_version, version.jar_sha1
        );
    }
    if let Some(entities) = entities {
        let _ = write!(
            out,
            "\n## Entity geometry\n\n- File: `{ENTITIES}`, entity format {ENTITY_FORMAT}, {} bytes, sha256 `{}`\n- Made by: `tools/entity-dump` (calls the game's `LayerDefinitions.createRoots()`), run once per release from {FIRST_DUMPED} on with the client jar and its libraries (from Mojang's version metadata, sha1-checked); releases before {FIRST_DUMPED} have no layer definitions and reuse {FIRST_DUMPED}'s geometry\n- Obfuscated releases: class and member names from Mojang's official `client_mappings` for that release, read at bake time only; the mappings are not redistributed\n- Distinct layers: {}\n\n| Version | Geometry |\n| --- | --- |\n",
            entities.bytes.len(),
            sha256_hex(&entities.bytes),
            entities.layers,
        );
        for (id, from) in &entities.versions {
            let geometry = match from {
                Some(from) => format!("same as {from}"),
                None => entities
                    .runtimes
                    .iter()
                    .find(|(version, _)| version == id)
                    .map_or_else(
                        || "dumped".to_owned(),
                        |(_, java)| format!("dumped with {java}"),
                    ),
            };
            let _ = writeln!(out, "| {id} | {geometry} |");
        }
        for (id, reason) in &entities.failed {
            let _ = writeln!(out, "| {id} | not included: {reason} |");
        }
    }
    if let Some(names) = names {
        let counts: Vec<String> = names
            .counts
            .iter()
            .map(|(language, count)| format!("{language} {count}"))
            .collect();
        let _ = write!(
            out,
            "\n## Names\n\n- File: `{NAMES}`, names format {NAMES_FORMAT}, {} bytes, sha256 `{}`\n- Made from: `assets/minecraft/lang/en_us.json` in each client jar, and `minecraft/lang/zh_cn.json` from each release's asset index (objects checked against the sha1 the index names); only `block.*`, `item.*` and `entity.*` keys of three parts, newest release first\n- Names per language: {}\n",
            names.bytes.len(),
            sha256_hex(&names.bytes),
            counts.join(", "),
        );
    }
    out
}

/// `NOTICE` beside the bundle (D2): what the published files contain, whose
/// they are, and what is not redistributed.
pub fn render_notice(entities: bool, names: bool) -> String {
    let mut out = format!(
        "LumilioCL baked block assets\n\n`{BUNDLE}` contains block states, block models, textures and colormaps\nextracted from Minecraft client jars published by Mojang Studios. These\nassets are copyright Mojang AB / Mojang Studios and are not covered by\nLumilioCL's license. They are redistributed only so that the LumilioCL\nlauncher can preview schematics when the matching game version is not\ninstalled, under the Minecraft End User License Agreement and Usage\nGuidelines (https://www.minecraft.net/en-us/eula,\nhttps://www.minecraft.net/en-us/usage-guidelines).\nNOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH\nMOJANG OR MICROSOFT.\n"
    );
    if entities {
        let _ = write!(
            out,
            "\n`{ENTITIES}` contains entity model geometry exported from the same\nclient jars by `tools/entity-dump`. It is Mojang's data under the same\nterms. Mojang's official obfuscation mappings (`client_mappings`) were read\nonly while baking and are not redistributed.\n"
        );
    }
    if names {
        let _ = write!(
            out,
            "\n`{NAMES}` contains the names of blocks, items and entities taken from\nthe game's language files (English from the client jars, Simplified\nChinese from Mojang's asset objects). They are Mojang's text under the\nsame terms.\n"
        );
    }
    out.push_str(
        "\n`blocks.json` and `SOURCE.md` record the version of each client jar and\nits sha1 from Mojang's version metadata, and the command that rebuilds\nthese files: `cargo xtask bake-blocks`. The tool that made them is\nLumilioCL (AGPL-3.0-only); its license does not extend to the assets.\n",
    );
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Downloads `url` to `to` unless `to` already has the sha1 `expected`.
pub(crate) fn fetch(url: &str, to: &Path, expected: &str) -> Result {
    if sha1_of(to).ok().as_deref() == Some(expected) {
        return Ok(());
    }
    curl(url, to)?;
    let sha1 = sha1_of(to)?;
    if sha1 != expected {
        return Err(format!(
            "{}: sha1 {sha1} differs from Mojang's {expected}",
            to.display()
        ));
    }
    Ok(())
}

pub(crate) fn read_json(path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))
}

struct Jar {
    id: String,
    path: PathBuf,
    sha1: String,
    /// The release's asset index: its id, address and sha1.
    assets: Option<(String, String, String)>,
}

/// The release's client jar in the cache, checked against Mojang's sha1.
fn client_jar(release: &GameRelease, work: &Path) -> Result<Jar> {
    let dir = work.join("versions").join(&release.id);
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let meta_file = dir.join("version.json");
    fetch(&release.url, &meta_file, &release.sha1)?;
    let meta = read_json(&meta_file)?;
    let client = &meta["downloads"]["client"];
    let (url, sha1) = (
        client["url"].as_str().ok_or("no client download")?,
        client["sha1"].as_str().ok_or("no client sha1")?,
    );
    let path = dir.join("client.jar");
    fetch(url, &path, sha1)?;
    let index = &meta["assetIndex"];
    let assets = match (
        index["id"].as_str(),
        index["url"].as_str(),
        index["sha1"].as_str(),
    ) {
        (Some(id), Some(url), Some(sha1)) => Some((id.to_owned(), url.to_owned(), sha1.to_owned())),
        _ => None,
    };
    Ok(Jar {
        id: release.id.clone(),
        path,
        sha1: sha1.to_owned(),
        assets,
    })
}

/// A language file of `jar`'s release: `en_us` from the jar itself, any
/// other from its asset index (cached under `work/assets/` and checked
/// against the sha1 the index names). `None` when the release has none.
fn lang_file(jar: &Jar, language: &str, work: &Path) -> Result<Option<Vec<u8>>> {
    if language == "en_us" {
        let file = std::fs::File::open(&jar.path).map_err(|error| error.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
        let mut entry = match archive.by_name("assets/minecraft/lang/en_us.json") {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).map_err(|error| error.to_string())?;
        return Ok(Some(bytes));
    }
    let Some((id, url, sha1)) = &jar.assets else {
        return Ok(None);
    };
    let indexes = work.join("assets").join("indexes");
    std::fs::create_dir_all(&indexes).map_err(|error| error.to_string())?;
    let index_file = indexes.join(format!("{id}.json"));
    fetch(url, &index_file, sha1)?;
    let index = read_json(&index_file)?;
    let Some(hash) = index["objects"][format!("minecraft/lang/{language}.json")]["hash"].as_str()
    else {
        return Ok(None);
    };
    if hash.len() < 2 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("{id}: asset index names a bad hash {hash}"));
    }
    let objects = work.join("assets").join("objects").join(&hash[..2]);
    std::fs::create_dir_all(&objects).map_err(|error| error.to_string())?;
    let object = objects.join(hash);
    fetch(&format!("{RESOURCES}{}/{hash}", &hash[..2]), &object, hash)?;
    std::fs::read(&object)
        .map(Some)
        .map_err(|error| format!("{}: {error}", object.display()))
}

/// `names.dat.z`: every release's language files, newest release first.
fn bake_names(jars: &[Jar], work: &Path) -> Result<NameBake> {
    let mut names = BlockNames::default();
    for jar in jars.iter().rev() {
        for language in NAME_LANGUAGES {
            if let Some(bytes) = lang_file(jar, language, work)? {
                let file = names_in_lang_file(&bytes)
                    .map_err(|error| format!("{} {language}: {error}", jar.id))?;
                names.add(language, file);
            }
        }
    }
    let counts = NAME_LANGUAGES
        .iter()
        .map(|language| ((*language).to_owned(), names.len(language)))
        .collect();
    let bytes = names.to_bytes().map_err(|error| error.to_string())?;
    Ok(NameBake { bytes, counts })
}

/// Bakes every jar on a few threads. Each pack keeps only the images no
/// earlier-arriving pack holds: the bundle stores the union once, and 49
/// packs each holding every image would not fit in memory.
fn bake_all(jars: &[Jar]) -> Result<Vec<BakedPack>> {
    let workers = std::thread::available_parallelism()
        .map_or(4, usize::from)
        .clamp(1, 8);
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..workers {
            let (next, sender) = (&next, sender.clone());
            scope.spawn(move || {
                loop {
                    let at = next.fetch_add(1, Ordering::Relaxed);
                    let Some(jar) = jars.get(at) else { break };
                    let baked = bake(&[AssetSource::Archive(jar.path.clone())])
                        .map_err(|error| format!("{}: {error}", jar.id));
                    if sender.send((at, baked)).is_err() {
                        break;
                    }
                }
            });
        }
        drop(sender);
        let mut seen = BTreeSet::new();
        let mut packs: Vec<Option<BakedPack>> = jars.iter().map(|_| None).collect();
        for (at, baked) in receiver {
            let mut pack = baked?;
            pack.assets.images.retain(|hash, _| seen.insert(*hash));
            eprintln!(
                "baked {}: {} blockstates, {} models, {} textures, {} skipped",
                jars[at].id,
                pack.assets.blockstates.len(),
                pack.assets.models.len(),
                pack.assets.textures.len(),
                pack.meta.skipped.len(),
            );
            packs[at] = Some(pack);
        }
        Ok(packs.into_iter().flatten().collect())
    })
}

/// Downloads, bakes and writes the bundle, its index and its provenance
/// under `target/block-assets/`.
pub fn run(root: &Path, target: &Path) -> Result {
    let started = Instant::now();
    let work = target.join("block-assets");
    std::fs::create_dir_all(&work).map_err(|error| error.to_string())?;
    let manifest_file = work.join("version_manifest_v2.json");
    curl(MANIFEST, &manifest_file)?;
    let releases = releases_since(&read_json(&manifest_file)?, FIRST)?;
    eprintln!(
        "{} releases from {FIRST} to {}",
        releases.len(),
        releases.last().map_or("", |release| release.id.as_str())
    );
    let jars = releases
        .iter()
        .map(|release| client_jar(release, &work))
        .collect::<Result<Vec<_>>>()?;
    let downloaded = started.elapsed();

    let baking = Instant::now();
    let mut packs = bake_all(&jars)?;
    let mut versions = Vec::new();
    for (jar, pack) in jars.iter().zip(&mut packs) {
        // 1.13.2 has no version.json; its data version is Mojang's published one.
        if pack.meta.data_version.is_none() && jar.id == FIRST {
            pack.meta.data_version = Some(1631);
        }
        pack.meta.game_version.get_or_insert_with(|| jar.id.clone());
        let data_version = pack
            .meta
            .data_version
            .ok_or_else(|| format!("{}: the client jar names no data version", jar.id))?;
        versions.push(BakedVersion {
            id: jar.id.clone(),
            data_version,
            jar_sha1: jar.sha1.clone(),
        });
    }
    let bundle = write_bundle(jars.iter().map(|jar| jar.id.as_str()).zip(packs.iter()))
        .map_err(|error| error.to_string())?;
    let baked = baking.elapsed();

    let images = Bundle::read(&bundle)
        .map_err(|error| error.to_string())?
        .image_count();
    let entity_jars: Vec<EntityJar> = jars
        .iter()
        .map(|jar| EntityJar {
            id: &jar.id,
            jar: &jar.path,
        })
        .collect();
    // `minecraft:textures/entity/bee/bee.png` is the pack's `minecraft:entity/bee/bee`.
    let has = |version: &str, texture: &str| {
        let id = texture
            .replacen("textures/", "", 1)
            .trim_end_matches(".png")
            .to_owned();
        jars.iter()
            .zip(&packs)
            .find(|(jar, _)| jar.id == version)
            .is_some_and(|(_, pack)| pack.assets.textures.contains_key(&id))
    };
    let entities = bake_entities(&work, &entity_jars, &has)?;
    let names = bake_names(&jars, &work)?;
    let tool_commit = commit(root, &TOOL_SOURCES);
    let write = |name: &str, bytes: &[u8]| {
        std::fs::write(work.join(name), bytes).map_err(|error| format!("{name}: {error}"))
    };
    write(BUNDLE, &bundle)?;
    write(ENTITIES, &entities.bytes)?;
    write(NAMES, &names.bytes)?;
    write(
        INDEX,
        render_index(
            &versions,
            &bundle,
            &tool_commit,
            Some(&entities),
            Some(&names),
        )
        .as_bytes(),
    )?;
    write(
        "SOURCE.md",
        render_source(
            &versions,
            &bundle,
            images,
            &tool_commit,
            Some(&entities),
            Some(&names),
        )
        .as_bytes(),
    )?;
    write(NOTICE, render_notice(true, true).as_bytes())?;
    for (id, reason) in &entities.failed {
        println!("no entity geometry for {id}: {reason}");
    }

    for (version, pack) in versions.iter().zip(&packs) {
        for skipped in &pack.meta.skipped {
            println!("skipped in {}: {skipped}", version.id);
        }
    }
    let mut moves = 0;
    for (pair, versions) in packs.windows(2).zip(versions.windows(2)) {
        for (old, new) in moved_textures(&pair[0].assets.textures, &pair[1].assets.textures) {
            println!(
                "moved between {} and {}: {old} -> {new} (not in texture_renames.json)",
                versions[0].id, versions[1].id
            );
            moves += 1;
        }
    }
    let size = bundle.len() as u64;
    println!(
        "{} versions, {size} bytes ({:.1} MiB{}), {images} distinct images, {moves} unsettled texture moves; download {:.0}s, bake {:.0}s",
        versions.len(),
        size as f64 / (1024.0 * 1024.0),
        if size > SLICE_THRESHOLD {
            ", over the D2 threshold"
        } else {
            ""
        },
        downloaded.as_secs_f64(),
        baked.as_secs_f64(),
    );
    println!(
        "entities: {} versions ({} dumped from {FIRST_DUMPED} on), {} bytes, {} distinct layers, {} failed; dump {:.0}s",
        entities.versions.len(),
        entities
            .versions
            .iter()
            .filter(|(_, from)| from.is_none())
            .count(),
        entities.bytes.len(),
        entities.layers,
        entities.failed.len(),
        entities.elapsed.as_secs_f64(),
    );
    let counts: Vec<String> = names
        .counts
        .iter()
        .map(|(language, count)| format!("{language} {count}"))
        .collect();
    println!("names: {} bytes ({})", names.bytes.len(), counts.join(", "));
    println!("{}", work.join(BUNDLE).display());
    println!("{}", work.join(ENTITIES).display());
    println!("{}", work.join(NAMES).display());
    entity_coverage(&packs, &versions, &entities.bytes)
}

/// Every entity type of every baked version is drawn, or is one the game
/// draws nothing for (plan schematic-studio T35, D32): what is not fails the
/// bake, after its files are written, named with the versions it is missing
/// from.
fn entity_coverage(packs: &[BakedPack], versions: &[BakedVersion], entities: &[u8]) -> Result {
    let bundle = EntityBundle::read(entities).map_err(|error| error.to_string())?;
    let mut gaps: BTreeMap<(String, &str), Vec<&str>> = BTreeMap::new();
    for (version, pack) in versions.iter().zip(packs) {
        let pack = schematic_mesher::ResourcePack::from_baked(pack)
            .with_entity_layers(bundle.layers_of(&version.id));
        let missing =
            schematic_mesher::mesher::entity::bindings::coverage(&pack, &pack.entity_looks);
        for (id, why) in missing {
            gaps.entry((id, why)).or_default().push(&version.id);
        }
    }
    if gaps.is_empty() {
        println!("entity coverage: every entity type of every version is drawn");
        return Ok(());
    }
    for ((id, why), versions) in &gaps {
        println!(
            "not drawn: {id}: {why} ({} versions, {} to {})",
            versions.len(),
            versions[0],
            versions[versions.len() - 1]
        );
    }
    Err(format!(
        "{} entity types are drawn by nothing (rows in forks/schematic-mesher/data/entity_bindings.json)",
        gaps.len()
    ))
}
