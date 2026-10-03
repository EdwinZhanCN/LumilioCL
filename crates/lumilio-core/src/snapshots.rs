//! Snapshots: zip backups of an instance's worlds and settings.
//!
//! Behavior notes: `docs/behavior/snapshots.md`.

use crate::layout::Layout;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;

use crate::discover::is_safe_file_name;

const MANIFEST: &str = "snapshot.json";
/// Largest total uncompressed size a restore will extract.
pub const RESTORE_LIMIT: u64 = 64 * 1024 * 1024 * 1024;

/// Files and folders (relative to the game directory) a full snapshot covers.
const SETTINGS_ENTRIES: [&str; 3] = ["config", "options.txt", "servers.dat"];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "world", rename_all = "snake_case")]
pub enum SnapshotScope {
    /// Every world plus the game's settings.
    Full,
    /// One world, by folder name.
    World(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Manifest {
    label: String,
    created: u64,
    scope: SnapshotScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotInfo {
    /// File stem; the snapshot's identity.
    pub id: String,
    pub label: String,
    pub created: u64,
    pub scope: SnapshotScope,
    pub size: u64,
}

#[derive(Debug)]
pub enum SnapshotError {
    Io(io::Error),
    Zip(String),
    UnsafeName(String),
    NotFound(String),
    /// Nothing to back up.
    Empty,
    /// The archive is not a snapshot made by this launcher.
    NotASnapshot,
    TooLarge,
    /// An earlier restore of this snapshot has not been settled yet.
    RecoveryPending(PathBuf),
    /// A restore failed and its originals could not all be put back; the
    /// operation folder holds everything needed to finish at next start.
    RestoreIncomplete {
        operation: PathBuf,
        reason: String,
    },
}

impl Display for SnapshotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "snapshot storage failed: {error}"),
            Self::Zip(message) => write!(f, "snapshot archive error: {message}"),
            Self::UnsafeName(name) => write!(f, "unsafe name {name:?}"),
            Self::NotFound(name) => write!(f, "no snapshot or world {name:?}"),
            Self::Empty => f.write_str("there is nothing to back up"),
            Self::NotASnapshot => f.write_str("not a LumilioCL snapshot"),
            Self::TooLarge => f.write_str("snapshot is larger than the restore limit"),
            Self::RecoveryPending(path) => {
                write!(f, "an earlier restore is unsettled at {}", path.display())
            }
            Self::RestoreIncomplete { operation, reason } => write!(
                f,
                "restore failed and could not be undone ({reason}); recovery data is kept at {}",
                operation.display()
            ),
        }
    }
}

impl Error for SnapshotError {}

impl From<io::Error> for SnapshotError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<zip::result::ZipError> for SnapshotError {
    fn from(error: zip::result::ZipError) -> Self {
        Self::Zip(error.to_string())
    }
}

/// Where an instance keeps its snapshots.
#[must_use]
pub fn directory(root: &Path, instance_id: &str) -> PathBuf {
    Layout::new(root).snapshots(instance_id)
}

/// Regular files under `base/relative`, as `/`-separated names relative to
/// `base`. Symbolic links are skipped.
fn collect(base: &Path, relative: &str, out: &mut Vec<String>) -> io::Result<()> {
    let path = base.join(relative);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.is_file() {
        out.push(relative.to_owned());
    } else if metadata.is_dir() {
        let mut names: Vec<_> = fs::read_dir(&path)?
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect();
        names.sort();
        for name in names {
            collect(base, &format!("{relative}/{name}"), out)?;
        }
    }
    Ok(())
}

