//! The screenshots an instance has taken (`<game>/screenshots`).
//!
//! Listing reads only file metadata. Thumbnails are small PNG copies in the
//! launcher's own folder, named after the file's identity (name, size and
//! modification time), so an edited or replaced screenshot never shows a
//! stale one and nothing is ever written into the game directory.

use crate::discover::is_safe_file_name;
use image::ImageReader;
use image::imageops::FilterType;
use sha1::{Digest, Sha1};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const DIRECTORY: &str = "screenshots";
/// Longest edge of a thumbnail, in pixels (a card shows it at half that).
pub const THUMBNAIL_EDGE: u32 = 480;
/// A screenshot is decoded only up to this many bytes of pixels.
const DECODE_LIMIT: u64 = 256 * 1024 * 1024;
/// Largest file the launcher will read for a copy to the clipboard.
const READ_LIMIT: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenshotInfo {
    /// The file name under `screenshots/` (its identity).
    pub file: String,
    /// Where the picture is, for showing it at full size.
    pub path: PathBuf,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: i64,
    pub size: u64,
}

#[derive(Debug)]
pub enum ScreenshotError {
    Io(io::Error),
    UnsafeName(String),
    NotFound(String),
    /// `screenshots/` is a link, so it may lead outside the instance.
    Linked,
    /// The file is not an image the launcher can read.
    Unreadable(String),
}

impl Display for ScreenshotError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "screenshot storage failed: {error}"),
            Self::UnsafeName(name) => write!(f, "unsafe screenshot name {name:?}"),
            Self::NotFound(name) => write!(f, "no screenshot {name:?}"),
            Self::Linked => {
                f.write_str("screenshots is a link; it could lead outside the instance")
            }
            Self::Unreadable(why) => write!(f, "cannot read the picture: {why}"),
        }
    }
}

impl Error for ScreenshotError {}

impl From<io::Error> for ScreenshotError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

fn is_picture(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            ["png", "jpg", "jpeg"]
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
        })
}

/// `screenshots/`, refused when it is a link out of the instance.
fn directory(game_dir: &Path) -> Result<PathBuf, ScreenshotError> {
    let path = game_dir.join(DIRECTORY);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(ScreenshotError::Linked),
        _ => Ok(path),
    }
}

/// One screenshot's file, checked to be a plain file of that directory.
fn locate(game_dir: &Path, file: &str) -> Result<PathBuf, ScreenshotError> {
    if !is_safe_file_name(file) || !is_picture(file) {
        return Err(ScreenshotError::UnsafeName(file.to_owned()));
    }
    let path = directory(game_dir)?.join(file);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() => Ok(path),
        Ok(_) => Err(ScreenshotError::UnsafeName(file.to_owned())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(ScreenshotError::NotFound(file.to_owned()))
        }
        Err(error) => Err(error.into()),
    }
}

fn modified_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|elapsed| i64::try_from(elapsed.as_millis()).ok())
        .unwrap_or(0)
}

/// The pictures in `screenshots/`, newest first. A missing folder is an empty list.
pub fn scan(game_dir: &Path) -> Result<Vec<ScreenshotInfo>, ScreenshotError> {
    let entries = match fs::read_dir(directory(game_dir)?) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut shots = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Ok(file) = entry.file_name().into_string() else {
            continue;
        };
        // `file_type` of a link is the link itself: links are not listed.
        if file.starts_with('.') || !is_picture(&file) || !entry.file_type()?.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        shots.push(ScreenshotInfo {
            path: entry.path(),
            file,
            modified_ms: modified_ms(&metadata),
            size: metadata.len(),
        });
    }
    shots.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| b.file.cmp(&a.file))
    });
    Ok(shots)
}

/// Where a thumbnail of `info` lives (it may not exist yet).
#[must_use]
pub fn thumbnail_path(cache_dir: &Path, info: &ScreenshotInfo) -> PathBuf {
    let mut hasher = Sha1::new();
    hasher.update(info.file.as_bytes());
    hasher.update(info.size.to_be_bytes());
    hasher.update(info.modified_ms.to_be_bytes());
    hasher.update(THUMBNAIL_EDGE.to_be_bytes());
    let name: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    cache_dir.join(format!("{name}.png"))
}

/// The thumbnail of one screenshot, made now if it is not there yet.
pub fn thumbnail(
    game_dir: &Path,
    cache_dir: &Path,
    file: &str,
) -> Result<PathBuf, ScreenshotError> {
    let source = locate(game_dir, file)?;
    let metadata = fs::metadata(&source)?;
    let info = ScreenshotInfo {
        file: file.to_owned(),
        path: source.clone(),
        modified_ms: modified_ms(&metadata),
        size: metadata.len(),
    };
    let target = thumbnail_path(cache_dir, &info);
    if target.is_file() {
        return Ok(target);
    }
    let mut reader = ImageReader::open(&source)?
        .with_guessed_format()
        .map_err(|error| ScreenshotError::Unreadable(error.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(DECODE_LIMIT);
    reader.limits(limits);
    let picture = reader
        .decode()
        .map_err(|error| ScreenshotError::Unreadable(error.to_string()))?;
    let small = picture.resize(THUMBNAIL_EDGE, THUMBNAIL_EDGE, FilterType::Triangle);
    fs::create_dir_all(cache_dir)?;
    // Written whole, then renamed in: a half-written file is never taken for a thumbnail.
    let temporary = target.with_extension("png.tmp");
    small
        .save_with_format(&temporary, image::ImageFormat::Png)
        .map_err(|error| ScreenshotError::Unreadable(error.to_string()))?;
    fs::rename(&temporary, &target).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })?;
    Ok(target)
}

/// The picture's bytes, for copying it elsewhere.
pub fn read(game_dir: &Path, file: &str) -> Result<Vec<u8>, ScreenshotError> {
    let path = locate(game_dir, file)?;
    if fs::metadata(&path)?.len() > READ_LIMIT {
        return Err(ScreenshotError::Unreadable("the file is too large".into()));
    }
    Ok(fs::read(path)?)
}

/// Deletes one screenshot (and nothing else).
pub fn delete(game_dir: &Path, file: &str) -> Result<(), ScreenshotError> {
    let path = locate(game_dir, file)?;
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests;
