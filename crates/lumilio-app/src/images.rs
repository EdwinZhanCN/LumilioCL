//! The HTTP client GPUI uses to fetch images by address: project icons, gallery
//! pictures and pictures inside descriptions.
//!
//! GPUI ships no client of its own on desktop, so `img("https://…")` would show
//! nothing. This one runs the request on a small tokio runtime (reqwest needs
//! one) and hands the bytes back; it only supports plain GET, which is all an
//! image needs.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use gpui_kit::http_client::http::HeaderValue;
use gpui_kit::http_client::{AsyncBody, HttpClient, Request, Response, Result, Url, anyhow};
use lumilio_core::USER_AGENT;
use tokio::runtime::{Builder, Runtime};

pub struct ImageClient {
    runtime: Runtime,
    client: reqwest::Client,
    user_agent: HeaderValue,
}

impl ImageClient {
    pub fn new() -> Result<Arc<Self>, String> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("lumilio-images")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let client = {
            let _guard = runtime.enter();
            reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|error| error.to_string())?
        };
        Ok(Arc::new(Self {
            runtime,
            client,
            user_agent: HeaderValue::from_static(USER_AGENT),
        }))
    }
}

type Answer = Pin<Box<dyn Future<Output = Result<Response<AsyncBody>>> + Send + 'static>>;

impl HttpClient for ImageClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        Some(&self.user_agent)
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, request: Request<AsyncBody>) -> Answer {
        let client = self.client.clone();
        let url = request.uri().to_string();
        let work = self.runtime.spawn(async move {
            let response = client.get(&url).send().await?;
            let status = response.status().as_u16();
            let bytes = response.bytes().await?;
            Ok::<_, reqwest::Error>((status, bytes.to_vec()))
        });
        Box::pin(async move {
            let (status, bytes) = work
                .await
                .map_err(|error| anyhow!("image request stopped: {error}"))?
                .map_err(|error| anyhow!("image request failed: {error}"))?;
            Ok(Response::builder()
                .status(status)
                .body(AsyncBody::from(bytes))?)
        })
    }
}

#[cfg(test)]
mod tests;
