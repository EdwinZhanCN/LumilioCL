use crate::Relaunch;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::process::Command;

/// Atomically replaces the executable. The running process keeps the old
/// file open; the new version starts on the next launch.
pub(super) fn install(executable: &Path, update: &Path) -> Result<Relaunch, String> {
    let parent = executable
        .parent()
        .ok_or_else(|| "running executable has no parent folder".to_owned())?;
    let stem = update
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".tar.gz"))
        .ok_or_else(|| "Linux update is not a tar.gz package".to_owned())?;
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let archive = Command::new("tar")
        .args(["-xzf"])
        .arg(update)
        .arg("-C")
        .arg(scratch.path())
        .args(["--no-same-owner", "--no-same-permissions", "--"])
        .arg(format!("{stem}/lumiliocl"))
        .output()
        .map_err(|error| format!("cannot run tar: {error}"))?;
    if !archive.status.success() {
        return Err(format!(
            "cannot unpack update: {}",
            String::from_utf8_lossy(&archive.stderr).trim()
        ));
    }
    let replacement = scratch.path().join(stem).join("lumiliocl");
    if !replacement.is_file() {
        return Err("Linux update package contains no lumiliocl executable".to_owned());
    }
    let staged = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    fs::copy(replacement, staged.path()).map_err(|error| error.to_string())?;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(0o755))
        .map_err(|error| error.to_string())?;
    staged
        .persist(executable)
        .map_err(|error| error.error.to_string())?;
    Ok(Relaunch::Restart(executable.to_owned()))
}
