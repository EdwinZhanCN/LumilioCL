//! Saved worlds in an instance.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::discover::is_safe_file_name;
use crate::nbt;

const SAVES: &str = "saves";
const LEVEL_FILE: &str = "level.dat";
const ICON_FILE: &str = "icon.png";
/// A copy in progress lives beside its target under `.<name>.copying`; hidden
/// folders are never listed, so a crash cannot leave a world that looks real.
const COPYING_SUFFIX: &str = ".copying";
/// Largest `level.dat` read.
const LEVEL_LIMIT: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldInfo {
    /// The folder name under `saves/` (the world's identity).
    pub folder: String,
    /// The name the player gave it; the folder name when unreadable.
    pub name: String,
    /// Milliseconds since the Unix epoch, as the game stores it.
    pub last_played_ms: Option<i64>,
    pub game_version: Option<String>,
    pub hardcore: bool,
    /// The world's cover, when the player chose one in the game
    /// (`saves/<folder>/icon.png`).
    pub icon: Option<PathBuf>,
    /// `level.dat` exists but could not be understood.
    pub damaged: bool,
    /// When the game last touched `session.lock` (ms since the Unix epoch).
    /// A game writes it when it opens the world, so while a game runs, the
    /// world with the newest one is the one being played.
    pub lock_touched_ms: Option<i64>,
}

#[derive(Debug)]
pub enum WorldError {
    Io(io::Error),
    UnsafeName(String),
    NotFound(String),
    AlreadyExists(String),
    /// `saves/` or the world itself is a link, so it may lead outside the instance.
    Linked(String),
    /// The archive holds no folder with a `level.dat`.
    NotAWorld,
    /// The archive is unreadable or unreasonably large.
    BadArchive(String),
}

impl Display for WorldError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "world storage failed: {error}"),
            Self::UnsafeName(name) => write!(f, "unsafe world name {name:?}"),
            Self::NotFound(name) => write!(f, "no world {name:?}"),
            Self::AlreadyExists(name) => write!(f, "world {name:?} already exists"),
            Self::Linked(name) => {
                write!(f, "{name:?} is a link; it could lead outside the instance")
            }
            Self::NotAWorld => f.write_str("the archive does not contain a world"),
            Self::BadArchive(why) => write!(f, "unusable world archive: {why}"),
        }
    }
}

impl Error for WorldError {}

impl From<io::Error> for WorldError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn read_level(path: &Path) -> Option<nbt::Tag> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > LEVEL_LIMIT {
        return None;
    }
    nbt::parse_maybe_gzip(&fs::read(path).ok()?).ok()
}

/// Lists worlds — folders under `saves/` holding a `level.dat` — most recently
/// played first, then by name. A missing `saves/` is an empty list.
pub fn scan(game_dir: &Path) -> Result<Vec<WorldInfo>, WorldError> {
    let entries = match fs::read_dir(saves_dir(game_dir)?) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut worlds = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(folder) = entry.file_name().into_string() else {
            continue;
        };
        let path = entry.path();
        let level = path.join(LEVEL_FILE);
        if folder.starts_with('.') || !entry.file_type()?.is_dir() || !level.is_file() {
            continue;
        }
        let data = read_level(&level);
        let icon = path.join(ICON_FILE);
        let field = |key: &[&str]| {
            data.as_ref()
                .and_then(|tag| tag.at(&[&["Data"], key].concat()))
        };
        worlds.push(WorldInfo {
            name: field(&["LevelName"])
                .and_then(|tag| tag.as_str())
                .filter(|name| !name.trim().is_empty())
                .map_or_else(|| folder.clone(), str::to_owned),
            last_played_ms: field(&["LastPlayed"]).and_then(nbt::Tag::as_i64),
            game_version: field(&["Version", "Name"])
                .and_then(|tag| tag.as_str())
                .map(str::to_owned),
            hardcore: field(&["hardcore"]).and_then(nbt::Tag::as_i64) == Some(1),
            icon: icon.is_file().then_some(icon),
            damaged: data.is_none(),
            lock_touched_ms: fs::metadata(path.join(LOCK_FILE))
                .and_then(|metadata| metadata.modified())
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok()),
            folder,
        });
    }
    worlds.sort_by(|a, b| {
        b.last_played_ms
            .cmp(&a.last_played_ms)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(worlds)
}

/// `saves/`, refused when it is a link out of the instance.
fn saves_dir(game_dir: &Path) -> Result<PathBuf, WorldError> {
    let path = game_dir.join(SAVES);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(WorldError::Linked(SAVES.to_owned()))
        }
        _ => Ok(path),
    }
}

