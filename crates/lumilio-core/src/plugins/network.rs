//! Permission-scoped HTTP over the launcher's transport. Mirrors replace an
//! authorized original URL only through the host-owned SourceChain.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::StreamExt;
use lumilio_plugin_api::{
    FetchMethod, FetchRequest, FetchResponse, Manifest, Permission, PluginError,
};
use tokio::runtime::Handle;
use tokio::time::Instant;
use url::Url;

use crate::fetch::DOCUMENT_LIMIT;
use crate::transfer::{HttpMethod, HttpRequest, SourceChain, Transport, TransportResponse};

const REQUEST_LIMIT: usize = 1024 * 1024;
const HEADER_LIMIT: usize = 16 * 1024;
const REDIRECT_LIMIT: usize = 10;

pub(super) struct Network {
    transport: Arc<dyn Transport>,
    sources: RwLock<Option<SourceChain>>,
}

impl Network {
    pub(super) fn new(transport: Arc<dyn Transport>, sources: Option<SourceChain>) -> Self {
        Self {
            transport,
            sources: RwLock::new(sources),
        }
    }

    pub(super) fn set_sources(&self, sources: SourceChain) {
        *self
            .sources
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sources);
    }

    pub(super) fn context(&self, timeout: Duration) -> NetworkContext {
        NetworkContext {
            transport: self.transport.clone(),
            sources: self
                .sources
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            handle: Handle::current(),
            deadline: Instant::now() + timeout,
        }
    }
}

pub(super) struct NetworkContext {
    transport: Arc<dyn Transport>,
    sources: Option<SourceChain>,
    handle: Handle,
    deadline: Instant,
}

fn unavailable(message: impl Into<String>) -> PluginError {
    PluginError::Unavailable(message.into())
}

fn http_url(text: &str) -> Result<Url, PluginError> {
    let url = Url::parse(text).map_err(|_| PluginError::PermissionDenied)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(PluginError::PermissionDenied);
    }
    Ok(url)
}

fn granted(manifest: &Manifest, url: &Url) -> bool {
    manifest.permissions.iter().any(|permission| {
        matches!(permission, Permission::Network { hosts }
            if hosts.iter().any(|host| Some(host.as_str()) == url.host_str()))
    })
}

fn safe_header(name: &str) -> bool {
    ["accept", "content-type", "user-agent"]
        .iter()
        .any(|safe| name.eq_ignore_ascii_case(safe))
}

fn validate_request(request: &FetchRequest) -> Result<(), PluginError> {
    if request
        .body
        .as_ref()
        .is_some_and(|body| body.len() > REQUEST_LIMIT)
    {
        return Err(PluginError::InvalidInput(
            "request body is larger than the limit".into(),
        ));
    }
    if request
        .headers
        .iter()
        .map(|(name, value)| name.len().saturating_add(value.len()))
        .try_fold(0_usize, |sum, size| sum.checked_add(size))
        .is_none_or(|size| size > HEADER_LIMIT)
    {
        return Err(PluginError::InvalidInput(
            "request headers are larger than the limit".into(),
        ));
    }
    for (name, value) in &request.headers {
        // Plugins cannot override routing or framing decided by the host.
        if [
            "host",
            "connection",
            "content-length",
            "transfer-encoding",
            "proxy-authorization",
            "proxy-connection",
        ]
        .iter()
        .any(|forbidden| name.eq_ignore_ascii_case(forbidden))
            || reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
            || reqwest::header::HeaderValue::from_str(value).is_err()
        {
            return Err(PluginError::InvalidInput("invalid request header".into()));
        }
    }
    Ok(())
}

impl NetworkContext {
    pub(super) fn fetch(
        &self,
        manifest: &Manifest,
        request: FetchRequest,
    ) -> Result<FetchResponse, PluginError> {
        let original = http_url(&request.url)?;
        if !granted(manifest, &original) {
            return Err(PluginError::PermissionDenied);
        }
        validate_request(&request)?;
        if Instant::now() >= self.deadline {
            return Err(unavailable("plugin network deadline expired"));
        }
        // Only called inside spawn_blocking. Dropping a pending transport
        // future at the shared deadline also releases a timed-out worker.
        self.handle.block_on(async {
            tokio::time::timeout_at(self.deadline, self.fetch_async(manifest, request))
                .await
                .map_err(|_| unavailable("plugin network deadline expired"))?
        })
    }

