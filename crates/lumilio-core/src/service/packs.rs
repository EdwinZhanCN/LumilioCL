use super::error::ServiceError;
use crate::activity::CancellationToken;
use std::path::PathBuf;

/// What a pack file is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PackKind {
    Modrinth,
    Prism,
    /// One of this launcher's own full backups.
    Backup,
}

/// Looks at a zip's entry names: a Modrinth pack names `modrinth.index.json`,
/// a MultiMC / Prism instance `mmc-pack.json`. Anything else is treated as a
/// Modrinth pack so its error is the usual one.
pub(super) fn sniff_pack(path: &std::path::Path) -> PackKind {
    let Ok(file) = std::fs::File::open(path) else {
        return PackKind::Modrinth;
    };
    let Ok(archive) = zip::ZipArchive::new(file) else {
        return PackKind::Modrinth;
    };
    let names: Vec<&str> = archive.file_names().collect();
    if names.contains(&"modrinth.index.json") {
        PackKind::Modrinth
    } else if names.contains(&"lumilio-backup.json") {
        PackKind::Backup
    } else if names
        .iter()
        .any(|name| name.rsplit('/').next() == Some("mmc-pack.json"))
    {
        PackKind::Prism
    } else {
        PackKind::Modrinth
    }
}

pub(super) enum UnpackError {
    Cancelled,
    Other(String),
}

/// Unpacks a MultiMC / Prism instance zip into `scratch` and returns the
/// folder holding `instance.cfg` (the top, or one folder down). Entries that
/// would leave `scratch` are skipped; the declared size is capped.
pub(super) fn unpack_instance_zip(
    archive: &std::path::Path,
    scratch: &std::path::Path,
    cancel: &CancellationToken,
) -> Result<PathBuf, UnpackError> {
    const LIMIT: u64 = 16 * 1024 * 1024 * 1024;
    let other = |error: &dyn std::fmt::Display| UnpackError::Other(error.to_string());
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).map_err(|e| other(&e))?)
        .map_err(|e| other(&e))?;
    let mut declared = 0_u64;
    for index in 0..zip.len() {
        declared = declared.saturating_add(zip.by_index(index).map_err(|e| other(&e))?.size());
    }
    if declared > LIMIT {
        return Err(UnpackError::Other("the zip is too large".to_owned()));
    }
    std::fs::create_dir_all(scratch).map_err(|e| other(&e))?;
    for index in 0..zip.len() {
        if cancel.is_cancelled() {
            return Err(UnpackError::Cancelled);
        }
        let mut entry = zip.by_index(index).map_err(|e| other(&e))?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let target = scratch.join(name);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| other(&e))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| other(&e))?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| other(&e))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| other(&e))?;
    }
    if scratch.join("instance.cfg").is_file() {
        return Ok(scratch.to_owned());
    }
    std::fs::read_dir(scratch)
        .map_err(|e| other(&e))?
        .flatten()
        .map(|entry| entry.path())
        .find(|dir| dir.join("instance.cfg").is_file())
        .ok_or_else(|| UnpackError::Other("the zip holds no instance".to_owned()))
}

/// How a backup failure is told: a cancel stays a cancel.
pub(super) fn backup_failure(error: crate::backup::BackupError) -> ServiceError {
    match error {
        crate::backup::BackupError::Cancelled => ServiceError::Cancelled,
        other => ServiceError::Install(other.to_string()),
    }
}