fn world_dir(game_dir: &Path, folder: &str) -> Result<PathBuf, WorldError> {
    if !is_safe_file_name(folder) {
        return Err(WorldError::UnsafeName(folder.to_owned()));
    }
    let path = saves_dir(game_dir)?.join(folder);
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(WorldError::Linked(folder.to_owned()))
        }
        Ok(metadata) if metadata.is_dir() => Ok(path),
        _ => Err(WorldError::NotFound(folder.to_owned())),
    }
}

/// Removes half-made copies left by an interrupted [`duplicate`]. Call it only
/// while nothing else is copying into this instance.
pub fn sweep_temporary(game_dir: &Path) -> usize {
    let Ok(saves) = saves_dir(game_dir) else {
        return 0;
    };
    let Ok(entries) = fs::read_dir(saves) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with('.') && name.ends_with(COPYING_SUFFIX)
        })
        .filter(|entry| fs::remove_dir_all(entry.path()).is_ok())
        .count()
}

/// Total size in bytes of a world's files. Symbolic links are not followed.
pub fn size(game_dir: &Path, folder: &str) -> Result<u64, WorldError> {
    fn walk(dir: &Path) -> io::Result<u64> {
        let mut total = 0;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() {
                total += walk(&entry.path())?;
            } else if kind.is_file() {
                total += entry.metadata()?.len();
            }
        }
        Ok(total)
    }
    Ok(walk(&world_dir(game_dir, folder)?)?)
}

pub fn delete(game_dir: &Path, folder: &str) -> Result<(), WorldError> {
    fs::remove_dir_all(world_dir(game_dir, folder)?)?;
    Ok(())
}

/// Copies a world to a new folder. Symbolic links are skipped rather than
/// followed. Returns the new folder name.
pub fn duplicate(game_dir: &Path, folder: &str, new_folder: &str) -> Result<String, WorldError> {
    let source = world_dir(game_dir, folder)?;
    if !is_safe_file_name(new_folder) {
        return Err(WorldError::UnsafeName(new_folder.to_owned()));
    }
    let saves = saves_dir(game_dir)?;
    let target = saves.join(new_folder);
    if target.symlink_metadata().is_ok() {
        return Err(WorldError::AlreadyExists(new_folder.to_owned()));
    }
    fn copy(from: &Path, to: &Path) -> io::Result<()> {
        fs::create_dir_all(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let destination = to.join(entry.file_name());
            if kind.is_dir() {
                copy(&entry.path(), &destination)?;
            } else if kind.is_file() {
                fs::copy(entry.path(), destination)?;
            }
        }
        Ok(())
    }
    // Build the copy under a hidden name and publish it with one rename, so a
    // crash leaves nothing that looks like a finished world.
    let staging = saves.join(format!(".{new_folder}{COPYING_SUFFIX}"));
    let _ = fs::remove_dir_all(&staging);
    if let Err(error) = copy(&source, &staging).and_then(|()| fs::rename(&staging, &target)) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error.into());
    }
    Ok(new_folder.to_owned())
}

/// Largest world archive imported, counted by the sizes it declares.
const IMPORT_LIMIT: u64 = 16 * 1024 * 1024 * 1024;
/// The game's lock file; it means nothing outside a running game.
const LOCK_FILE: &str = "session.lock";

fn archive_error(error: zip::result::ZipError) -> WorldError {
    WorldError::BadArchive(error.to_string())
}