/// Creates a snapshot and returns its id. The archive is written to a temporary
/// file and renamed, so a failure never leaves a half-written snapshot.
pub fn create(
    root: &Path,
    instance_id: &str,
    game_dir: &Path,
    scope: SnapshotScope,
    label: &str,
    now: u64,
) -> Result<String, SnapshotError> {
    let mut files = Vec::new();
    match &scope {
        SnapshotScope::Full => {
            collect(game_dir, "saves", &mut files)?;
            for entry in SETTINGS_ENTRIES {
                collect(game_dir, entry, &mut files)?;
            }
        }
        SnapshotScope::World(folder) => {
            if !is_safe_file_name(folder) {
                return Err(SnapshotError::UnsafeName(folder.clone()));
            }
            if !game_dir.join("saves").join(folder).is_dir() {
                return Err(SnapshotError::NotFound(folder.clone()));
            }
            collect(game_dir, &format!("saves/{folder}"), &mut files)?;
        }
    }
    if files.is_empty() {
        return Err(SnapshotError::Empty);
    }

    let folder = directory(root, instance_id);
    fs::create_dir_all(&folder)?;
    let mut id = format!("snapshot-{now}");
    let mut counter = 2;
    while folder.join(format!("{id}.zip")).exists() {
        id = format!("snapshot-{now}-{counter}");
        counter += 1;
    }
    let target = folder.join(format!("{id}.zip"));
    let temporary = folder.join(format!("{id}.zip.tmp"));
    let result = (|| -> Result<(), SnapshotError> {
        let mut writer = zip::ZipWriter::new(fs::File::create(&temporary)?);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.start_file(MANIFEST, options)?;
        let manifest = Manifest {
            label: label.trim().to_owned(),
            created: now,
            scope,
        };
        writer.write_all(
            serde_json::to_string(&manifest)
                .map_err(|error| SnapshotError::Zip(error.to_string()))?
                .as_bytes(),
        )?;
        for name in &files {
            writer.start_file(name, options)?;
            io::copy(&mut fs::File::open(game_dir.join(name))?, &mut writer)?;
        }
        writer.finish()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    fs::rename(&temporary, &target)?;
    Ok(id)
}

fn read_manifest(path: &Path) -> Result<Manifest, SnapshotError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    let entry = archive
        .by_name(MANIFEST)
        .map_err(|_| SnapshotError::NotASnapshot)?;
    let mut text = String::new();
    entry.take(64 * 1024).read_to_string(&mut text)?;
    serde_json::from_str(&text).map_err(|_| SnapshotError::NotASnapshot)
}

/// Snapshots, newest first. Files that are not readable snapshots are skipped.
pub fn list(root: &Path, instance_id: &str) -> Result<Vec<SnapshotInfo>, SnapshotError> {
    let entries = match fs::read_dir(directory(root, instance_id)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut infos = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let Some(id) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".zip"))
            .map(str::to_owned)
        else {
            continue;
        };
        if let Ok(manifest) = read_manifest(&path) {
            infos.push(SnapshotInfo {
                size: fs::metadata(&path)?.len(),
                id,
                label: manifest.label,
                created: manifest.created,
                scope: manifest.scope,
            });
        }
    }
    infos.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
    Ok(infos)
}

pub fn delete(root: &Path, instance_id: &str, id: &str) -> Result<(), SnapshotError> {
    if !is_safe_file_name(id) {
        return Err(SnapshotError::UnsafeName(id.to_owned()));
    }
    let path = directory(root, instance_id).join(format!("{id}.zip"));
    if !path.is_file() {
        return Err(SnapshotError::NotFound(id.to_owned()));
    }
    fs::remove_file(path)?;
    Ok(())
}

/// The top-level item an archive entry belongs to, if restoring it is allowed:
/// `saves/<world>`, or one of the settings entries.
fn restore_unit(name: &Path) -> Option<PathBuf> {
    let mut parts = name.components().map(|c| c.as_os_str().to_str());
    let first = parts.next()??;
    if first == "saves" {
        let world = parts.next()??;
        Some(PathBuf::from("saves").join(world))
    } else if SETTINGS_ENTRIES.contains(&first) {
        Some(PathBuf::from(first))
    } else {
        None
    }
}

const RESTORE_JOURNAL: &str = "restore.json";
const NEW: &str = "new";
const OLD: &str = "old";

#[derive(Debug, Deserialize, Serialize)]
struct RestoreJournal {
    schema: u32,
    instance_id: String,
    snapshot: String,
    units: Vec<PathBuf>,
}

fn restore_dir(root: &Path, instance_id: &str, id: &str) -> PathBuf {
    Layout::new(root)
        .operations()
        .join(format!("restore-{instance_id}-{id}"))
}

