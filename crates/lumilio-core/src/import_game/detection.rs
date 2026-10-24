use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use serde::Deserialize;

use super::profile;
use crate::instance::Loader;

/// Which launcher's folder a game was found in, as a type rather than a
/// sentence: the interface turns it into words in the person's language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameOrigin {
    MultiMcPrism,
    MinecraftFolder,
}

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
    pub origin: GameOrigin,
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
pub(super) struct PackFile {
    pub(super) components: Vec<Component>,
}

#[derive(Deserialize)]
pub(super) struct Component {
    pub(super) uid: String,
    #[serde(default)]
    pub(super) version: Option<String>,
}

/// `key=value` lines of a simple settings file.
pub(super) fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name.trim() == key).then_some(value.trim())
    })
}

pub(super) fn read_prism(folder: &Path) -> Result<FoundGame, ImportError> {
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
        || "Imported game".to_owned(),
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
        origin: GameOrigin::MultiMcPrism,
    })
}

pub(super) fn read_plain(folder: &Path) -> Result<Vec<FoundGame>, ImportError> {
    let versions = folder.join("versions");
    let mut found = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(&versions)?.flatten().collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let dir = entry.path();
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some((game_version, loader, loader_version)) = profile::read(&versions, &name) else {
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
            origin: GameOrigin::MinecraftFolder,
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
pub(super) const LEFT_BEHIND: [&str; 10] = [
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