    async fn fetch_async(
        &self,
        manifest: &Manifest,
        request: FetchRequest,
    ) -> Result<FetchResponse, PluginError> {
        // POST has no mirror/retry, as in the current Modrinth client. GET
        // with credentials also stays on its original host.
        let credentialed = request.headers.iter().any(|(name, _)| !safe_header(name));
        let sources = self
            .sources
            .as_ref()
            .ok_or_else(|| unavailable("invalid mirror configuration"))?;
        let candidates = if request.method == FetchMethod::Get && !credentialed {
            sources.candidates(&request.url)
        } else {
            vec![request.url.clone()]
        };
        let mut candidates = candidates.into_iter().peekable();
        let mut last = unavailable("no source to fetch from");
        while let Some(candidate) = candidates.next() {
            let candidate = http_url(&candidate)?;
            match self
                .read_candidate(manifest, request.clone(), candidate)
                .await
            {
                Ok(answer)
                    if (200..300).contains(&answer.status) || candidates.peek().is_none() =>
                {
                    return Ok(answer);
                }
                Ok(answer) => last = unavailable(format!("HTTP {}", answer.status)),
                Err(PluginError::PermissionDenied) => return Err(PluginError::PermissionDenied),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    async fn read_candidate(
        &self,
        manifest: &Manifest,
        request: FetchRequest,
        candidate: Url,
    ) -> Result<FetchResponse, PluginError> {
        let mut current = candidate.clone();
        let mut request = HttpRequest {
            method: match request.method {
                FetchMethod::Get => HttpMethod::Get,
                FetchMethod::Post => HttpMethod::Post,
            },
            url: current.to_string(),
            headers: request.headers,
            body: request.body,
        };
        for hop in 0..=REDIRECT_LIMIT {
            let response = self
                .transport
                .send_no_redirect(request.clone())
                .await
                .map_err(|error| unavailable(error.to_string()))?;
            let status = response.status();
            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                let location = response
                    .header("location")
                    .ok_or_else(|| unavailable("redirect has no location"))?;
                let next = current
                    .join(location)
                    .map_err(|_| PluginError::PermissionDenied)?;
                let next = http_url(next.as_str())?;
                // A mapped mirror can redirect within its own origin. Any
                // other host still needs the plugin's grant.
                if (next.origin() != candidate.origin() && !granted(manifest, &next))
                    || (current.scheme() == "https" && next.scheme() != "https")
                {
                    return Err(PluginError::PermissionDenied);
                }
                if hop == REDIRECT_LIMIT {
                    return Err(unavailable("too many redirects"));
                }
                if current.origin() != next.origin() {
                    request.headers.retain(|(name, _)| safe_header(name));
                }
                if status == 303
                    || (matches!(status, 301 | 302) && request.method == HttpMethod::Post)
                {
                    request.method = HttpMethod::Get;
                    request.body = None;
                    request
                        .headers
                        .retain(|(name, _)| !name.eq_ignore_ascii_case("content-type"));
                }
                current = next;
                request.url = current.to_string();
                continue;
            }
            return read_response(response).await;
        }
        Err(unavailable("too many redirects"))
    }
}

async fn read_response(response: TransportResponse) -> Result<FetchResponse, PluginError> {
    if response
        .content_length()
        .is_some_and(|length| length > DOCUMENT_LIMIT as u64)
    {
        return Err(unavailable("response is larger than the limit"));
    }
    let status = response.status();
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|error| unavailable(error.to_string()))?;
        if bytes
            .len()
            .checked_add(chunk.len())
            .is_none_or(|length| length > DOCUMENT_LIMIT)
        {
            return Err(unavailable("response is larger than the limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(FetchResponse {
        status,
        body: bytes,
    })
}