fn remove_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn present(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Puts every unit back as it was before the restore began, whatever step the
/// restore reached. Each unit is judged from the disk alone: while the staged
/// copy is still in `new/` it was never placed (the original is in `old/` or
/// still at the target); once `new/` is gone it was placed and is removed in
/// favour of `old/` (or just removed when there was no original). A target
/// someone recreated in the meantime is never overwritten.
fn rollback(operation: &Path, game_dir: &Path, units: &[PathBuf]) -> Result<(), String> {
    for unit in units.iter().rev() {
        let (staged, original) = (
            operation.join(NEW).join(unit),
            operation.join(OLD).join(unit),
        );
        let target = game_dir.join(unit);
        let failed = |error: io::Error| format!("{}: {error}", unit.display());
        if present(&staged) {
            if present(&original) {
                if present(&target) {
                    return Err(format!(
                        "{} was recreated during the restore",
                        unit.display()
                    ));
                }
                fs::rename(&original, &target).map_err(failed)?;
            }
        } else {
            remove_path(&target).map_err(failed)?;
            if present(&original) {
                fs::rename(&original, &target).map_err(failed)?;
            }
        }
    }
    Ok(())
}

/// Restores a snapshot into `game_dir`.
///
/// The archive is first extracted into an operation folder under
/// `state/operations/`, so a failure while extracting leaves the game
/// untouched. Then each restored unit — a world folder, `config`,
/// `options.txt`, `servers.dat` — has its original moved aside into the same
/// folder and the staged copy renamed into place. If any step fails, or the
/// launcher dies part-way (see [`recover_interrupted`]), every unit is put
/// back as it was; the snapshot stays available to retry. Worlds and settings
/// not in the snapshot are left alone, and entries that would escape the game
/// directory or are not restorable units are ignored.
pub fn restore(
    root: &Path,
    instance_id: &str,
    id: &str,
    game_dir: &Path,
) -> Result<Vec<PathBuf>, SnapshotError> {
    if !is_safe_file_name(id) || !is_safe_file_name(instance_id) {
        return Err(SnapshotError::UnsafeName(id.to_owned()));
    }
    let path = directory(root, instance_id).join(format!("{id}.zip"));
    if !path.is_file() {
        return Err(SnapshotError::NotFound(id.to_owned()));
    }
    read_manifest(&path)?;
    let mut archive = zip::ZipArchive::new(fs::File::open(&path)?)?;

    let mut total = 0_u64;
    for index in 0..archive.len() {
        total = total.saturating_add(archive.by_index(index)?.size());
    }
    if total > RESTORE_LIMIT {
        return Err(SnapshotError::TooLarge);
    }

    let operation = restore_dir(root, instance_id, id);
    if present(&operation) {
        return Err(SnapshotError::RecoveryPending(operation));
    }
    let staging = operation.join(NEW);
    fs::create_dir_all(&staging)?;
    let staged = (|| -> Result<Vec<PathBuf>, SnapshotError> {
        let mut units = Vec::new();
        for index in 0..archive.len() {
            let mut entry = archive.by_index(index)?;
            let Some(name) = entry.enclosed_name() else {
                continue;
            };
            let Some(unit) = restore_unit(&name) else {
                continue;
            };
            if entry.is_dir() {
                continue;
            }
            let out = staging.join(&name);
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent)?;
            }
            io::copy(&mut entry, &mut fs::File::create(&out)?)?;
            if !units.contains(&unit) {
                units.push(unit);
            }
        }
        Ok(units)
    })();
    let units = match staged {
        Ok(units) => units,
        Err(error) => {
            let _ = fs::remove_dir_all(&operation);
            return Err(error);
        }
    };

    let journal = RestoreJournal {
        schema: 1,
        instance_id: instance_id.to_owned(),
        snapshot: id.to_owned(),
        units: units.clone(),
    };
    let journaled = serde_json::to_vec(&journal)
        .map_err(io::Error::other)
        .and_then(|bytes| fs::write(operation.join(RESTORE_JOURNAL), bytes));
    if let Err(error) = journaled {
        let _ = fs::remove_dir_all(&operation);
        return Err(error.into());
    }

    let committed = (|| -> io::Result<()> {
        for unit in &units {
            let target = game_dir.join(unit);
            if present(&target) {
                let original = operation.join(OLD).join(unit);
                if let Some(parent) = original.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&target, &original)?;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(staging.join(unit), &target)?;
        }
        Ok(())
    })();
    if let Err(error) = committed {
        return match rollback(&operation, game_dir, &units) {
            Ok(()) => {
                let _ = fs::remove_dir_all(&operation);
                Err(error.into())
            }
            // Originals could not all be put back: keep everything for recovery.
            Err(reason) => Err(SnapshotError::RestoreIncomplete {
                operation,
                reason: format!("{error}; rollback: {reason}"),
            }),
        };
    }
    // Journal first: a leftover folder without one is only garbage, never a
    // reason to undo a restore that finished.
    let _ = fs::remove_file(operation.join(RESTORE_JOURNAL));
    let _ = fs::remove_dir_all(&operation);
    Ok(units)
}

