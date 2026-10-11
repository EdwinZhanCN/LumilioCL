use super::install;
use crate::Relaunch;
use std::fs;
use std::io::{Read as _, Seek as _};
use std::path::Path;
use std::process::Command;

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn read_from_start(file: &mut fs::File) -> String {
    let mut contents = String::new();
    file.rewind().unwrap();
    file.read_to_string(&mut contents).unwrap();
    contents
}

fn succeed(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "macos")]
#[test]
fn the_bundle_is_replaced_beside_the_running_files_and_restarts_from_the_bundle() {
    let scratch = tempfile::tempdir().unwrap();
    let image_source = scratch.path().join("image");
    write(
        &image_source.join("LumilioCL.app/Contents/MacOS/lumiliocl"),
        "new",
    );
    write(
        &image_source.join("LumilioCL.app/Contents/Resources/kept.txt"),
        "new",
    );
    let image = scratch.path().join("LumilioCL-0.1.2-macos-arm64.dmg");
    succeed(
        Command::new("hdiutil")
            .args([
                "create",
                "-quiet",
                "-format",
                "UDZO",
                "-volname",
                "LumilioCL",
            ])
            .arg("-srcfolder")
            .arg(&image_source)
            .arg(&image),
    );

    let app = scratch.path().join("Applications/LumilioCL.app");
    let executable = app.join("Contents/MacOS/lumiliocl");
    write(&executable, "old");
    write(&app.join("Contents/Resources/stale.txt"), "old");
    let mut running = fs::File::open(&executable).unwrap();

    let relaunch = install(&executable, &image).unwrap();

    assert_eq!(relaunch, Relaunch::Restart(app.clone()));
    assert_eq!(fs::read_to_string(&executable).unwrap(), "new");
    assert!(app.join("Contents/Resources/kept.txt").is_file());
    assert!(
        !app.join("Contents/Resources/stale.txt").exists(),
        "files the new version no longer ships are removed"
    );
    assert!(
        !app.join("LumilioCL.app").exists(),
        "the bundle's contents are copied, not the bundle into itself"
    );
    assert_eq!(
        read_from_start(&mut running),
        "old",
        "the running process keeps the file it opened"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn the_executable_is_replaced_atomically_and_restarts_from_its_original_path() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = tempfile::tempdir().unwrap();
    let stem = "LumilioCL-0.1.2-linux-x64";
    write(&scratch.path().join(stem).join("lumiliocl"), "new");
    let archive = scratch.path().join(format!("{stem}.tar.gz"));
    succeed(
        Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(scratch.path())
            .arg(stem),
    );

    let executable = scratch.path().join("lib/lumiliocl/lumiliocl");
    write(&executable, "old");
    let mut running = fs::File::open(&executable).unwrap();

    let relaunch = install(&executable, &archive).unwrap();

    assert_eq!(relaunch, Relaunch::Restart(executable.clone()));
    assert_eq!(fs::read_to_string(&executable).unwrap(), "new");
    assert_eq!(
        fs::metadata(&executable).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(
        read_from_start(&mut running),
        "old",
        "the running process keeps the file it opened"
    );
}
