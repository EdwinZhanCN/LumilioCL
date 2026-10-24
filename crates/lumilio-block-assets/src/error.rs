use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::Path;

/// A source could not be read at all. Single unreadable entries do not fail
/// a bake; they are listed in [`PackMeta::skipped`](crate::PackMeta::skipped).
#[derive(Debug)]
pub enum BakeError {
    Io {
        path: String,
        error: std::io::Error,
    },
    /// The archive is not a zip, or is larger than any resource pack.
    Archive(String),
}

impl BakeError {
    pub(super) fn io(path: &Path, error: std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            error,
        }
    }

    pub(super) fn at(self, path: &Path) -> Self {
        match self {
            Self::Archive(reason) => Self::Archive(format!("{}: {reason}", path.display())),
            other => other,
        }
    }
}

impl Display for BakeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, error } => write!(f, "{path}: {error}"),
            Self::Archive(reason) => write!(f, "not a readable resource archive: {reason}"),
        }
    }
}

impl Error for BakeError {}

/// A baked file could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    Io(String),
    /// Not a zstd frame holding a baked container.
    NotABundle,
    /// Written by a newer launcher.
    UnsupportedFormat(u32),
    Corrupt(String),
    MissingPack(String),
}

impl Display for ContainerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(reason) => write!(f, "baked block assets: {reason}"),
            Self::NotABundle => f.write_str("the file is not a baked block-asset file"),
            Self::UnsupportedFormat(format) => write!(
                f,
                "the baked block-asset file uses format {format}, which this launcher cannot read"
            ),
            Self::Corrupt(reason) => write!(f, "the baked block-asset file is damaged: {reason}"),
            Self::MissingPack(key) => write!(f, "the baked block-asset file has no pack {key}"),
        }
    }
}

impl Error for ContainerError {}