/// What start-up recovery did with one interrupted restore.
#[derive(Debug)]
pub(crate) struct Interrupted {
    pub instance_id: String,
    pub snapshot: String,
    pub operation: PathBuf,
    /// The units put back, or why that could not be finished.
    pub outcome: Result<Vec<PathBuf>, String>,
}

/// Rolls back restores the launcher died in the middle of. Folders without a
/// journal are only staging garbage (the game was not touched) and are removed
/// silently. Safe to call again.
pub(crate) fn recover_interrupted(layout: &Layout) -> Vec<Interrupted> {
    let Ok(entries) = fs::read_dir(layout.operations()) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("restore-"))
        })
        .collect();
    dirs.sort();
    let mut found = Vec::new();
    for operation in dirs {
        let journal = fs::read(operation.join(RESTORE_JOURNAL))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<RestoreJournal>(&bytes).ok());
        let Some(journal) = journal else {
            if present(&operation.join(RESTORE_JOURNAL)) {
                found.push(Interrupted {
                    instance_id: String::new(),
                    snapshot: String::new(),
                    outcome: Err("restore journal could not be read".to_owned()),
                    operation,
                });
            } else {
                let _ = fs::remove_dir_all(&operation);
            }
            continue;
        };
        let game_dir = layout.game(&journal.instance_id);
        let outcome = if journal.schema > 1 || !is_safe_file_name(&journal.instance_id) {
            Err("restore journal is from a newer launcher".to_owned())
        } else if journal.units.iter().any(|unit| {
            restore_unit(unit).as_deref() != Some(unit.as_path())
                || !unit
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
        }) {
            Err("restore journal names a path outside the restorable units".to_owned())
        } else {
            rollback(&operation, &game_dir, &journal.units)
                .and_then(|()| {
                    let _ = fs::remove_file(operation.join(RESTORE_JOURNAL));
                    fs::remove_dir_all(&operation).map_err(|error| error.to_string())
                })
                .map(|()| journal.units.clone())
        };
        found.push(Interrupted {
            instance_id: journal.instance_id,
            snapshot: journal.snapshot,
            operation,
            outcome,
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        game: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("launcher");
        let game = root.join("instances/a/game");
        for (path, body) in [
            ("saves/One/level.dat", "one"),
            ("saves/One/region/r.0.0.mca", "region-one"),
            ("saves/Two/level.dat", "two"),
            ("config/mod.toml", "cfg"),
            ("options.txt", "fov:70"),
        ] {
            let full = game.join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, body).unwrap();
        }
        Fixture {
            root,
            game,
            _dir: dir,
        }
    }

    #[test]
    fn a_full_snapshot_lists_with_its_details() {
        let f = fixture();
        let id = create(
            &f.root,
            "a",
            &f.game,
            SnapshotScope::Full,
            " before update ",
            100,
        )
        .unwrap();
        assert_eq!(id, "snapshot-100");
        let second = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 100).unwrap();
        assert_eq!(second, "snapshot-100-2");
        create(
            &f.root,
            "a",
            &f.game,
            SnapshotScope::World("One".into()),
            "one",
            200,
        )
        .unwrap();
        let infos = list(&f.root, "a").unwrap();
        assert_eq!(infos.len(), 3);
        assert_eq!(infos[0].scope, SnapshotScope::World("One".into()));
        let full = infos.iter().find(|i| i.id == "snapshot-100").unwrap();
        assert_eq!(full.label, "before update");
        assert!(full.size > 0);
    }

    #[test]
    fn nothing_to_back_up_and_bad_worlds_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(&game).unwrap();
        assert!(matches!(
            create(dir.path(), "a", &game, SnapshotScope::Full, "", 1),
            Err(SnapshotError::Empty)
        ));
        let f = fixture();
        assert!(matches!(
            create(
                &f.root,
                "a",
                &f.game,
                SnapshotScope::World("../x".into()),
                "",
                1
            ),
            Err(SnapshotError::UnsafeName(_))
        ));
        assert!(matches!(
            create(
                &f.root,
                "a",
                &f.game,
                SnapshotScope::World("Ghost".into()),
                "",
                1
            ),
            Err(SnapshotError::NotFound(_))
        ));
        assert!(
            list(&f.root, "a").unwrap().is_empty(),
            "failures leave no snapshot behind"
        );
        assert!(!directory(&f.root, "a").join("snapshot-1.zip.tmp").exists());
    }

    #[test]
    fn restoring_a_world_replaces_only_that_world() {
        let f = fixture();
        let id = create(
            &f.root,
            "a",
            &f.game,
            SnapshotScope::World("One".into()),
            "",
            1,
        )
        .unwrap();
        fs::write(f.game.join("saves/One/level.dat"), "changed").unwrap();
        fs::write(f.game.join("saves/One/extra.dat"), "added later").unwrap();
        fs::write(f.game.join("saves/Two/level.dat"), "two-changed").unwrap();
        let units = restore(&f.root, "a", &id, &f.game).unwrap();
        assert_eq!(units, [PathBuf::from("saves/One")]);
        assert_eq!(
            fs::read_to_string(f.game.join("saves/One/level.dat")).unwrap(),
            "one"
        );
        assert!(
            !f.game.join("saves/One/extra.dat").exists(),
            "the world is replaced, not merged"
        );
        assert_eq!(
            fs::read_to_string(f.game.join("saves/Two/level.dat")).unwrap(),
            "two-changed"
        );
        assert!(
            !Layout::new(&f.root).operations().exists()
                || fs::read_dir(Layout::new(&f.root).operations())
                    .unwrap()
                    .next()
                    .is_none()
        );
    }

    #[test]
    fn restoring_a_full_snapshot_brings_back_deleted_worlds_and_settings() {
        let f = fixture();
        let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
        fs::remove_dir_all(f.game.join("saves/Two")).unwrap();
        fs::remove_dir_all(f.game.join("config")).unwrap();
        fs::write(f.game.join("options.txt"), "fov:110").unwrap();
        fs::create_dir_all(f.game.join("saves/Three")).unwrap();
        fs::write(f.game.join("saves/Three/level.dat"), "three").unwrap();
        restore(&f.root, "a", &id, &f.game).unwrap();
        assert_eq!(
            fs::read_to_string(f.game.join("saves/Two/level.dat")).unwrap(),
            "two"
        );
        assert_eq!(
            fs::read_to_string(f.game.join("config/mod.toml")).unwrap(),
            "cfg"
        );
        assert_eq!(
            fs::read_to_string(f.game.join("options.txt")).unwrap(),
            "fov:70"
        );
        assert!(
            f.game.join("saves/Three/level.dat").exists(),
            "worlds not in the snapshot survive"
        );
    }

    #[test]
    fn a_hostile_archive_cannot_write_outside_the_restorable_units() {
        let f = fixture();
        let folder = directory(&f.root, "a");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("evil.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = SimpleFileOptions::default();
        writer.start_file(MANIFEST, options).unwrap();
        writer
            .write_all(br#"{"label":"","created":1,"scope":{"kind":"full"}}"#)
            .unwrap();
        for name in ["../escaped.txt", "mods/evil.jar", "saves/Good/level.dat"] {
            writer.start_file(name, options).unwrap();
            writer.write_all(b"x").unwrap();
        }
        writer.finish().unwrap();
        let units = restore(&f.root, "a", "evil", &f.game).unwrap();
        assert_eq!(units, [PathBuf::from("saves/Good")]);
        assert!(!f.game.parent().unwrap().join("escaped.txt").exists());
        assert!(!f.game.join("mods/evil.jar").exists());
    }

    #[test]
    fn foreign_archives_are_not_snapshots_and_ids_are_guarded() {
        let f = fixture();
        let folder = directory(&f.root, "a");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("junk.zip"), "not a zip").unwrap();
        assert!(list(&f.root, "a").unwrap().is_empty());
        assert!(matches!(
            restore(&f.root, "a", "junk", &f.game),
            Err(SnapshotError::Zip(_))
        ));
        assert!(matches!(
            restore(&f.root, "a", "../x", &f.game),
            Err(SnapshotError::UnsafeName(_))
        ));
        assert!(matches!(
            delete(&f.root, "a", "ghost"),
            Err(SnapshotError::NotFound(_))
        ));
    }

    /// All files of the fixture game, to compare before/after.
    fn tree(game: &Path) -> Vec<(String, String)> {
        let mut out = Vec::new();
        fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
            let mut entries: Vec<_> = fs::read_dir(dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    walk(base, &path, out);
                } else {
                    out.push((
                        path.strip_prefix(base).unwrap().display().to_string(),
                        fs::read_to_string(&path).unwrap(),
                    ));
                }
            }
        }
        walk(game, game, &mut out);
        out
    }

    fn operations(f: &Fixture) -> Vec<PathBuf> {
        fs::read_dir(Layout::new(&f.root).operations())
            .map(|entries| entries.map(|e| e.unwrap().path()).collect())
            .unwrap_or_default()
    }

    #[test]
    fn a_commit_that_fails_at_any_unit_puts_every_original_back() {
        use std::os::unix::fs::PermissionsExt;
        // Writable saves but read-only game folder: saves/One and saves/Two are
        // replaced, then `config` (an entry of the game folder) cannot be moved.
        // Read-only saves: the very first unit fails.
        for locked in ["game", "saves"] {
            let f = fixture();
            let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
            fs::write(f.game.join("saves/One/level.dat"), "changed-one").unwrap();
            fs::write(f.game.join("saves/Two/level.dat"), "changed-two").unwrap();
            fs::write(f.game.join("options.txt"), "fov:110").unwrap();
            let before = tree(&f.game);
            let lock = if locked == "game" {
                f.game.clone()
            } else {
                f.game.join("saves")
            };
            fs::set_permissions(&lock, fs::Permissions::from_mode(0o555)).unwrap();
            let result = restore(&f.root, "a", &id, &f.game);
            fs::set_permissions(&lock, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(
                matches!(result, Err(SnapshotError::Io(_))),
                "{locked}: {result:?}"
            );
            assert_eq!(
                tree(&f.game),
                before,
                "{locked}: the game is exactly as it was"
            );
            assert!(operations(&f).is_empty(), "{locked}: nothing left behind");
            // The snapshot is intact and a retry succeeds.
            restore(&f.root, "a", &id, &f.game).unwrap();
            assert_eq!(
                fs::read_to_string(f.game.join("options.txt")).unwrap(),
                "fov:70"
            );
        }
    }

    #[test]
    fn an_archive_that_cannot_be_staged_leaves_the_game_alone() {
        let f = fixture();
        let folder = directory(&f.root, "a");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("cut.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        let options = SimpleFileOptions::default();
        writer.start_file(MANIFEST, options).unwrap();
        writer
            .write_all(br#"{"label":"","created":1,"scope":{"kind":"full"}}"#)
            .unwrap();
        writer.start_file("saves/One/level.dat", options).unwrap();
        writer.write_all(b"new-one").unwrap();
        writer.finish().unwrap();
        // Truncate the archive so extraction fails after the manifest is readable.
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..bytes.len() - 40]).unwrap();
        let before = tree(&f.game);
        assert!(restore(&f.root, "a", "cut", &f.game).is_err());
        assert_eq!(tree(&f.game), before);
        assert!(operations(&f).is_empty());
    }

    #[test]
    fn a_restore_the_launcher_died_in_is_undone_at_the_next_start() {
        let f = fixture();
        let layout = Layout::new(&f.root);
        let id = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
        fs::write(f.game.join("saves/One/level.dat"), "changed-one").unwrap();
        fs::write(f.game.join("saves/Two/level.dat"), "changed-two").unwrap();
        let before = tree(&f.game);
        // Rehearse a crash after the first unit was replaced and the second's
        // original moved aside but not yet replaced.
        let operation = restore_dir(&f.root, "a", &id);
        let staged = operation.join(NEW);
        fs::create_dir_all(staged.join("saves/One")).unwrap();
        fs::create_dir_all(staged.join("saves/Two")).unwrap();
        fs::write(staged.join("saves/One/level.dat"), "one").unwrap();
        fs::write(staged.join("saves/Two/level.dat"), "two").unwrap();
        fs::create_dir_all(operation.join(OLD).join("saves")).unwrap();
        fs::rename(
            f.game.join("saves/One"),
            operation.join(OLD).join("saves/One"),
        )
        .unwrap();
        fs::rename(staged.join("saves/One"), f.game.join("saves/One")).unwrap();
        fs::rename(
            f.game.join("saves/Two"),
            operation.join(OLD).join("saves/Two"),
        )
        .unwrap();
        fs::write(
            operation.join(RESTORE_JOURNAL),
            serde_json::to_vec(&RestoreJournal {
                schema: 1,
                instance_id: "a".into(),
                snapshot: id.clone(),
                units: vec![PathBuf::from("saves/One"), PathBuf::from("saves/Two")],
            })
            .unwrap(),
        )
        .unwrap();
        // The recovery looks for the game under the layout's profile folder.
        let profile_game = layout.game("a");
        fs::create_dir_all(profile_game.parent().unwrap()).unwrap();
        fs::rename(&f.game, &profile_game).unwrap();
        let found = recover_interrupted(&layout);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].snapshot, id);
        assert!(found[0].outcome.is_ok(), "{:?}", found[0].outcome);
        assert_eq!(tree(&profile_game), before);
        assert!(operations(&f).is_empty());
        // A second pass finds nothing to do.
        assert!(recover_interrupted(&layout).is_empty());
    }

    #[test]
    fn a_folder_without_a_journal_is_garbage_and_a_recreated_target_is_never_overwritten() {
        let f = fixture();
        let layout = Layout::new(&f.root);
        let stale = layout.operations().join("restore-a-old");
        fs::create_dir_all(stale.join(NEW).join("saves/X")).unwrap();
        assert!(recover_interrupted(&layout).is_empty());
        assert!(!stale.exists(), "no journal: the game was never touched");

        let operation = layout.operations().join("restore-a-s");
        fs::create_dir_all(operation.join(NEW).join("saves/One")).unwrap();
        fs::create_dir_all(operation.join(OLD).join("saves/One")).unwrap();
        fs::write(operation.join(OLD).join("saves/One/level.dat"), "original").unwrap();
        fs::write(
            operation.join(RESTORE_JOURNAL),
            serde_json::to_vec(&RestoreJournal {
                schema: 1,
                instance_id: "a".into(),
                snapshot: "s".into(),
                units: vec![PathBuf::from("saves/One")],
            })
            .unwrap(),
        )
        .unwrap();
        let game = layout.game("a");
        fs::create_dir_all(game.join("saves/One")).unwrap();
        fs::write(game.join("saves/One/level.dat"), "made by someone else").unwrap();
        let found = recover_interrupted(&layout);
        assert!(found[0].outcome.is_err());
        assert_eq!(
            fs::read_to_string(game.join("saves/One/level.dat")).unwrap(),
            "made by someone else"
        );
        assert!(
            operation.join(OLD).join("saves/One/level.dat").is_file(),
            "evidence kept"
        );
    }

    #[test]
    fn delete_removes_only_the_named_snapshot() {
        let f = fixture();
        let a = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 1).unwrap();
        let b = create(&f.root, "a", &f.game, SnapshotScope::Full, "", 2).unwrap();
        delete(&f.root, "a", &a).unwrap();
        let ids: Vec<_> = list(&f.root, "a")
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(ids, [b]);
    }
}
