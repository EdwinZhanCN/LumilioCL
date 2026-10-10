//! A portable ZIP and an Inno Setup installer around the same executable
//! (PACKAGING.md §3). The icon and VERSIONINFO are already inside the exe
//! (`lumilio-app/build.rs`).
//!
//! Signing order: the exe, then the packages, then `setup.exe` and its
//! uninstaller. `LUMILIO_WINDOWS_SIGN_COMMAND` is the signing command line
//! with `$f` where the file goes, e.g.
//! `signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /f cert.pfx /p … $f`;
//! Inno runs the same command for what it builds. Keep paths in it free of
//! spaces: Inno receives it inside one quoted argument. Without it nothing is
//! signed and SmartScreen warns on first run.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::release::{
    self, APP_ID, APP_NAME, BINARY, HOMEPAGE, Release, Result, copy, run, run_labelled, write,
};

const INSTALLER_SCRIPT: &str = include_str!("../packaging/LumilioCL.iss");

pub fn package(release: &Release, skip_build: bool) -> Result<Vec<PathBuf>> {
    if skip_build {
        return Err("Windows packages need separate installer and portable builds".to_owned());
    }
    if !skip_build {
        release.build(&[])?;
    }
    let signer = std::env::var("LUMILIO_WINDOWS_SIGN_COMMAND")
        .ok()
        .filter(|command| !command.trim().is_empty());
    let stage = release.stage("windows")?;
    let payload = stage.join(APP_NAME);
    let exe = payload.join(format!("{BINARY}.exe"));
    copy(&release.binary(), &exe)?;
    if let Some(signer) = &signer {
        run_labelled(
            &mut sign_command(signer, &exe),
            &format!("sign {}", exe.display()),
        )?;
    }
    copy(&release.license(), &payload.join("LICENSE.txt"))?;
    release.notices(&payload)?;
    let signature = if signer.is_some() {
        "Authenticode"
    } else {
        "unsigned"
    };
    write(
        &payload.join("BUILD.txt"),
        release.build_info("windows", signature),
    )?;

    let stem = release.stem("windows");
    let portable = release.dist_file(&format!("{stem}-portable.zip"))?;
    release.build(&[("LUMILIO_UPDATE_EXPLANATION", "portable-windows")])?;
    let portable_payload = stage.join("portable");
    let portable_exe = portable_payload.join(format!("{BINARY}.exe"));
    copy(&release.binary(), &portable_exe)?;
    if let Some(signer) = &signer {
        run_labelled(
            &mut sign_command(signer, &portable_exe),
            &format!("sign {}", portable_exe.display()),
        )?;
    }
    copy(&release.license(), &portable_payload.join("LICENSE.txt"))?;
    release.notices(&portable_payload)?;
    write(
        &portable_payload.join("BUILD.txt"),
        release.build_info("windows", signature),
    )?;
    zip_folder(&portable_payload, APP_NAME, &portable)?;

    let script = stage.join("LumilioCL.iss");
    write(&script, INSTALLER_SCRIPT)?;
    let setup_base = format!("{stem}-setup");
    let setup = release.dist_file(&format!("{setup_base}.exe"))?;
    let mut iscc = Command::new(iscc()?);
    iscc.arg("/Qp");
    for (name, value) in [
        ("AppVersion", release.version.to_string()),
        ("NumericVersion", release.version.windows()),
        ("AppId", APP_ID.to_owned()),
        ("Homepage", HOMEPAGE.to_owned()),
        ("SourceDir", payload.display().to_string()),
        ("OutputDir", release.dist.display().to_string()),
        ("OutputBase", setup_base),
        (
            "IconFile",
            release
                .icons()
                .join("windows/LumilioCL.ico")
                .display()
                .to_string(),
        ),
    ] {
        iscc.arg(format!("/D{name}={value}"));
    }
    match &signer {
        Some(signer) => {
            iscc.arg("/DSign");
            raw_arg(&mut iscc, &format!("\"/Slumilio={signer}\""));
            iscc.arg(&script);
            run_labelled(&mut iscc, "iscc (signing the installer and uninstaller)")?;
        }
        None => run(iscc.arg(&script))?,
    }
    if !setup.is_file() {
        return Err(format!("Inno Setup did not write {}", setup.display()));
    }
    if signer.is_none() {
        eprintln!("warning: unsigned; SmartScreen will warn on first run");
    }
    Ok(vec![setup, portable])
}

/// The signing command for one file. It runs through `cmd` so the configured
/// line keeps its own quoting, exactly as Inno will run it.
fn sign_command(signer: &str, file: &Path) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C");
    raw_arg(
        &mut command,
        &signer.replace("$f", &format!("\"{}\"", file.display())),
    );
    command
}

#[cfg(windows)]
fn raw_arg(command: &mut Command, arg: &str) {
    use std::os::windows::process::CommandExt as _;
    command.raw_arg(arg);
}

#[cfg(not(windows))]
fn raw_arg(command: &mut Command, arg: &str) {
    command.arg(arg);
}

/// `ISCC.exe` from `PATH` or Inno Setup 6's usual install folders.
fn iscc() -> Result<PathBuf> {
    if release::on_path("iscc") {
        return Ok(PathBuf::from("iscc"));
    }
    ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .flat_map(|base| {
            [
                base.join("Inno Setup 6/ISCC.exe"),
                base.join("Programs/Inno Setup 6/ISCC.exe"),
            ]
        })
        .find(|path| path.is_file())
        .ok_or_else(|| "Inno Setup 6 (ISCC.exe) is not installed".to_owned())
}

/// Zips `folder`'s files under `prefix/`, so the archive unpacks into one folder.
pub fn zip_folder(folder: &Path, prefix: &str, archive: &Path) -> Result {
    let file = fs::File::create(archive).map_err(|error| release::io("create", archive, error))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    add_to_zip(&mut zip, folder, prefix, options)?;
    zip.finish()
        .map_err(|error| format!("cannot finish {}: {error}", archive.display()))?;
    Ok(())
}

fn add_to_zip(
    zip: &mut zip::ZipWriter<fs::File>,
    folder: &Path,
    prefix: &str,
    options: zip::write::SimpleFileOptions,
) -> Result {
    let mut entries: Vec<_> = fs::read_dir(folder)
        .map_err(|error| release::io("read", folder, error))?
        .filter_map(|entry| entry.ok())
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        if path.is_dir() {
            add_to_zip(zip, &path, &name, options)?;
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| release::io("read", &path, error))?;
        zip.start_file(name.as_str(), options)
            .and_then(|()| zip.write_all(&bytes).map_err(Into::into))
            .map_err(|error| format!("cannot add {name} to the archive: {error}"))?;
    }
    Ok(())
}
