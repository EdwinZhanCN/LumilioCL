use super::kinds::ProjectKind;
use super::versions::Version;
use crate::transfer::{TransferError, TransferRequest};
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

#[derive(Debug, Eq, PartialEq)]
pub enum IntentError {
    /// Modpacks are installed as new instances, not copied into a folder.
    NotAFileKind,
    NoFile,
    /// The file name would escape the destination folder or is not portable.
    UnsafeFileName(String),
    Transfer(String),
}

impl Display for IntentError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFileKind => f.write_str("modpacks are installed as instances"),
            Self::NoFile => f.write_str("version has no downloadable file"),
            Self::UnsafeFileName(name) => write!(f, "unsafe file name {name:?}"),
            Self::Transfer(message) => write!(f, "{message}"),
        }
    }
}

impl Error for IntentError {}

impl From<TransferError> for IntentError {
    fn from(error: TransferError) -> Self {
        Self::Transfer(error.to_string())
    }
}

pub(crate) fn is_safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
        && name.trim() == name
}

/// The transfer that puts `version`'s file into `game_dir`, verified by the
/// published size and SHA-1. `sources` should already include any mirrors.
pub fn install_request(
    kind: ProjectKind,
    version: &Version,
    game_dir: &Path,
    sources: Vec<String>,
) -> Result<TransferRequest, IntentError> {
    let folder = kind.install_folder().ok_or(IntentError::NotAFileKind)?;
    let file = version.install_file().ok_or(IntentError::NoFile)?;
    if !is_safe_file_name(&file.filename) {
        return Err(IntentError::UnsafeFileName(file.filename.clone()));
    }
    let destination: PathBuf = game_dir.join(folder).join(&file.filename);
    let mut request = TransferRequest::new(
        format!("content:{}:{}", version.project_id, version.id),
        sources,
        destination,
    )?;
    if file.size > 0 {
        request = request.expect_size(file.size);
    }
    if let Some(sha1) = &file.sha1 {
        request = request.expect_sha1(sha1)?;
    }
    Ok(request)
}
