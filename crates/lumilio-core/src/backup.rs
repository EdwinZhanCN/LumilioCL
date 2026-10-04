//! Full backups of one game (`ADR 0015`).
//!
//! A backup is a zip: `lumilio-backup.json` describing the game, and the
//! whole game folder under `game/` (logs and crash reports left out, links
//! never followed). Restoring builds a new game; it never touches an existing
//! one.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::activity::CancellationToken;
use crate::instance::{InstanceSettings, Loader};
use crate::modpack::is_safe_relative;

const META: &str = "lumilio-backup.json";
const GAME: &str = "game/";
const FORMAT: u32 = 1;
/// Top-level folders that are volatile output, never user content.
const SKIPPED: [&str; 2] = ["logs", "crash-reports"];
const META_LIMIT: u64 = 1024 * 1024;
/// Largest total size restored, counted by the sizes the archive declares.
const RESTORE_LIMIT: u64 = 64 * 1024 * 1024 * 1024;

/// What a backup says about the game it holds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackupMeta {
    pub format: u32,
    pub name: String,
    pub game_version: String,
    pub loader: Loader,
    pub loader_version: Option<String>,
    /// Seconds since the Unix epoch.
    pub created: u64,
    /// The game's own settings (a Java path is machine-specific and is left
    /// out when restoring).
    #[serde(default)]
    pub settings: InstanceSettings,
}

#[derive(Debug)]
pub enum BackupError {
    Io(io::Error),
    Zip(String),
    /// Not a backup made by this launcher, or from a newer format.
    NotABackup,
    TooLarge,
    Cancelled,
}

impl Display for BackupError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "backup failed: {error}"),
            Self::Zip(message) => write!(f, "backup archive error: {message}"),
            Self::NotABackup => f.write_str("this is not a backup this launcher can read"),
            Self::TooLarge => f.write_str("the backup is too large"),
            Self::Cancelled => f.write_str("the backup was cancelled"),
        }
    }
}

impl Error for BackupError {}

impl From<io::Error> for BackupError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for BackupError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}

/// Writes the backup through a `.part` file renamed at the end; a cancelled
/// or failed backup leaves nothing. Returns the archive's size.
pub fn write(
    game_dir: &Path,
    meta: &BackupMeta,
    destination: &Path,
    cancel: &CancellationToken,
) -> Result<u64, BackupError> {
    fn add(
        writer: &mut zip::ZipWriter<fs::File>,
        options: zip::write::SimpleFileOptions,
        dir: &Path,
        prefix: &str,
        top: bool,
        cancel: &CancellationToken,
    ) -> Result<(), BackupError> {
        let mut entries: Vec<_> = fs::read_dir(dir)?.collect::<Result<_, _>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            if top && SKIPPED.contains(&name.as_str()) {
                continue;
            }
            if cancel.is_cancelled() {
                return Err(BackupError::Cancelled);
            }
            let kind = entry.file_type()?;
            let path = format!("{prefix}{name}");
            if kind.is_dir() {
                writer.add_directory(format!("{path}/"), options)?;
                add(
                    writer,
                    options,
                    &entry.path(),
                    &format!("{path}/"),
                    false,
                    cancel,
                )?;
            } else if kind.is_file() {
                writer.start_file(path, options)?;
                io::copy(&mut fs::File::open(entry.path())?, writer)?;
            }
        }
        Ok(())
    }
    let part = destination.with_extension("lumilio-backup.part");
    let written = (|| {
        let mut writer = zip::ZipWriter::new(fs::File::create(&part)?);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .large_file(true);
        writer.start_file(META, options)?;
        writer.write_all(
            serde_json::to_string_pretty(meta)
                .map_err(|error| BackupError::Zip(error.to_string()))?
                .as_bytes(),
        )?;
        writer.add_directory(GAME, options)?;
        if game_dir.is_dir() {
            add(&mut writer, options, game_dir, GAME, true, cancel)?;
        }
        writer.finish()?;
        fs::rename(&part, destination)?;
        Ok::<_, BackupError>(fs::metadata(destination)?.len())
    })();
    if written.is_err() {
        let _ = fs::remove_file(&part);
    }
    written
}

/// What a backup says about its game.
pub fn read_meta(archive: &Path) -> Result<BackupMeta, BackupError> {
    let mut archive =
        zip::ZipArchive::new(fs::File::open(archive)?).map_err(|_| BackupError::NotABackup)?;
    let entry = archive.by_name(META).map_err(|_| BackupError::NotABackup)?;
    let mut text = String::new();
    io::Read::read_to_string(&mut io::Read::take(entry, META_LIMIT), &mut text)
        .map_err(|_| BackupError::NotABackup)?;
    let meta: BackupMeta = serde_json::from_str(&text).map_err(|_| BackupError::NotABackup)?;
    if meta.format != FORMAT || meta.name.trim().is_empty() {
        return Err(BackupError::NotABackup);
    }
    Ok(meta)
}

