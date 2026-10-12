//! Unpacking a JDK archive (`.tar.gz` or `.zip`) into a runtime folder.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// How an archive is packed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveKind {
    TarGz,
    Zip,
}

impl ArchiveKind {
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
        }
    }
}

fn refused(what: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("refused {what} in the Java archive"),
    )
}

/// Where an entry goes below the runtime folder: its path without the one
/// folder every JDK archive wraps its contents in (`graalvm-jdk-21…/`).
/// `None` for that folder itself; an error for anything that could leave
/// the runtime folder.
fn inner_path(raw: &Path) -> io::Result<Option<PathBuf>> {
    let mut parts = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            _ => return Err(refused(raw.display())),
        }
    }
    if parts.len() < 2 {
        return Ok(None);
    }
    Ok(Some(parts[1..].iter().collect()))
}

/// The `/`-separated form of a path inside the runtime, for link checks.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Unpacks `archive` into `into` without its top folder. Entries that would
/// land outside `into`, links that point out of it and special files are
/// refused; executable bits are kept.
pub fn unpack(archive: &Path, kind: ArchiveKind, into: &Path) -> io::Result<()> {
    fs::create_dir_all(into)?;
    match kind {
        ArchiveKind::TarGz => unpack_tar(archive, into),
        ArchiveKind::Zip => unpack_zip(archive, into),
    }
}

fn unpack_tar(archive: &Path, into: &Path) -> io::Result<()> {
    use tar::EntryType;
    let reader = flate2::read::GzDecoder::new(io::BufReader::new(fs::File::open(archive)?));
    let mut tar = tar::Archive::new(reader);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let raw = entry.path()?.into_owned();
        let Some(relative) = inner_path(&raw)? else {
            continue;
        };
        let target = into.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        match entry.header().entry_type() {
            EntryType::Directory => fs::create_dir_all(&target)?,
            EntryType::Regular | EntryType::Continuous => {
                entry.unpack(&target)?;
            }
            EntryType::Symlink => {
                let link = entry
                    .link_name()?
                    .ok_or_else(|| refused("a link without a target"))?
                    .into_owned();
                let link_text = slashed(&link);
                if link.is_absolute() || !super::link_stays_inside(&slashed(&relative), &link_text)
                {
                    return Err(refused(link.display()));
                }
                #[cfg(unix)]
                std::os::unix::fs::symlink(&link, &target)?;
            }
            EntryType::Link => {
                // A hard link names another entry of the archive.
                let link = entry
                    .link_name()?
                    .ok_or_else(|| refused("a link without a target"))?
                    .into_owned();
                let source = inner_path(&link)?.ok_or_else(|| refused(link.display()))?;
                fs::copy(into.join(source), &target)?;
            }
            // Extended headers are read by the tar reader itself; devices
            // and pipes have no place in a JDK.
            _ => {}
        }
    }
    Ok(())
}

fn unpack_zip(archive: &Path, into: &Path) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?).map_err(io::Error::other)?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(io::Error::other)?;
        let raw = file
            .enclosed_name()
            .ok_or_else(|| refused(file.name().to_owned()))?;
        let Some(relative) = inner_path(&raw)? else {
            continue;
        };
        let target = into.join(&relative);
        if file.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if file.is_symlink() {
            // Windows archives carry none; a link elsewhere is not needed to run.
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        io::copy(&mut file, &mut fs::File::create(&target)?)?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
