use crate::Relaunch;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Replaces the running bundle with the one in the disk image. The launcher
/// keeps running; the new version starts on the next launch.
pub(super) fn install(executable: &Path, update: &Path) -> Result<Relaunch, String> {
    let app = executable
        .ancestors()
        .find(|path| path.extension().is_some_and(|extension| extension == "app"))
        .ok_or_else(|| "LumilioCL is not running from an app bundle".to_owned())?;
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let mount = scratch.path().join("mount");
    std::fs::create_dir(&mount).map_err(|error| error.to_string())?;
    run(Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-mountpoint"])
        .arg(&mount)
        .arg(update))?;
    // The same approach as Zed's auto_update (crates/auto_update): rsync
    // writes each file beside its target and renames it into place, so the
    // running process keeps the files it has open, and --delete drops files
    // that the new version no longer ships.
    let copied = mounted_app(&mount).and_then(|source| {
        run(Command::new("rsync")
            .args(["-a", "--delete"])
            .arg(contents_of(&source))
            .arg(app))
    });
    let detached = run(Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount));
    copied?;
    detached?;
    Ok(Relaunch::Restart(app.to_owned()))
}

fn mounted_app(mount: &Path) -> Result<PathBuf, String> {
    let app = mount.join("LumilioCL.app");
    if app.is_dir() {
        Ok(app)
    } else {
        Err("disk image contains no LumilioCL.app".to_owned())
    }
}

/// rsync copies a source with a trailing slash as its contents, not as a
/// folder inside the target.
fn contents_of(folder: &Path) -> OsString {
    let mut path = folder.as_os_str().to_owned();
    path.push("/");
    path
}

fn run(command: &mut Command) -> Result<(), String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}",
            command.get_program().to_string_lossy(),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
