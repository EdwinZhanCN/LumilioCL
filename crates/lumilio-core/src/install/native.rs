use super::plan::NativeBundle;
use super::{
    MAX_ARCHIVE_ENTRIES, MAX_ARCHIVE_EXPANDED_SIZE, MAX_ARCHIVE_FILE_SIZE, PUBLICATION_SEQUENCE,
};
use crate::activity::CancellationToken;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::File as StandardFile;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;
use std::{fmt, io};
use zip::ZipArchive;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveLimits {
    pub(super) maximum_entries: usize,
    pub(super) maximum_file_size: u64,
    pub(super) maximum_expanded_size: u64,
}

impl ArchiveLimits {
    pub fn new(
        maximum_entries: usize,
        maximum_file_size: u64,
        maximum_expanded_size: u64,
    ) -> Result<Self, NativeError> {
        if maximum_entries == 0 || maximum_file_size == 0 || maximum_expanded_size == 0 {
            return Err(NativeError::InvalidLimits);
        }
        Ok(Self {
            maximum_entries,
            maximum_file_size,
            maximum_expanded_size,
        })
    }
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            maximum_entries: MAX_ARCHIVE_ENTRIES,
            maximum_file_size: MAX_ARCHIVE_FILE_SIZE,
            maximum_expanded_size: MAX_ARCHIVE_EXPANDED_SIZE,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePublication {
    pub(super) published_files: usize,
}

impl NativePublication {
    #[must_use]
    pub const fn published_files(self) -> usize {
        self.published_files
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativePublisher {
    pub(super) limits: ArchiveLimits,
}

impl NativePublisher {
    #[must_use]
    pub const fn with_limits(limits: ArchiveLimits) -> Self {
        Self { limits }
    }

    pub async fn publish(
        &self,
        bundles: &[NativeBundle],
        destination: impl AsRef<Path>,
        cancellation: CancellationToken,
    ) -> Result<NativePublication, NativeError> {
        if bundles.is_empty() {
            return Ok(NativePublication { published_files: 0 });
        }
        let bundles = bundles.to_vec();
        let destination = destination.as_ref().to_owned();
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            publish_native_blocking(&bundles, &destination, limits, &cancellation)
        })
        .await
        .map_err(|error| NativeError::Worker(error.to_string()))?
    }
}

#[derive(Debug)]
pub enum NativeError {
    InvalidLimits,
    Cancelled,
    UnsafePath { archive: PathBuf, entry: String },
    UnsupportedEntry { archive: PathBuf, entry: String },
    LimitExceeded { archive: PathBuf, detail: String },
    Archive { archive: PathBuf, message: String },
    FileSystem { path: PathBuf, message: String },
    Worker(String),
}

impl Display for NativeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("archive limits must be positive"),
            Self::Cancelled => formatter.write_str("native publication was cancelled"),
            Self::UnsafePath { archive, entry } => write!(
                formatter,
                "unsafe archive path {entry:?} in {}",
                archive.display()
            ),
            Self::UnsupportedEntry { archive, entry } => write!(
                formatter,
                "unsupported archive entry {entry:?} in {}",
                archive.display()
            ),
            Self::LimitExceeded { archive, detail } => {
                write!(
                    formatter,
                    "archive limit exceeded in {}: {detail}",
                    archive.display()
                )
            }
            Self::Archive { archive, message } => {
                write!(
                    formatter,
                    "cannot read archive {}: {message}",
                    archive.display()
                )
            }
            Self::FileSystem { path, message } => {
                write!(
                    formatter,
                    "filesystem operation failed at {}: {message}",
                    path.display()
                )
            }
            Self::Worker(message) => write!(formatter, "native worker failed: {message}"),
        }
    }
}

impl Error for NativeError {}

pub(super) fn publish_native_blocking(
    bundles: &[NativeBundle],
    destination: &Path,
    limits: ArchiveLimits,
    cancellation: &CancellationToken,
) -> Result<NativePublication, NativeError> {
    if cancellation.is_cancelled() {
        return Err(NativeError::Cancelled);
    }
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| native_fs_error(parent, error))?;
    let stage = unique_sibling(destination, "stage");
    let backup = unique_sibling(destination, "backup");
    std::fs::create_dir(&stage).map_err(|error| native_fs_error(&stage, error))?;

    let result = extract_all(bundles, &stage, limits, cancellation);
    let published_files = match result {
        Ok(files) => files,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(error);
        }
    };
    if cancellation.is_cancelled() {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(NativeError::Cancelled);
    }

    let had_destination = destination.exists();
    if had_destination {
        std::fs::rename(destination, &backup)
            .map_err(|error| native_fs_error(destination, error))?;
    }
    if let Err(error) = std::fs::rename(&stage, destination) {
        if had_destination {
            let _ = std::fs::rename(&backup, destination);
        }
        let _ = std::fs::remove_dir_all(&stage);
        return Err(native_fs_error(destination, error));
    }
    if had_destination {
        remove_any_path(&backup).map_err(|error| native_fs_error(&backup, error))?;
    }
    Ok(NativePublication { published_files })
}