/// Writes a world into a zip at `destination` as `<folder>/…`, through a
/// `.part` file renamed at the end, so a failure leaves no half archive.
/// Links are skipped, as is the game's lock file. Returns the archive's size.
pub fn export_zip(game_dir: &Path, folder: &str, destination: &Path) -> Result<u64, WorldError> {
    fn add(
        writer: &mut zip::ZipWriter<fs::File>,
        options: zip::write::SimpleFileOptions,
        dir: &Path,
        prefix: &str,
    ) -> Result<(), WorldError> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let kind = entry.file_type()?;
            let path = format!("{prefix}/{name}");
            if kind.is_dir() {
                writer
                    .add_directory(format!("{path}/"), options)
                    .map_err(archive_error)?;
                add(writer, options, &entry.path(), &path)?;
            } else if kind.is_file() && name != LOCK_FILE {
                writer.start_file(path, options).map_err(archive_error)?;
                io::copy(&mut fs::File::open(entry.path())?, writer)?;
            }
        }
        Ok(())
    }
    let source = world_dir(game_dir, folder)?;
    let part = destination.with_extension("zip.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer
            .add_directory(format!("{folder}/"), options)
            .map_err(archive_error)?;
        add(&mut writer, options, &source, folder)?;
        writer.finish().map_err(archive_error)?;
        fs::rename(&part, destination)?;
        Ok::<_, WorldError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

/// Adds the world in a zip to `saves/` and returns its folder name. The world
/// is the shallowest folder holding a `level.dat` (the archive's top level
/// counts, and then the archive's file name names it). A taken name gets
/// `2`, `3`, … appended; nothing existing is touched, and entries that would
/// leave the world's folder are skipped.
pub fn import_zip(game_dir: &Path, archive_path: &Path) -> Result<String, WorldError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(archive_path)?).map_err(archive_error)?;
    let mut declared = 0_u64;
    // The prefix (with its trailing '/', or empty) of the shallowest level.dat.
    let mut root: Option<String> = None;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(archive_error)?;
        declared = declared.saturating_add(entry.size());
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(name) = name.to_str().map(|name| name.replace('\\', "/")) else {
            continue;
        };
        if name.starts_with("__MACOSX/") || entry.is_dir() {
            continue;
        }
        let Some(prefix) = name.strip_suffix(LEVEL_FILE) else {
            continue;
        };
        if (!prefix.is_empty() && !prefix.ends_with('/')) || prefix.matches('/').count() > 1 {
            continue;
        }
        if root.as_ref().is_none_or(|best| prefix.len() < best.len()) {
            root = Some(prefix.to_owned());
        }
    }
    if declared > IMPORT_LIMIT {
        return Err(WorldError::BadArchive("it is too large".to_owned()));
    }
    let root = root.ok_or(WorldError::NotAWorld)?;
    let wanted = root.trim_end_matches('/').to_owned();
    let base = if wanted.is_empty() {
        archive_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("World")
            .trim()
            .to_owned()
    } else {
        wanted
    };
    if !is_safe_file_name(&base) || base.starts_with('.') {
        return Err(WorldError::UnsafeName(base));
    }
    let saves = saves_dir(game_dir)?;
    fs::create_dir_all(&saves)?;
    let taken = |name: &str| saves.join(name).symlink_metadata().is_ok();
    let folder = if taken(&base) {
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !taken(name))
            .expect("an unbounded range always yields a free name")
    } else {
        base
    };
    let staging = saves.join(format!(".{folder}{COPYING_SUFFIX}"));
    let _ = fs::remove_dir_all(&staging);
    let extracted = (|| {
        fs::create_dir_all(&staging)?;
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index).map_err(archive_error)?;
            let Some(name) = entry.enclosed_name() else {
                continue;
            };
            let Some(relative) = name
                .to_str()
                .map(|name| name.replace('\\', "/"))
                .and_then(|name| name.strip_prefix(&root).map(str::to_owned))
            else {
                continue;
            };
            if relative.is_empty()
                || entry.is_dir()
                || relative.starts_with("__MACOSX/")
                || relative.split('/').any(str::is_empty)
            {
                continue;
            }
            let target = staging.join(&relative);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            io::copy(&mut entry, &mut fs::File::create(target)?)?;
        }
        fs::rename(&staging, saves.join(&folder))?;
        Ok::<_, WorldError>(())
    })();
    if let Err(error) = extracted {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(folder)
}

/// A folder name for a copy of `folder`: `<folder> copy`, then `<folder> copy 2`, …
#[must_use]
pub fn copy_name(game_dir: &Path, folder: &str) -> String {
    let taken = |name: &str| game_dir.join(SAVES).join(name).symlink_metadata().is_ok();
    let first = format!("{folder} copy");
    if !taken(&first) {
        return first;
    }
    (2..)
        .map(|n| format!("{first} {n}"))
        .find(|name| !taken(name))
        .expect("an unbounded range always yields a free name")
}

#[cfg(test)]
mod tests;
