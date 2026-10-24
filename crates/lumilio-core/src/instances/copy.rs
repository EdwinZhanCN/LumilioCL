//! Copying an instance's game folder.
//!
//! The copy is a set of independent files: nothing is hard-linked or symlinked
//! back to the source, so changing one instance can never change the other.

use std::path::Path;
use std::{fs, io};

use crate::activity::CancellationToken;

/// Top-level folders that are volatile output, never user content.
const SKIPPED: [&str; 2] = ["logs", "crash-reports"];
const WORLDS: &str = "saves";

#[derive(Debug)]
pub(crate) enum CopyError {
    Io(io::Error),
    Cancelled,
}

impl From<io::Error> for CopyError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Copies `from` into the (empty) folder `to`, returning how many files were
/// copied. `logs/` and `crash-reports/` are left out, `saves/` only when
/// `include_worlds`. Symbolic links are skipped, never followed. A missing
/// source is an empty copy. The token is checked before every file.
pub(crate) fn copy_game(
    from: &Path,
    to: &Path,
    include_worlds: bool,
    cancel: &CancellationToken,
) -> Result<u64, CopyError> {
    match fs::symlink_metadata(from) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the game folder is a link; it could lead outside the instance",
            )
            .into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    }
    let mut copied = 0;
    walk(from, to, true, include_worlds, cancel, &mut copied)?;
    Ok(copied)
}

fn walk(
    from: &Path,
    to: &Path,
    top: bool,
    include_worlds: bool,
    cancel: &CancellationToken,
    copied: &mut u64,
) -> Result<(), CopyError> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let kind = entry.file_type()?;
        if top {
            let name = name.to_string_lossy();
            if SKIPPED.contains(&name.as_ref()) || (name == WORLDS && !include_worlds) {
                continue;
            }
        }
        if cancel.is_cancelled() {
            return Err(CopyError::Cancelled);
        }
        let destination = to.join(&name);
        if kind.is_dir() {
            walk(
                &entry.path(),
                &destination,
                false,
                include_worlds,
                cancel,
                copied,
            )?;
        } else if kind.is_file() {
            fs::copy(entry.path(), destination)?;
            *copied += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
