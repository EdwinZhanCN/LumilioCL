//! The Home wallpaper: a picture the person chose to replace the procedural
//! world behind Home.
//!
//! Choosing copies the picture into the launcher's own folder, scaled down to
//! what a window can show, so the original can move or go and a huge photo is
//! never held as a texture. The copy is named after its content, so a new
//! picture is a new file and nothing shows a stale one.

use image::imageops::FilterType;
use image::{ImageReader, codecs::jpeg::JpegEncoder};
use sha1::{Digest, Sha1};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::Path;

/// The widest and tallest a stored wallpaper gets, in pixels.
pub const MAX_WIDTH: u32 = 2560;
pub const MAX_HEIGHT: u32 = 1600;
/// The largest picture file read.
const SOURCE_LIMIT: u64 = 64 * 1024 * 1024;
/// A picture is decoded only up to this many bytes of pixels.
const DECODE_LIMIT: u64 = 512 * 1024 * 1024;
const JPEG_QUALITY: u8 = 92;
const PREFIX: &str = "wallpaper-";

#[derive(Debug)]
pub enum WallpaperError {
    Io(io::Error),
    /// The file is not a picture the launcher can read.
    Unreadable(String),
    TooLarge(u64),
}

impl Display for WallpaperError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "wallpaper storage failed: {error}"),
            Self::Unreadable(why) => write!(f, "cannot read the picture: {why}"),
            Self::TooLarge(size) => write!(f, "the picture is {size} bytes, over the limit"),
        }
    }
}

impl Error for WallpaperError {}

impl From<io::Error> for WallpaperError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Copies `source` into `dir` as the wallpaper and returns the stored file's
/// name. Earlier wallpapers in `dir` are removed once the new one is in place.
/// Blocking: call it off the UI thread.
pub fn import_wallpaper(source: &Path, dir: &Path) -> Result<String, WallpaperError> {
    let size = fs::metadata(source)?.len();
    if size > SOURCE_LIMIT {
        return Err(WallpaperError::TooLarge(size));
    }
    let unreadable = |error: &dyn Display| WallpaperError::Unreadable(error.to_string());
    let mut reader = ImageReader::open(source)?
        .with_guessed_format()
        .map_err(|error| unreadable(&error))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(DECODE_LIMIT);
    reader.limits(limits);
    let mut picture = reader.decode().map_err(|error| unreadable(&error))?;
    if picture.width() > MAX_WIDTH || picture.height() > MAX_HEIGHT {
        picture = picture.resize(MAX_WIDTH, MAX_HEIGHT, FilterType::Lanczos3);
    }
    let rgb = picture.to_rgb8();
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)
        .encode_image(&rgb)
        .map_err(|error| unreadable(&error))?;

    let digest = Sha1::digest(&bytes);
    let name = format!(
        "{PREFIX}{:02x}{:02x}{:02x}{:02x}.jpg",
        digest[0], digest[1], digest[2], digest[3]
    );
    fs::create_dir_all(dir)?;
    let target = dir.join(&name);
    let partial = dir.join(format!("{name}.partial"));
    fs::write(&partial, &bytes)?;
    fs::rename(&partial, &target)?;
    remove_wallpapers(dir, Some(&name));
    Ok(name)
}

/// Deletes the stored wallpapers in `dir` except `keep`. Best effort: a file
/// that cannot be removed is only wasted space.
pub fn remove_wallpapers(dir: &Path, keep: Option<&str>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name();
        let Some(file) = file.to_str() else {
            continue;
        };
        if file.starts_with(PREFIX) && Some(file) != keep {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests;
