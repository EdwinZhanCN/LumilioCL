//! Copying an instance's game folder.
//!
//! The copy is a set of independent files: nothing is hard-linked or symlinked
//! back to the source, so changing one instance can never change the other.

use std::fs;
use std::io;
use std::path::Path;

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
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    fn source() -> (tempfile::TempDir, std::path::PathBuf) {
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
    fn copies_independent_files_and_leaves_out_volatile_output_and_links() {
        let (dir, game) = source();
        std::os::unix::fs::symlink(dir.path(), game.join("mods/link")).unwrap();
        let target = dir.path().join("copy");
        let copied = copy_game(&game, &target, true, &CancellationToken::new()).unwrap();
        assert_eq!(copied, 4);
        assert_eq!(
            fs::read(target.join("saves/W/level.dat")).unwrap(),
            b"world"
        );
        assert!(!target.join("logs").exists() && !target.join("crash-reports").exists());
        assert!(!target.join("mods/link").exists());
        // Not hard-linked: editing the copy cannot change the source.
        assert_ne!(
            fs::metadata(game.join("options.txt")).unwrap().ino(),
            fs::metadata(target.join("options.txt")).unwrap().ino()
        );
        fs::write(target.join("options.txt"), "changed").unwrap();
        assert_eq!(fs::read(game.join("options.txt")).unwrap(), b"fov");
    }

    #[test]
    fn worlds_are_optional_and_a_missing_source_is_an_empty_copy() {
        let (dir, game) = source();
        let target = dir.path().join("copy");
        copy_game(&game, &target, false, &CancellationToken::new()).unwrap();
        assert!(!target.join("saves").exists() && target.join("mods/a.jar").is_file());
        let empty = dir.path().join("empty");
        assert_eq!(
            copy_game(
                &dir.path().join("nothing"),
                &empty,
                true,
                &CancellationToken::new()
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn cancelling_stops_the_copy_and_a_linked_game_folder_is_refused() {
        let (dir, game) = source();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            copy_game(&game, &dir.path().join("c1"), true, &cancel),
            Err(CopyError::Cancelled)
        ));
        let link = dir.path().join("linked-game");
        std::os::unix::fs::symlink(&game, &link).unwrap();
        assert!(matches!(
            copy_game(
                &link,
                &dir.path().join("c2"),
                true,
                &CancellationToken::new()
            ),
            Err(CopyError::Io(_))
        ));
    }
}