/// Unpacks the backup's game folder into the (empty) folder `into`. Entries
/// that would leave it are skipped. Returns how many files were written.
pub fn restore(
    archive: &Path,
    into: &Path,
    cancel: &CancellationToken,
) -> Result<usize, BackupError> {
    let mut archive =
        zip::ZipArchive::new(fs::File::open(archive)?).map_err(|_| BackupError::NotABackup)?;
    let mut declared = 0_u64;
    for index in 0..archive.len() {
        declared = declared.saturating_add(archive.by_index(index)?.size());
    }
    if declared > RESTORE_LIMIT {
        return Err(BackupError::TooLarge);
    }
    fs::create_dir_all(into)?;
    let mut written = 0;
    for index in 0..archive.len() {
        if cancel.is_cancelled() {
            return Err(BackupError::Cancelled);
        }
        let mut entry = archive.by_index(index)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Some(relative) = name
            .to_str()
            .map(|name| name.replace('\\', "/"))
            .and_then(|name| name.strip_prefix(GAME).map(str::to_owned))
            .filter(|relative| !relative.is_empty() && is_safe_relative(relative))
        else {
            continue;
        };
        let target: PathBuf = into.join(&relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        io::copy(&mut entry, &mut fs::File::create(target)?)?;
        written += 1;
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> BackupMeta {
        BackupMeta {
            format: FORMAT,
            name: "生存".into(),
            game_version: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: Some("0.16.0".into()),
            created: 5,
            settings: InstanceSettings {
                max_memory_mb: Some(4096),
                ..InstanceSettings::default()
            },
        }
    }

    fn game() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        for (path, body) in [
            ("mods/a.jar", "mod"),
            ("config/x.toml", "cfg"),
            ("options.txt", "fov"),
            ("saves/W/level.dat", "world"),
            ("logs/latest.log", "log"),
            ("crash-reports/c.txt", "crash"),
        ] {
            let full = game.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, body).unwrap();
        }
        (dir, game)
    }

    #[test]
    fn a_backup_holds_everything_but_logs_and_restores_it_whole() {
        let (dir, game) = game();
        let archive = dir.path().join("b.zip");
        let size = write(&game, &meta(), &archive, &CancellationToken::new()).unwrap();
        assert!(size > 0 && !dir.path().join("b.lumilio-backup.part").exists());
        assert_eq!(read_meta(&archive).unwrap(), meta());

        let restored = dir.path().join("restored");
        let count = restore(&archive, &restored, &CancellationToken::new()).unwrap();
        assert_eq!(count, 4);
        assert_eq!(fs::read(restored.join("mods/a.jar")).unwrap(), b"mod");
        assert_eq!(
            fs::read(restored.join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(!restored.join("logs").exists() && !restored.join("crash-reports").exists());
    }

    #[test]
    fn a_cancelled_backup_leaves_nothing_and_a_foreign_zip_is_not_a_backup() {
        let (dir, game) = game();
        let archive = dir.path().join("never.zip");
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            write(&game, &meta(), &archive, &cancel),
            Err(BackupError::Cancelled)
        ));
        assert!(!archive.exists() && !dir.path().join("never.lumilio-backup.part").exists());

        let other = dir.path().join("other.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&other).unwrap());
        writer
            .start_file("readme.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"hi").unwrap();
        writer.finish().unwrap();
        assert!(matches!(read_meta(&other), Err(BackupError::NotABackup)));
        let text = dir.path().join("text.zip");
        fs::write(&text, b"not a zip").unwrap();
        assert!(matches!(read_meta(&text), Err(BackupError::NotABackup)));
    }

    #[test]
    fn restoring_skips_entries_that_would_escape_and_a_newer_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file(META, options).unwrap();
        writer
            .write_all(serde_json::to_string(&meta()).unwrap().as_bytes())
            .unwrap();
        for name in [
            "game/ok.txt",
            "game/../../escape.txt",
            "/abs.txt",
            "other/x.txt",
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"x").unwrap();
        }
        writer.finish().unwrap();
        let into = dir.path().join("into");
        assert_eq!(
            restore(&archive, &into, &CancellationToken::new()).unwrap(),
            1
        );
        assert!(into.join("ok.txt").is_file());
        assert!(!dir.path().join("escape.txt").exists() && !into.join("other").exists());

        let future = dir.path().join("future.zip");
        let mut newer = meta();
        newer.format = FORMAT + 1;
        let mut writer = zip::ZipWriter::new(fs::File::create(&future).unwrap());
        writer.start_file(META, options).unwrap();
        writer
            .write_all(serde_json::to_string(&newer).unwrap().as_bytes())
            .unwrap();
        writer.finish().unwrap();
        assert!(matches!(read_meta(&future), Err(BackupError::NotABackup)));
    }
}