pub(super) fn extract_all(
    bundles: &[NativeBundle],
    stage: &Path,
    limits: ArchiveLimits,
    cancellation: &CancellationToken,
) -> Result<usize, NativeError> {
    let mut expanded_size = 0_u64;
    let mut entry_count = 0_usize;
    let mut published_files = BTreeSet::new();
    for bundle in bundles {
        let file = StandardFile::open(&bundle.archive)
            .map_err(|error| native_fs_error(&bundle.archive, error))?;
        let mut archive = ZipArchive::new(file).map_err(|error| NativeError::Archive {
            archive: bundle.archive.clone(),
            message: error.to_string(),
        })?;
        entry_count = entry_count.saturating_add(archive.len());
        if entry_count > limits.maximum_entries {
            return Err(NativeError::LimitExceeded {
                archive: bundle.archive.clone(),
                detail: format!("more than {} entries", limits.maximum_entries),
            });
        }
        for index in 0..archive.len() {
            if cancellation.is_cancelled() {
                return Err(NativeError::Cancelled);
            }
            let mut entry = archive
                .by_index(index)
                .map_err(|error| NativeError::Archive {
                    archive: bundle.archive.clone(),
                    message: error.to_string(),
                })?;
            let raw_name = entry.name().to_owned();
            let relative =
                strict_archive_path(&raw_name).ok_or_else(|| NativeError::UnsafePath {
                    archive: bundle.archive.clone(),
                    entry: raw_name.clone(),
                })?;
            validate_entry_type(
                &bundle.archive,
                &raw_name,
                entry.is_dir(),
                entry.unix_mode(),
            )?;
            if bundle
                .exclusions
                .iter()
                .any(|prefix| raw_name.starts_with(prefix))
            {
                continue;
            }
            if entry.is_dir() {
                let directory = stage.join(relative);
                std::fs::create_dir_all(&directory)
                    .map_err(|error| native_fs_error(&directory, error))?;
                continue;
            }
            if entry.size() > limits.maximum_file_size {
                return Err(NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: format!("entry {raw_name:?} is {} bytes", entry.size()),
                });
            }
            expanded_size = expanded_size.checked_add(entry.size()).ok_or_else(|| {
                NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: "expanded size overflowed".to_owned(),
                }
            })?;
            if expanded_size > limits.maximum_expanded_size {
                return Err(NativeError::LimitExceeded {
                    archive: bundle.archive.clone(),
                    detail: format!(
                        "expanded content exceeds {} bytes",
                        limits.maximum_expanded_size
                    ),
                });
            }
            let output_path = stage.join(&relative);
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent).map_err(|error| native_fs_error(parent, error))?;
            }
            let mut output = StandardFile::create(&output_path)
                .map_err(|error| native_fs_error(&output_path, error))?;
            let copied = io::copy(&mut entry, &mut output)
                .map_err(|error| native_fs_error(&output_path, error))?;
            if copied != entry.size() {
                return Err(NativeError::Archive {
                    archive: bundle.archive.clone(),
                    message: format!(
                        "entry {raw_name:?} produced {copied} bytes, expected {}",
                        entry.size()
                    ),
                });
            }
            output
                .flush()
                .map_err(|error| native_fs_error(&output_path, error))?;
            published_files.insert(relative);
        }
    }
    Ok(published_files.len())
}

pub(super) fn validate_entry_type(
    archive: &Path,
    entry: &str,
    is_directory: bool,
    unix_mode: Option<u32>,
) -> Result<(), NativeError> {
    let Some(mode) = unix_mode else {
        return Ok(());
    };
    let file_type = mode & 0o170_000;
    let supported = file_type == 0
        || (!is_directory && file_type == 0o100_000)
        || (is_directory && file_type == 0o040_000);
    if supported {
        Ok(())
    } else {
        Err(NativeError::UnsupportedEntry {
            archive: archive.to_owned(),
            entry: entry.to_owned(),
        })
    }
}

pub(super) fn strict_archive_path(raw: &str) -> Option<PathBuf> {
    if raw.is_empty() || raw.contains('\0') || raw.contains('\\') || raw.contains(':') {
        return None;
    }
    let trimmed = raw.strip_suffix('/').unwrap_or(raw);
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Some(path.to_owned())
    } else {
        None
    }
}

pub(super) fn unique_sibling(destination: &Path, suffix: &str) -> PathBuf {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("publication");
    let sequence = PUBLICATION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        ".{name}.lumilio.{}.{}.{suffix}",
        std::process::id(),
        sequence
    ))
}

pub(super) fn remove_any_path(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn native_fs_error(path: &Path, error: io::Error) -> NativeError {
    NativeError::FileSystem {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
