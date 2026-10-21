//! What one version folder of a plain `.minecraft` is: its game version and
//! loader, read from the version json (and the jsons it inherits from) and,
//! when they say nothing, from the game jar.
//!
//! The version id is only a name. PCL and HMCL let the person choose it and
//! packs rename it, so it is asked last. Launchers keep the game version in
//! different places: PCL in `clientVersion`, HMCL in its `game` patch, Forge
//! and NeoForge in `--fml.mcVersion`, the official installers in
//! `inheritsFrom`. The loader is told from the libraries, not from the name.
//!
//! The library rules are adapted from HMCL
//! (`HMCLCore/src/main/java/org/jackhuang/hmcl/game/GameComponentType.java` and
//! `GameComponentAnalyzer.java`),
//! Copyright (C) 2026 huangyuhui and contributors, GPL-3.0-or-later.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::instance::Loader;

/// How deep an `inheritsFrom` chain is followed.
const MAX_CHAIN: usize = 8;

/// Main classes of the mod loaders; one of them without a library we can
/// read is a loader we cannot name.
const LOADER_MAIN_PACKAGES: [&str; 6] = [
    "net.minecraftforge",
    "net.neoforged",
    "top.outlands",
    "net.fabricmc",
    "org.quiltmc",
    "cpw.mods",
];

/// A game version inside a name: `1.21.1`, `26.3`, `1.21-pre1`,
/// `26.1-snapshot-1`, `24w14a`.
static GAME_VERSION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:^|[^0-9.])(\d{2}w\d{2}[a-z]|(?:1\.\d+(?:\.\d+)?|[2-9]\d\.\d+(?:\.\d+)?)(?:-(?:pre|rc|snapshot-)\d+)?)",
    )
    .expect("the game version pattern is valid")
});

/// A version json and the ones it inherits from, child first.
struct Chain {
    names: Vec<String>,
    jsons: Vec<Value>,
}

impl Chain {
    fn load(versions: &Path, name: &str) -> Option<Self> {
        let mut chain = Self {
            names: Vec::new(),
            jsons: Vec::new(),
        };
        let mut seen = BTreeSet::new();
        let mut next = Some(name.to_owned());
        while let Some(name) = next.take() {
            if chain.jsons.len() == MAX_CHAIN || !seen.insert(name.clone()) {
                break;
            }
            let path = versions.join(&name).join(format!("{name}.json"));
            let Some(json) = fs::read_to_string(path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .filter(Value::is_object)
            else {
                break;
            };
            next = text_of(&json, "inheritsFrom").map(str::to_owned);
            chain.names.push(name);
            chain.jsons.push(json);
        }
        (!chain.jsons.is_empty()).then_some(chain)
    }

    /// The first non-empty string `key` holds, child first.
    fn text(&self, key: &str) -> Option<&str> {
        self.jsons.iter().find_map(|json| text_of(json, key))
    }

    /// The game version the chain ends on: the last `inheritsFrom`.
    fn inherited_game(&self) -> Option<&str> {
        self.jsons
            .iter()
            .rev()
            .find_map(|json| text_of(json, "inheritsFrom"))
    }

    /// `(group, artifact, version)` of every library.
    fn libraries(&self) -> Vec<(&str, &str, &str)> {
        self.jsons
            .iter()
            .filter_map(|json| json.get("libraries")?.as_array())
            .flatten()
            .filter_map(|library| {
                let mut parts = library.get("name")?.as_str()?.split(':');
                Some((parts.next()?, parts.next()?, parts.next()?))
            })
            .collect()
    }

    /// HMCL's `patches`: `(id, version)`.
    fn patches(&self) -> Vec<(&str, &str)> {
        self.jsons
            .iter()
            .filter_map(|json| json.get("patches")?.as_array())
            .flatten()
            .filter_map(|patch| Some((patch.get("id")?.as_str()?, patch.get("version")?.as_str()?)))
            .collect()
    }

    /// The value after `flag` in the game arguments, modern or legacy.
    fn argument(&self, flag: &str) -> Option<&str> {
        self.jsons.iter().find_map(|json| {
            let modern = json
                .pointer("/arguments/game")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str);
            let legacy = json
                .get("minecraftArguments")
                .and_then(Value::as_str)
                .into_iter()
                .flat_map(str::split_whitespace);
            let words: Vec<&str> = modern.chain(legacy).collect();
            words
                .windows(2)
                .find(|pair| pair[0] == flag)
                .map(|pair| pair[1])
                .filter(|value| !value.is_empty())
        })
    }

    /// The `id` the game jar's own `version.json` gives.
    fn jar_game(&self, versions: &Path) -> Option<String> {
        let jars = self
            .jsons
            .iter()
            .filter_map(|json| text_of(json, "jar"))
            .chain(self.names.iter().map(String::as_str));
        jars.into_iter().find_map(|jar| {
            let file = fs::File::open(versions.join(jar).join(format!("{jar}.jar"))).ok()?;
            let mut archive = zip::ZipArchive::new(file).ok()?;
            let mut text = String::new();
            archive
                .by_name("version.json")
                .ok()?
                .take(64 * 1024)
                .read_to_string(&mut text)
                .ok()?;
            let json: Value = serde_json::from_str(&text).ok()?;
            // Early 1.14 builds write `1.14 / <hash>`.
            let id = text_of(&json, "id")?;
            Some(id.split(" / ").next().unwrap_or(id).to_owned())
        })
    }
}

fn text_of<'a>(json: &'a Value, key: &str) -> Option<&'a str> {
    json.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

