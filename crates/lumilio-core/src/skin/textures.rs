use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

use image::{ImageFormat, ImageReader, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::fetch::fetch_document;
use crate::transfer::{TransferEngine, Transport};

/// LittleSkin's CustomSkinLoader API, built in.
pub const LITTLE_SKIN_CSL: &str = "https://littleskin.cn/csl";
/// A skin or cape picture is small; a larger file is not one.
pub(super) const PICTURE_LIMIT: u64 = 2 * 1024 * 1024;
/// Skins are 64×32 or 64×64, HD packs go higher; beyond this it is not a skin.
pub(super) const MAX_SIDE: u32 = 4096;

/// How the arms are shaped.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkinModel {
    /// The classic 4-pixel arms (HMCL's `default`, "Steve").
    #[default]
    Wide,
    /// The 3-pixel arms ("Alex").
    Slim,
}

/// What an offline player looks like. No choice at all is the game's own
/// default skin.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkinChoice {
    /// Picture files on this computer.
    Local {
        #[serde(default)]
        model: SkinModel,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skin: Option<PathBuf>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cape: Option<PathBuf>,
    },
    /// The player's profile on LittleSkin, by profile name.
    LittleSkin,
    /// A CustomSkinLoader API (`<api>/<name>.json`).
    Csl { api: String },
}

#[derive(Debug)]
pub enum SkinError {
    Io(io::Error),
    /// The file is not a picture a skin can be made of.
    Picture(String),
    Network(String),
    /// The skin site answered something that is not what the format says.
    Malformed(String),
    /// The CustomSkinLoader address is not an address.
    InvalidApi(String),
}

impl Display for SkinError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "skin file could not be read: {error}"),
            Self::Picture(why) => write!(f, "not a usable skin picture: {why}"),
            Self::Network(why) => write!(f, "could not reach the skin site: {why}"),
            Self::Malformed(why) => write!(f, "the skin site answered oddly: {why}"),
            Self::InvalidApi(input) => write!(f, "not a skin site address: {input}"),
        }
    }
}

impl Error for SkinError {}

impl From<io::Error> for SkinError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// A picture ready to be served: its name (a content hash) and PNG bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Texture {
    /// 64 lowercase hex digits, the address under `/textures/`.
    pub hash: String,
    pub png: Vec<u8>,
}

/// What the game will be shown.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct LoadedSkin {
    pub model: SkinModel,
    pub skin: Option<Texture>,
    pub cape: Option<Texture>,
}

