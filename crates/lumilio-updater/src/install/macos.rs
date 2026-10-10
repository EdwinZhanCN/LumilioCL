use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn install_and_restart(update: &Path) -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
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
    let copied =
        mounted_app(&mount).and_then(|source| run(Command::new("ditto").arg(source).arg(app)));
    let detached = run(Command::new("hdiutil")
        .args(["detach", "-quiet"])
        .arg(&mount));
    copied?;
    detached?;
    run(Command::new("open").arg("-n").arg(PathBuf::from(app)))
}

fn mounted_app(mount: &Path) -> Result<PathBuf, String> {
    let app = mount.join("LumilioCL.app");
    if app.is_dir() {
        Ok(app)
    } else {
        Err("disk image contains no LumilioCL.app".to_owned())
    }
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