/// The Forge build in a library version: `1.20.1-47.2.0` and
/// `1.7.10-10.13.4.1614-1.7.10` give `47.2.0` and `10.13.4.1614`; a version
/// that names no game is the build itself.
fn forge_build(version: &str) -> String {
    let mut parts = version.split('-');
    match (parts.next(), parts.next()) {
        (Some(_), Some(build)) if !build.is_empty() => build.to_owned(),
        _ => version.to_owned(),
    }
}

/// What a chain says about mods.
#[derive(Debug, Eq, PartialEq)]
enum Mods {
    Vanilla,
    Loader(Loader, String),
    /// A loader is there, but not one this launcher installs, or its version
    /// cannot be read.
    Unnamed,
}

fn mods(chain: &Chain) -> Mods {
    for (id, version) in chain.patches() {
        let loader = match id {
            "fabric" => Loader::Fabric,
            "quilt" => Loader::Quilt,
            "neoforge" => Loader::NeoForge,
            "forge" => Loader::Forge,
            _ => continue,
        };
        return Mods::Loader(loader, version.to_owned());
    }
    let libraries = chain.libraries();
    let library = |group: &str, artifacts: &[&str]| {
        libraries
            .iter()
            .find(|(g, a, _)| *g == group && artifacts.contains(a))
            .map(|(_, _, version)| *version)
    };
    let legacy_fabric = libraries.iter().any(|(g, _, _)| *g == "net.legacyfabric");
    if let Some(version) = library("org.quiltmc", &["quilt-loader"]) {
        return Mods::Loader(Loader::Quilt, version.to_owned());
    }
    if !legacy_fabric && let Some(version) = library("net.fabricmc", &["fabric-loader"]) {
        return Mods::Loader(Loader::Fabric, version.to_owned());
    }
    if library("net.neoforged.fancymodloader", &["core", "loader"]).is_some() {
        let version = chain
            .argument("--fml.neoForgeVersion")
            .or_else(|| chain.argument("--fml.forgeVersion"))
            .map(str::to_owned)
            .or_else(|| library("net.neoforged", &["neoforge"]).map(str::to_owned))
            .or_else(|| library("net.neoforged", &["forge"]).map(forge_build));
        return version.map_or(Mods::Unnamed, |version| {
            Mods::Loader(Loader::NeoForge, version)
        });
    }
    if let Some(version) = library(
        "net.minecraftforge",
        &["forge", "fmlloader", "minecraftforge"],
    ) {
        let version = chain
            .argument("--fml.forgeVersion")
            .map_or_else(|| forge_build(version), str::to_owned);
        return Mods::Loader(Loader::Forge, version);
    }
    let unsupported = legacy_fabric
        || library("com.cleanroommc", &["cleanroom"]).is_some()
        || library("com.mumfrey", &["liteloader"]).is_some()
        || libraries.iter().any(|(g, a, _)| {
            matches!(*g, "optifine" | "net.optifine") && !a.contains("launchwrapper")
        });
    let loader_main = chain.text("mainClass").is_some_and(|main| {
        LOADER_MAIN_PACKAGES
            .iter()
            .any(|package| main.starts_with(package))
    });
    let tweaked = chain
        .argument("--tweakClass")
        .is_some_and(|tweak| !tweak.starts_with("net.minecraft.launchwrapper."));
    // A json that inherits and adds nothing we can name is modded all the same.
    let inherits = text_of(&chain.jsons[0], "inheritsFrom").is_some();
    if unsupported || loader_main || tweaked || inherits {
        Mods::Unnamed
    } else {
        Mods::Vanilla
    }
}

/// The game version a name carries, for a folder that says it nowhere else.
/// A loader version inside the name (`fabric-loader-0.16.0-1.21.1`) is not
/// one: it starts with `0.` or follows a dot.
fn game_in_name(id: &str) -> Option<String> {
    GAME_VERSION
        .captures(id)
        .and_then(|found| found.get(1))
        .map(|found| found.as_str().to_owned())
}

fn game_version(chain: &Chain, versions: &Path, name: &str) -> Option<String> {
    let patch = || {
        chain
            .patches()
            .into_iter()
            .find(|(id, _)| *id == "game")
            .map(|(_, version)| version)
    };
    let libraries = chain.libraries();
    let from_library = || {
        libraries
            .iter()
            .find_map(|(group, artifact, version)| match (*group, *artifact) {
                ("net.fabricmc" | "org.quiltmc", "intermediary") => Some((*version).to_owned()),
                ("net.minecraftforge", "forge" | "fmlloader") => {
                    version.split_once('-').map(|(game, _)| game.to_owned())
                }
                _ => None,
            })
    };
    chain
        .text("clientVersion")
        .or_else(patch)
        .or_else(|| chain.argument("--fml.mcVersion"))
        .or_else(|| chain.inherited_game())
        .map(str::to_owned)
        .or_else(|| chain.jar_game(versions))
        .or_else(from_library)
        .or_else(|| chain.text("id").and_then(game_in_name))
        .or_else(|| game_in_name(name))
        .filter(|game| !game.is_empty() && !game.contains(char::is_whitespace))
}

/// The game version, loader and loader version of `versions/<name>`, or
/// `None` when the folder holds no readable version or a loader this launcher
/// cannot install.
pub(super) fn read(versions: &Path, name: &str) -> Option<(String, Loader, Option<String>)> {
    let chain = Chain::load(versions, name)?;
    let (loader, loader_version) = match mods(&chain) {
        Mods::Vanilla => (Loader::Vanilla, None),
        Mods::Loader(loader, version) => (loader, Some(version)),
        Mods::Unnamed => return None,
    };
    Some((
        game_version(&chain, versions, name)?,
        loader,
        loader_version,
    ))
}