/// HMCL's texture hash: SHA-256 over the size and every pixel as ARGB, column
/// by column, with the colour of a fully transparent pixel ignored.
pub(super) fn pixel_hash(image: &RgbaImage) -> String {
    let mut digest = Sha256::new();
    digest.update(i32::try_from(image.width()).unwrap_or(0).to_be_bytes());
    digest.update(i32::try_from(image.height()).unwrap_or(0).to_be_bytes());
    for x in 0..image.width() {
        for y in 0..image.height() {
            let [r, g, b, a] = image.get_pixel(x, y).0;
            let argb = if a == 0 {
                0_u32
            } else {
                u32::from(a) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            };
            digest.update(argb.to_be_bytes());
        }
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl Texture {
    /// Reads a picture, checks it is a skin-sized image and names it by its
    /// pixels. The bytes served are a fresh PNG of those pixels.
    pub fn from_picture(bytes: &[u8]) -> Result<Self, SkinError> {
        if bytes.len() as u64 > PICTURE_LIMIT {
            return Err(SkinError::Picture("the file is too large".to_owned()));
        }
        let mut reader = ImageReader::new(io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|error| SkinError::Picture(error.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(64 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|error| SkinError::Picture(error.to_string()))?
            .to_rgba8();
        let hash = pixel_hash(&image);
        let mut png = Vec::new();
        image
            .write_to(&mut io::Cursor::new(&mut png), ImageFormat::Png)
            .map_err(|error| SkinError::Picture(error.to_string()))?;
        Ok(Self { hash, png })
    }

    pub(super) fn from_file(path: &Path) -> Result<Self, SkinError> {
        if fs::metadata(path)?.len() > PICTURE_LIMIT {
            return Err(SkinError::Picture("the file is too large".to_owned()));
        }
        Self::from_picture(&fs::read(path)?)
    }
}

/// A CustomSkinLoader address made absolute: `https://` when none is given,
/// no trailing `/`.
pub fn normalize_api(input: &str) -> Result<String, SkinError> {
    let text = input.trim();
    let with_scheme = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| SkinError::InvalidApi(input.to_owned()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SkinError::InvalidApi(input.to_owned()));
    }
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Checks a choice can be used now (the files are pictures, the address is an
/// address), so a mistake shows in the dialog and not at launch.
pub fn check(choice: &SkinChoice) -> Result<(), SkinError> {
    match choice {
        SkinChoice::Local { skin, cape, .. } => {
            for path in [skin, cape].into_iter().flatten() {
                Texture::from_file(path)?;
            }
            Ok(())
        }
        SkinChoice::LittleSkin => Ok(()),
        SkinChoice::Csl { api } => normalize_api(api).map(|_| ()),
    }
}

pub(super) fn text_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
}

/// A texture hash is part of an address: letters and digits only.
pub(super) fn plain_hash(hash: &str) -> Result<&str, SkinError> {
    if !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        Ok(hash)
    } else {
        Err(SkinError::Malformed(
            "a texture name is not valid".to_owned(),
        ))
    }
}

pub(super) async fn fetch_picture(
    engine: &TransferEngine<impl Transport>,
    api: &str,
    hash: &str,
) -> Result<Texture, SkinError> {
    let url = format!("{api}/textures/{}", plain_hash(hash)?);
    let bytes = fetch_document(engine, &[url])
        .await
        .map_err(|error| SkinError::Network(error.to_string()))?;
    Texture::from_picture(&bytes)
}

/// The skin a CustomSkinLoader API has for `player` (none when it has no
/// profile by that name).
pub(super) async fn load_csl(
    engine: &TransferEngine<impl Transport>,
    api: &str,
    player: &str,
) -> Result<LoadedSkin, SkinError> {
    let document = fetch_document(engine, &[format!("{api}/{player}.json")])
        .await
        .map_err(|error| SkinError::Network(error.to_string()))?;
    let profile: Value = serde_json::from_slice(&document)
        .map_err(|_| SkinError::Malformed("the profile is not JSON".to_owned()))?;
    // A profile with no name is "no skin", as on the site itself.
    if text_of(&profile, "username").is_none() {
        return Ok(LoadedSkin::default());
    }
    let textures = profile.get("textures").or_else(|| profile.get("skins"));
    let in_textures = |key: &str| textures.and_then(|textures| text_of(textures, key));
    // The arm shape follows which skin the profile has; a profile that only
    // has the old flat `skin` field is the classic shape.
    let (model, skin_hash) = if let Some(hash) = in_textures("slim") {
        (SkinModel::Slim, Some(hash))
    } else if let Some(hash) = in_textures("default").or_else(|| text_of(&profile, "skin")) {
        (SkinModel::Wide, Some(hash))
    } else {
        (SkinModel::Wide, None)
    };
    let cape_hash = in_textures("cape").or_else(|| text_of(&profile, "cape"));
    let skin = match skin_hash {
        Some(hash) => Some(fetch_picture(engine, api, hash).await?),
        None => None,
    };
    let cape = match cape_hash {
        Some(hash) => Some(fetch_picture(engine, api, hash).await?),
        None => None,
    };
    Ok(LoadedSkin { model, skin, cape })
}

/// Loads what `choice` shows for the player called `player`.
pub async fn load(
    engine: &TransferEngine<impl Transport>,
    choice: &SkinChoice,
    player: &str,
) -> Result<LoadedSkin, SkinError> {
    match choice {
        SkinChoice::Local { model, skin, cape } => {
            let read = |path: &Option<PathBuf>| path.as_deref().map(Texture::from_file).transpose();
            Ok(LoadedSkin {
                model: *model,
                skin: read(skin)?,
                cape: read(cape)?,
            })
        }
        SkinChoice::LittleSkin => load_csl(engine, LITTLE_SKIN_CSL, player).await,
        SkinChoice::Csl { api } => load_csl(engine, &normalize_api(api)?, player).await,
    }
}
