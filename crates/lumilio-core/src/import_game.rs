//! Reading games made by other launchers (`ADR 0016`).
//!
//! Two layouts are understood: a MultiMC/Prism instance (`instance.cfg` and
//! `mmc-pack.json`, game files in `.minecraft` or `minecraft`) and a plain
//! `.minecraft` folder (the game and loader read from `versions/*/*.json`).
//! Reading never modifies the source. Behavior notes:
//! `docs/behavior/import-game.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::instance::Loader;

/// A game found in someone else's folder.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundGame {
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// The folder holding the game's own files (mods, saves, options…).
    pub game_dir: PathBuf,
    /// Where it was found, for the person to recognise.
    pub origin: &'static str,
}

#[derive(Debug)]
pub enum ImportError {
    Io(io::Error),
    /// The folder is neither layout.
    Unrecognised,
    /// The layout is right but the game or loader cannot be told from it.
    Unreadable(String),
}

impl Display for ImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "could not read the folder: {error}"),
            Self::Unrecognised => f.write_str("this folder is not a game from a known launcher"),
            Self::Unreadable(why) => write!(f, "could not tell which game this is: {why}"),
        }
    }
}

impl Error for ImportError {}

impl From<io::Error> for ImportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Deserialize)]
struct PackFile {
    components: Vec<Component>,
}

#[derive(Deserialize)]
struct Component {
    uid: String,
    #[serde(default)]
    version: Option<String>,
}

/// `key=value` lines of a simple settings file.
fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then_some(value.trim())
    })
}

fn read_prism(folder: &Path) -> Result<FoundGame, ImportError> {
    let config = fs::read_to_string(folder.join("instance.cfg"))?;
    let pack: PackFile = serde_json::from_str(&fs::read_to_string(folder.join("mmc-pack.json"))?)
        .map_err(|error| ImportError::Unreadable(error.to_string()))?;
    let version_of = |uid: &str| {
        pack.components
            .iter()
            .find(|component| component.uid == uid)
            .and_then(|component| component.version.clone())
    };
    let game_version = version_of("net.minecraft")
        .ok_or_else(|| ImportError::Unreadable("no Minecraft version".to_owned()))?;
    let (loader, loader_version) = [
        ("net.fabricmc.fabric-loader", Loader::Fabric),
        ("org.quiltmc.quilt-loader", Loader::Quilt),
        ("net.neoforged", Loader::NeoForge),
        ("net.minecraftforge", Loader::Forge),
    ]
    .into_iter()
    .find_map(|(uid, loader)| version_of(uid).map(|version| (loader, Some(version))))
    .unwrap_or((Loader::Vanilla, None));
    let game_dir = [".minecraft", "minecraft"]
        .into_iter()
        .map(|name| folder.join(name))
        .find(|dir| dir.is_dir())
        .ok_or_else(|| ImportError::Unreadable("the instance has no game folder".to_owned()))?;
    let fallback = folder.file_name().map_or_else(
        || "导入的游戏".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    Ok(FoundGame {
        name: setting(&config, "name")
            .filter(|name| !name.is_empty())
            .map_or(fallback, str::to_owned),
        game_version,
        loader,
        loader_version,
        game_dir,
        origin: "MultiMC / Prism",
    })
}

#[derive(Deserialize)]
struct VersionJson {
    id: String,
    #[serde(rename = "inheritsFrom", default)]
    inherits_from: Option<String>,
}

/// The game and loader a version id stands for, given what it inherits from.
fn read_version(id: &str, inherits: Option<&str>) -> Option<(String, Loader, Option<String>)> {
    if let Some(rest) = id.strip_prefix("fabric-loader-") {
        // fabric-loader-<loader>-<game>
        let game = inherits
            .map(str::to_owned)
            .or_else(|| rest.rsplit_once('-').map(|(_, g)| g.to_owned()))?;
        let loader = rest.strip_suffix(&format!("-{game}"))?.to_owned();
        return Some((game, Loader::Fabric, Some(loader)));
    }
    if let Some(rest) = id.strip_prefix("quilt-loader-") {
        let game = inherits
            .map(str::to_owned)
            .or_else(|| rest.rsplit_once('-').map(|(_, g)| g.to_owned()))?;
        let loader = rest.strip_suffix(&format!("-{game}"))?.to_owned();
        return Some((game, Loader::Quilt, Some(loader)));
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        return Some((
            inherits?.to_owned(),
            Loader::NeoForge,
            Some(version.to_owned()),
        ));
    }
    if let Some((game, version)) = id.split_once("-forge-") {
        return Some((game.to_owned(), Loader::Forge, Some(version.to_owned())));
    }
    if inherits.is_some() {
        // Modded in a way we cannot name.
        return None;
    }
    // A vanilla version id is a game version (release, snapshot, old).
    id.chars()
        .next()
        .filter(char::is_ascii_digit)
        .map(|_| (id.to_owned(), Loader::Vanilla, None))
        .or_else(|| {
            id.contains('w')
                .then(|| (id.to_owned(), Loader::Vanilla, None))
        })
}

fn read_plain(folder: &Path) -> Result<Vec<FoundGame>, ImportError> {
    let versions = folder.join("versions");
    let mut found = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&versions)?.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let dir = entry.path();
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let Ok(text) = fs::read_to_string(dir.join(format!("{name}.json"))) else {
            continue;
        };
        let Ok(version) = serde_json::from_str::<VersionJson>(&text) else {
            continue;
        };
        let Some((game_version, loader, loader_version)) =
            read_version(&version.id, version.inherits_from.as_deref())
        else {
            continue;
        };
        // Some launchers keep a version's own mods and saves beside its json.
        let isolated = ["mods", "saves", "config", "options.txt"]
            .iter()
            .any(|own| dir.join(own).exists());
        found.push(FoundGame {
            name: name.clone(),
            game_version,
            loader,
            loader_version,
            game_dir: if isolated { dir } else { folder.to_owned() },
            origin: "Minecraft 文件夹",
        });
    }
    if found.is_empty() {
        return Err(ImportError::Unreadable(
            "no version in it could be understood".to_owned(),
        ));
    }
    Ok(found)
}

