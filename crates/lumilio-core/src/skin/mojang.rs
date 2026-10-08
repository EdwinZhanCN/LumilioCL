//! Minecraft profile appearance protocol. Endpoints and multipart fields
//! follow Modrinth `packages/app-lib/src/state/minecraft_skins/mojang_api.rs`
//! (Copyright Modrinth contributors, GPL-3.0-only; ADR 0022). Transport,
//! validation, errors and delayed confirmation belong to our core boundary.

use super::{AccountLook, SkinModel, cape_pixels, skin_pixels};
use crate::microsoft::{PROFILE_URL, Secret};
use crate::transfer::{HttpMethod, HttpRequest, Transport};
use futures_util::StreamExt as _;
use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppearanceError {
    NoGameOwnership,
    SignInRequired,
    Picture(String),
    Network(String),
    Protocol(String),
    Refused(u16),
    NotMicrosoft,
    CapeNotOwned,
    Storage(String),
    UnknownSkin,
    InvalidOrder,
}

impl Display for AppearanceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGameOwnership => f.write_str("the account has no Minecraft profile"),
            Self::SignInRequired => f.write_str("the Minecraft token is no longer valid"),
            Self::Picture(why) => write!(f, "invalid skin picture: {why}"),
            Self::Network(why) => write!(f, "appearance request failed: {why}"),
            Self::Protocol(why) => write!(f, "unexpected appearance response: {why}"),
            Self::Refused(status) => write!(f, "appearance request refused: HTTP {status}"),
            Self::NotMicrosoft => f.write_str("appearance changes require a Microsoft account"),
            Self::CapeNotOwned => f.write_str("the profile does not own that cape"),
            Self::Storage(why) => write!(f, "skin library storage failed: {why}"),
            Self::UnknownSkin => f.write_str("that skin is not in the local library"),
            Self::InvalidOrder => {
                f.write_str("skin order must contain each library entry exactly once")
            }
        }
    }
}
impl std::error::Error for AppearanceError {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MojangProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<MojangSkin>,
    #[serde(default)]
    pub capes: Vec<MojangCape>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MojangSkin {
    pub id: String,
    pub url: String,
    pub state: String,
    #[serde(rename = "variant", deserialize_with = "variant")]
    pub model: SkinModel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct MojangCape {
    pub id: String,
    pub url: String,
    pub state: String,
    pub alias: String,
}

fn variant<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<SkinModel, D::Error> {
    let value = String::deserialize(deserializer)?;
    match value.as_str() {
        "CLASSIC" => Ok(SkinModel::Wide),
        "SLIM" => Ok(SkinModel::Slim),
        _ => Err(serde::de::Error::custom("unknown skin variant")),
    }
}

impl MojangProfile {
    #[must_use]
    pub fn active_skin(&self) -> Option<&MojangSkin> {
        self.skins.iter().find(|skin| skin.state == "ACTIVE")
    }
    #[must_use]
    pub fn active_cape(&self) -> Option<&MojangCape> {
        self.capes.iter().find(|cape| cape.state == "ACTIVE")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppearanceChange {
    Upload { png: Vec<u8>, model: SkinModel },
    DefaultSkin,
    Cape(Option<String>),
}

/// A successful write is accepted even if its response has no profile or
/// still reports the previous look. Read again after the propagation delay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppearanceUpdate {
    pub profile: Option<MojangProfile>,
}

pub struct MojangClient<'a, T: Transport + ?Sized> {
    transport: &'a T,
}

impl<'a, T: Transport + ?Sized> MojangClient<'a, T> {
    #[must_use]
    pub const fn new(transport: &'a T) -> Self {
        Self { transport }
    }

    async fn send(
        &self,
        method: HttpMethod,
        suffix: &str,
        token: &Secret,
        content: Option<(String, Vec<u8>)>,
    ) -> Result<Vec<u8>, AppearanceError> {
        let mut headers = vec![
            ("authorization".into(), format!("Bearer {}", token.expose())),
            ("accept".into(), "application/json".into()),
        ];
        let body = content.map(|(kind, bytes)| {
            headers.push(("content-type".into(), kind));
            bytes
        });
        let response = self
            .transport
            .send(HttpRequest {
                method,
                url: format!("{PROFILE_URL}{suffix}"),
                headers,
                body,
            })
            .await
            .map_err(|error| AppearanceError::Network(error.to_string()))?;
        match response.status() {
            200..=299 => {}
            401 => return Err(AppearanceError::SignInRequired),
            404 => return Err(AppearanceError::NoGameOwnership),
            400 if suffix == "/skins" => {
                return Err(AppearanceError::Picture(
                    "server rejected the picture".into(),
                ));
            }
            status => return Err(AppearanceError::Refused(status)),
        }
        bounded_body(response, 1024 * 1024).await
    }

    pub async fn profile(&self, token: &Secret) -> Result<MojangProfile, AppearanceError> {
        let bytes = self.send(HttpMethod::Get, "", token, None).await?;
        serde_json::from_slice(&bytes).map_err(|error| AppearanceError::Protocol(error.to_string()))
    }

    /// PNG validation precedes every request, including legacy conversion.
    pub async fn change(
        &self,
        token: &Secret,
        change: AppearanceChange,
    ) -> Result<AppearanceUpdate, AppearanceError> {
        let (method, suffix, content) = match change {
            AppearanceChange::Upload { png, model } => {
                let png = normalize_png(&png)?;
                let mut random = [0u8; 16];
                getrandom::fill(&mut random)
                    .map_err(|error| AppearanceError::Protocol(error.to_string()))?;
                let boundary = format!(
                    "lumilio-{}",
                    random
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>()
                );
                let variant = if model == SkinModel::Slim {
                    "slim"
                } else {
                    "classic"
                };
                let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"variant\"\r\n\r\n{variant}\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"skin.png\"\r\nContent-Type: image/png\r\n\r\n").into_bytes();
                body.extend(png);
                body.extend(format!("\r\n--{boundary}--\r\n").as_bytes());
                (
                    HttpMethod::Post,
                    "/skins",
                    Some((format!("multipart/form-data; boundary={boundary}"), body)),
                )
            }
            AppearanceChange::DefaultSkin => (HttpMethod::Delete, "/skins/active", None),
            AppearanceChange::Cape(None) => (HttpMethod::Delete, "/capes/active", None),
            AppearanceChange::Cape(Some(id)) => {
                if !self
                    .profile(token)
                    .await?
                    .capes
                    .iter()
                    .any(|cape| cape.id == id)
                {
                    return Err(AppearanceError::CapeNotOwned);
                }
                (
                    HttpMethod::Put,
                    "/capes/active",
                    Some((
                        "application/json".into(),
                        serde_json::json!({"capeId": id}).to_string().into_bytes(),
                    )),
                )
            }
        };
        let bytes = self.send(method, suffix, token, content).await?;
        Ok(AppearanceUpdate {
            profile: serde_json::from_slice(&bytes).ok(),
        })
    }

    /// Texture requests never carry the account's bearer token.
    pub async fn skin_png(&self, skin: &MojangSkin) -> Result<Vec<u8>, AppearanceError> {
        normalize_png(&self.texture(&skin.url).await?)
    }

    /// Texture requests never carry the account's bearer token.
    pub async fn look(&self, profile: &MojangProfile) -> Result<AccountLook, AppearanceError> {
        let mut look = AccountLook::default();
        if let Some(skin) = profile.active_skin() {
            look.model = skin.model;
            look.skin = Some(
                skin_pixels(&self.texture(&skin.url).await?)
                    .map_err(|error| AppearanceError::Picture(error.to_string()))?,
            );
        }
        if let Some(cape) = profile.active_cape() {
            look.cape = Some(
                cape_pixels(&self.texture(&cape.url).await?)
                    .map_err(|error| AppearanceError::Picture(error.to_string()))?,
            );
        }
        Ok(look)
    }

    async fn texture(&self, address: &str) -> Result<Vec<u8>, AppearanceError> {
        let mut url = url::Url::parse(address)
            .map_err(|error| AppearanceError::Protocol(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str() != Some("textures.minecraft.net")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return Err(AppearanceError::Protocol(
                "untrusted Mojang texture address".into(),
            ));
        }
        // Profile URLs historically use HTTP; fetch their HTTPS equivalent.
        url.set_scheme("https")
            .map_err(|()| AppearanceError::Protocol("invalid texture scheme".into()))?;
        let response = self
            .transport
            .get(url.as_str())
            .await
            .map_err(|error| AppearanceError::Network(error.to_string()))?;
        if response.status() != 200 {
            return Err(AppearanceError::Refused(response.status()));
        }
        bounded_body(response, super::PICTURE_LIMIT as usize).await
    }
}

async fn bounded_body(
    response: crate::transfer::TransportResponse,
    limit: usize,
) -> Result<Vec<u8>, AppearanceError> {
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|error| AppearanceError::Network(error.to_string()))?;
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(AppearanceError::Protocol(
                "response exceeds size limit".into(),
            ));
        }
        bytes.extend(chunk);
    }
    Ok(bytes)
}

pub(super) fn normalize_png(bytes: &[u8]) -> Result<Vec<u8>, AppearanceError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(AppearanceError::Picture("skin must be a PNG".into()));
    }
    let pixels = skin_pixels(bytes).map_err(|error| AppearanceError::Picture(error.to_string()))?;
    if pixels.width != 64 || pixels.height != 64 {
        return Err(AppearanceError::Picture(
            "Mojang skins must be 64×64 or legacy 64×32".into(),
        ));
    }
    let image = image::RgbaImage::from_raw(64, 64, pixels.rgba).expect("validated skin pixels");
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| AppearanceError::Picture(error.to_string()))?;
    Ok(png)
}

#[cfg(test)]
mod tests;
