//! A `.tar.gz` with a per-user `install.sh`, and a `.deb`, built from the
//! same files (PACKAGING.md §4). Build on the oldest distribution supported:
//! the binary needs that glibc or newer.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::release::{
    self, APP_ID, APP_NAME, BINARY, HOMEPAGE, Release, Result, copy, copy_tree, create_dir,
    on_path, read, run, set_mode, symlink, write,
};

const INSTALL_SH: &str = include_str!("../packaging/install.sh");
pub const MAINTAINER: &str = "EdwinZhan <EdwinZhanCN@users.noreply.github.com>";
/// The renderer loads Vulkan at run time, so `dpkg-shlibdeps` cannot see it.
const LOADED_AT_RUN_TIME: &str = "libvulkan1";

pub fn package(release: &Release, skip_build: bool) -> Result<Vec<PathBuf>> {
    if !skip_build {
        release.build(&[])?;
    }
    let desktop = release.icons().join(format!("linux/{APP_ID}.desktop"));
    if on_path("desktop-file-validate") {
        run(Command::new("desktop-file-validate").arg(&desktop))?;
    } else {
        eprintln!(
            "warning: desktop-file-validate is not installed; the .desktop file is unchecked"
        );
    }
    let stage = release.stage("linux")?;
    let mut artifacts = vec![tarball(release, &stage, &desktop)?];
    if on_path("dpkg-deb") && on_path("dpkg-shlibdeps") {
        artifacts.push(deb(release, &stage, &desktop)?);
    } else {
        eprintln!("warning: dpkg-deb or dpkg-shlibdeps missing; no .deb this time");
    }
    Ok(artifacts)
}

fn tarball(release: &Release, stage: &Path, desktop: &Path) -> Result<PathBuf> {
    let stem = release.stem("linux");
    let folder = stage.join(&stem);
    copy(&release.binary(), &folder.join(BINARY))?;
    set_mode(&folder.join(BINARY), 0o755)?;
    write(&folder.join("install.sh"), INSTALL_SH)?;
    set_mode(&folder.join("install.sh"), 0o755)?;
    copy(
        desktop,
        &folder.join(format!("share/applications/{APP_ID}.desktop")),
    )?;
    copy_tree(
        &release.icons().join("linux/hicolor"),
        &folder.join("share/icons/hicolor"),
    )?;
    copy(&release.license(), &folder.join("LICENSE"))?;
    release.notices(&folder)?;
    write(
        &folder.join("BUILD.txt"),
        release.build_info("linux", "unsigned"),
    )?;
    let archive = release.dist_file(&format!("{stem}.tar.gz"))?;
    run(Command::new("tar")
        .arg("-C")
        .arg(stage)
        .args(["--owner=0", "--group=0", "--numeric-owner", "-czf"])
        .arg(&archive)
        .arg(&stem))?;
    Ok(archive)
}

fn deb(release: &Release, stage: &Path, desktop: &Path) -> Result<PathBuf> {
    let root = stage.join("deb");
    let installed = root.join(format!("usr/lib/lumiliocl/{BINARY}"));
    copy(&release.binary(), &installed)?;
    set_mode(&installed, 0o755)?;
    create_dir(&root.join("usr/bin"))?;
    symlink(
        Path::new(&format!("../lib/lumiliocl/{BINARY}")),
        &root.join(format!("usr/bin/{BINARY}")),
    )?;
    copy(
        desktop,
        &root.join(format!("usr/share/applications/{APP_ID}.desktop")),
    )?;
    copy_tree(
        &release.icons().join("linux/hicolor"),
        &root.join("usr/share/icons/hicolor"),
    )?;
    let doc = root.join("usr/share/doc/lumiliocl");
    copy(&release.license(), &doc.join("copyright"))?;
    release.notices(&doc)?;
    write(
        &doc.join("BUILD.txt"),
        release.build_info("linux", "unsigned"),
    )?;

    let architecture = read(Command::new("dpkg").arg("--print-architecture"))?
        .trim()
        .to_owned();
    let depends = shared_library_depends(stage, &installed)?;
    let size = installed_size_kib(&root)?;
    write(
        &root.join("DEBIAN/control"),
        control(release, &architecture, &depends, size),
    )?;
    let package = release.dist_file(&format!("{}.deb", release.stem("linux")))?;
    run(Command::new("dpkg-deb")
        .args(["--root-owner-group", "-Zxz", "--build"])
        .arg(&root)
        .arg(&package))?;
    Ok(package)
}

/// `Depends` as `dpkg-shlibdeps` derives it from the binary's linked
/// libraries, plus what is loaded at run time.
fn shared_library_depends(stage: &Path, binary: &Path) -> Result<String> {
    // dpkg-shlibdeps insists on running inside a source package.
    let work = stage.join("shlibdeps");
    write(
        &work.join("debian/control"),
        "Source: lumiliocl\n\nPackage: lumiliocl\nArchitecture: any\n",
    )?;
    let output = read(
        Command::new("dpkg-shlibdeps")
            .current_dir(&work)
            .arg("-O")
            .arg(format!("-e{}", binary.display())),
    )?;
    let linked = output
        .lines()
        .find_map(|line| line.strip_prefix("shlibs:Depends="))
        .ok_or_else(|| format!("dpkg-shlibdeps gave no dependencies: {output}"))?;
    Ok(format!("{}, {LOADED_AT_RUN_TIME}", linked.trim()))
}

fn installed_size_kib(root: &Path) -> Result<u64> {
    fn walk(path: &Path, total: &mut u64) -> Result {
        for entry in fs::read_dir(path).map_err(|error| release::io("read", path, error))? {
            let entry = entry.map_err(|error| release::io("read", path, error))?;
            let meta = entry
                .path()
                .symlink_metadata()
                .map_err(|error| release::io("inspect", &entry.path(), error))?;
            if meta.is_dir() {
                walk(&entry.path(), total)?;
            } else {
                *total += meta.len().div_ceil(1024);
            }
        }
        Ok(())
    }
    let mut total = 0;
    walk(root, &mut total)?;
    Ok(total)
}

pub fn control(release: &Release, architecture: &str, depends: &str, size_kib: u64) -> String {
    format!(
        "Package: lumiliocl\n\
         Version: {version}\n\
         Architecture: {architecture}\n\
         Maintainer: {MAINTAINER}\n\
         Installed-Size: {size_kib}\n\
         Depends: {depends}\n\
         Recommends: mesa-vulkan-drivers | vulkan-icd\n\
         Section: games\n\
         Priority: optional\n\
         Homepage: {HOMEPAGE}\n\
         Description: {APP_NAME}, a Minecraft launcher\n \
         Installs and launches Minecraft: game versions, mod loaders, mods and\n \
         modpacks, Java runtimes and accounts.\n",
        version = release.version.debian(),
    )
}