/// What games a chosen folder holds. A MultiMC/Prism instance is one game; a
/// plain Minecraft folder has one per readable version.
pub fn detect(folder: &Path) -> Result<Vec<FoundGame>, ImportError> {
    if folder.join("instance.cfg").is_file() && folder.join("mmc-pack.json").is_file() {
        return Ok(vec![read_prism(folder)?]);
    }
    // The instances folder of Prism holds instances as sub-folders.
    if folder.join("versions").is_dir() {
        return read_plain(folder);
    }
    if folder.file_name().is_some_and(|name| name == ".minecraft")
        || folder.join(".minecraft").join("versions").is_dir()
    {
        let inner = if folder.join("versions").is_dir() {
            folder.to_owned()
        } else {
            folder.join(".minecraft")
        };
        return read_plain(&inner);
    }
    Err(ImportError::Unrecognised)
}

/// Top-level entries that belong to the other launcher or are volatile, not
/// to the game as a player has it.
const LEFT_BEHIND: [&str; 10] = [
    "versions",
    "libraries",
    "assets",
    "runtime",
    "logs",
    "crash-reports",
    "launcher_profiles.json",
    "launcher_accounts.json",
    "usercache.json",
    "natives",
];

/// Copies a found game's files into the (empty) folder `to`. The other
/// launcher's own folders, hidden entries and links are left behind. Returns
/// how many files were copied.
pub(crate) fn copy_game_files(
    game: &FoundGame,
    to: &Path,
    cancel: &crate::activity::CancellationToken,
) -> Result<u64, ImportError> {
    fn walk(
        from: &Path,
        to: &Path,
        top: bool,
        cancel: &crate::activity::CancellationToken,
        copied: &mut u64,
    ) -> Result<(), ImportError> {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let name = entry.file_name();
            let text = name.to_string_lossy();
            if top && (LEFT_BEHIND.contains(&text.as_ref()) || text.starts_with('.')) {
                continue;
            }
            if cancel.is_cancelled() {
                return Err(ImportError::Io(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "cancelled",
                )));
            }
            let kind = entry.file_type()?;
            let destination = to.join(&name);
            if kind.is_dir() {
                walk(&entry.path(), &destination, false, cancel, copied)?;
            } else if kind.is_file() {
                fs::copy(entry.path(), destination)?;
                *copied += 1;
            }
        }
        Ok(())
    }
    let mut copied = 0;
    walk(&game.game_dir, to, true, cancel, &mut copied)?;
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn a_prism_instance_is_read_with_its_name_game_and_loader() {
        let dir = tempfile::tempdir().unwrap();
        let instance = dir.path().join("inst");
        write(
            &instance.join("instance.cfg"),
            "InstanceType=OneSix\nname=我的生存\n",
        );
        write(
            &instance.join("mmc-pack.json"),
            r#"{"components":[
                {"uid":"org.lwjgl3","version":"3.3.1"},
                {"uid":"net.minecraft","version":"1.20.1"},
                {"uid":"net.fabricmc.fabric-loader","version":"0.15.7"}],"formatVersion":1}"#,
        );
        write(&instance.join(".minecraft/mods/a.jar"), "mod");
        let found = detect(&instance).unwrap();
        assert_eq!(found.len(), 1);
        let game = &found[0];
        assert_eq!(
            (game.name.as_str(), game.game_version.as_str(), game.loader),
            ("我的生存", "1.20.1", Loader::Fabric)
        );
        assert_eq!(game.loader_version.as_deref(), Some("0.15.7"));
        assert!(game.game_dir.ends_with(".minecraft"));

        // No game folder, or no Minecraft component, is unreadable.
        let bare = dir.path().join("bare");
        write(&bare.join("instance.cfg"), "name=x");
        write(&bare.join("mmc-pack.json"), r#"{"components":[]}"#);
        assert!(matches!(detect(&bare), Err(ImportError::Unreadable(_))));
    }

    #[test]
    fn version_ids_of_the_common_loaders_are_taken_apart() {
        let read = |id: &str, from: Option<&str>| read_version(id, from);
        assert_eq!(
            read("fabric-loader-0.15.7-1.20.1", Some("1.20.1")),
            Some(("1.20.1".into(), Loader::Fabric, Some("0.15.7".into())))
        );
        assert_eq!(
            read("quilt-loader-0.26.0-1.21.1", Some("1.21.1")),
            Some(("1.21.1".into(), Loader::Quilt, Some("0.26.0".into())))
        );
        assert_eq!(
            read("1.20.1-forge-47.2.0", Some("1.20.1")),
            Some(("1.20.1".into(), Loader::Forge, Some("47.2.0".into())))
        );
        assert_eq!(
            read("neoforge-21.1.50", Some("1.21.1")),
            Some(("1.21.1".into(), Loader::NeoForge, Some("21.1.50".into())))
        );
        assert_eq!(
            read("1.21.1", None),
            Some(("1.21.1".into(), Loader::Vanilla, None))
        );
        assert_eq!(
            read("23w31a", None),
            Some(("23w31a".into(), Loader::Vanilla, None))
        );
        assert_eq!(
            read("optifine-thing", Some("1.12.2")),
            None,
            "unnameable mods are not guessed"
        );
    }

    #[test]
    fn a_minecraft_folder_lists_each_readable_version_and_finds_isolated_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(".minecraft");
        write(
            &root.join("versions/1.21.1/1.21.1.json"),
            r#"{"id":"1.21.1"}"#,
        );
        write(
            &root.join("versions/fab/fab.json"),
            r#"{"id":"fabric-loader-0.16.0-1.21.1","inheritsFrom":"1.21.1"}"#,
        );
        write(&root.join("versions/fab/mods/a.jar"), "mod");
        write(&root.join("versions/junk/junk.json"), "not json");
        write(&root.join("saves/W/level.dat"), "world");
        let found = detect(&root).unwrap();
        let names: Vec<_> = found.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["1.21.1", "fab"]);
        assert_eq!(
            found[0].game_dir, root,
            "shared folder when nothing is isolated"
        );
        assert_eq!(found[1].game_dir, root.join("versions/fab"));
        assert_eq!(found[1].loader, Loader::Fabric);
        // The folder that contains .minecraft works too.
        assert_eq!(detect(dir.path()).unwrap().len(), 2);
        assert!(matches!(
            detect(&dir.path().join("nothing")),
            Err(ImportError::Unrecognised)
        ));
    }

    #[test]
    fn copying_takes_the_players_files_and_leaves_the_other_launchers_behind() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join(".minecraft");
        for path in [
            "mods/a.jar",
            "saves/W/level.dat",
            "options.txt",
            "libraries/x.jar",
            "assets/y",
            "versions/1.21.1/1.21.1.json",
            "logs/latest.log",
            ".hidden",
            "launcher_profiles.json",
        ] {
            write(&root.join(path), "x");
        }
        let game = FoundGame {
            name: "g".into(),
            game_version: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
            game_dir: root.clone(),
            origin: "t",
        };
        let into = dir.path().join("into");
        let copied =
            copy_game_files(&game, &into, &crate::activity::CancellationToken::new()).unwrap();
        assert_eq!(copied, 3);
        for kept in ["mods/a.jar", "saves/W/level.dat", "options.txt"] {
            assert!(into.join(kept).is_file(), "{kept}");
        }
        for left in [
            "libraries",
            "assets",
            "versions",
            "logs",
            ".hidden",
            "launcher_profiles.json",
        ] {
            assert!(!into.join(left).exists(), "{left}");
        }
        assert!(root.join("mods/a.jar").is_file(), "the source is untouched");
    }
}
