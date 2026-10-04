//! Finding an authentication server and reading what it says about itself
//! (`GET <api root>`), as HMCL's `AuthlibInjectorServer` does.

use std::collections::BTreeMap;

use futures_util::StreamExt as _;
use serde_json::Value;
use url::Url;

use super::{BODY_LIMIT, YggdrasilError, snippet};
use crate::transfer::Transport;

/// The LittleSkin Yggdrasil API, built in (HMCL bundles it the same way).
pub const LITTLE_SKIN_URL: &str = "https://littleskin.cn/api/yggdrasil/";

/// An authentication server as the launcher keeps it: its API root and the
/// facts from its metadata that the screens use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthServer {
    /// The API root, ending with `/`.
    pub url: String,
    /// `meta.serverName`; absent when the server does not say.
    pub name: Option<String>,
    /// `meta.feature.non_email_login`: sign in with a user name, not an email.
    pub non_email_login: bool,
    /// `meta.links` (home page, registration…).
    pub links: BTreeMap<String, String>,
}

impl AuthServer {
    /// What to call the server: its own name, else its address.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.url)
    }

    /// An address without TLS: passwords would cross the network in the clear.
    #[must_use]
    pub fn is_insecure(&self) -> bool {
        self.url.starts_with("http://")
    }

    /// Reads a metadata document fetched from `url`.
    pub fn from_metadata(url: &str, text: &str) -> Result<Self, YggdrasilError> {
        let value: Value = serde_json::from_str(text)
            .map_err(|_| YggdrasilError::Malformed(snippet(text.as_bytes())))?;
        let meta = value.get("meta").and_then(Value::as_object);
        let name = meta
            .and_then(|meta| meta.get("serverName"))
            .and_then(Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .map(str::to_owned);
        let links = meta
            .and_then(|meta| meta.get("links"))
            .and_then(Value::as_object)
            .map(|links| {
                links
                    .iter()
                    .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let non_email_login = meta
            .and_then(|meta| meta.get("feature.non_email_login"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(Self {
            url: with_slash(url),
            name,
            non_email_login,
            links,
        })
    }
}

/// LittleSkin, before its metadata has been read: the name is known.
#[must_use]
pub fn little_skin() -> AuthServer {
    AuthServer {
        url: LITTLE_SKIN_URL.to_owned(),
        name: Some("LittleSkin".to_owned()),
        non_email_login: false,
        links: BTreeMap::new(),
    }
}

fn with_slash(url: &str) -> String {
    if url.ends_with('/') {
        url.to_owned()
    } else {
        format!("{url}/")
    }
}

fn same_address(a: &str, b: &str) -> bool {
    with_slash(a) == with_slash(b)
}

/// The address a person typed, made into an absolute http(s) address: a
/// missing scheme is `https://`.
pub(super) fn normalize_address(input: &str) -> Result<String, YggdrasilError> {
    let text = input.trim();
    let text = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let url = Url::parse(&text)
        .map_err(|_| YggdrasilError::Malformed(format!("not an address: {input}")))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(YggdrasilError::Malformed(format!(
            "not an address: {input}"
        )));
    }
    Ok(url.to_string())
}

async fn read(
    transport: &(impl Transport + ?Sized),
    url: &str,
) -> Result<(Option<String>, String), YggdrasilError> {
    let response = transport
        .get(url)
        .await
        .map_err(|error| YggdrasilError::Network(error.to_string()))?;
    let status = response.status();
    let location = response
        .header("x-authlib-injector-api-location")
        .map(str::to_owned);
    let mut stream = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        bytes.extend(chunk.map_err(|error| YggdrasilError::Network(error.to_string()))?);
        if bytes.len() > BODY_LIMIT {
            return Err(YggdrasilError::Malformed(
                "the answer is too large".to_owned(),
            ));
        }
    }
    if !(200..300).contains(&status) {
        return Err(YggdrasilError::Network(format!(
            "the server answered {status}"
        )));
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| YggdrasilError::Malformed("the answer is not text".to_owned()))?;
    Ok((location, text))
}

/// The server's raw metadata document (also handed to authlib-injector so the
/// game does not ask again).
pub async fn fetch_metadata(
    transport: &(impl Transport + ?Sized),
    api_root: &str,
) -> Result<String, YggdrasilError> {
    Ok(read(transport, api_root).await?.1)
}

/// Finds the server a person means: the typed address, or the API location it
/// points to through the `x-authlib-injector-api-location` header.
pub async fn locate_server(
    transport: &(impl Transport + ?Sized),
    input: &str,
) -> Result<AuthServer, YggdrasilError> {
    let typed = normalize_address(input)?;
    let (location, mut text) = read(transport, &typed).await?;
    let mut url = typed.clone();
    if let Some(location) = location {
        let absolute = Url::parse(&typed)
            .and_then(|base| base.join(&location))
            .map_err(|_| {
                YggdrasilError::Malformed("the API location is not an address".to_owned())
            })?
            .to_string();
        if !same_address(&typed, &absolute) {
            text = read(transport, &absolute).await?.1;
            url = absolute;
        }
    }
    AuthServer::from_metadata(&url, &text)
}
