//! Read-only Yggdrasil session textures. No login token is sent to a texture
//! server. The profile UUID must match the account that requested it.
use super::{AccountLook, SkinError, SkinModel, cape_pixels, skin_pixels};
use crate::{account::ProfileId, transfer::Transport};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt as _;
use serde_json::Value;

async fn read(
    transport: &(impl Transport + ?Sized),
    address: &str,
    limit: usize,
) -> Result<Vec<u8>, SkinError> {
    let url = url::Url::parse(address).map_err(|_| SkinError::InvalidApi(address.into()))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(SkinError::InvalidApi(address.into()));
    }
    let response = transport
        .get(url.as_str())
        .await
        .map_err(|e| SkinError::Network(e.to_string()))?;
    if response.status() == 204 || response.status() == 404 {
        return Ok(Vec::new());
    }
    if !(200..300).contains(&response.status()) {
        return Err(SkinError::Network(format!("HTTP {}", response.status())));
    }
    let mut stream = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend(chunk.map_err(|e| SkinError::Network(e.to_string()))?);
        if bytes.len() > limit {
            return Err(SkinError::Malformed("answer exceeds size limit".into()));
        }
    }
    Ok(bytes)
}

pub(crate) async fn load(
    transport: &(impl Transport + ?Sized),
    root: &str,
    id: ProfileId,
) -> Result<AccountLook, SkinError> {
    let address = format!(
        "{}/sessionserver/session/minecraft/profile/{}",
        root.trim_end_matches('/'),
        id.compact()
    );
    let bytes = read(transport, &address, 1024 * 1024).await?;
    if bytes.is_empty() {
        return Ok(AccountLook::default());
    }
    let profile: Value = serde_json::from_slice(&bytes)
        .map_err(|_| SkinError::Malformed("profile is not JSON".into()))?;
    if profile["id"]
        .as_str()
        .and_then(|text| ProfileId::parse(text).ok())
        != Some(id)
    {
        return Err(SkinError::Malformed(
            "profile belongs to another account".into(),
        ));
    }
    let property = profile["properties"]
        .as_array()
        .and_then(|properties| properties.iter().find(|p| p["name"] == "textures"));
    let Some(encoded) = property.and_then(|p| p["value"].as_str()) else {
        return Ok(AccountLook::default());
    };
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| SkinError::Malformed("textures are not base64".into()))?;
    let textures: Value = serde_json::from_slice(&bytes)
        .map_err(|_| SkinError::Malformed("textures are not JSON".into()))?;
    let skin = &textures["textures"]["SKIN"];
    let cape = &textures["textures"]["CAPE"];
    let mut look = AccountLook {
        model: if skin["metadata"]["model"] == "slim" {
            SkinModel::Slim
        } else {
            SkinModel::Wide
        },
        ..AccountLook::default()
    };
    if let Some(url) = skin["url"].as_str() {
        look.skin = Some(skin_pixels(&read(transport, url, 2 * 1024 * 1024).await?)?)
    }
    if let Some(url) = cape["url"].as_str() {
        look.cape = Some(cape_pixels(&read(transport, url, 2 * 1024 * 1024).await?)?)
    }
    Ok(look)
}

#[cfg(test)]
mod tests;
