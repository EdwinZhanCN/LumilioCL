use futures_util::stream;
use futures_util::stream::Stream;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;

pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + Send + 'a>>;
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, TransportError>> + Send + 'static>>;

/// How a [`HttpRequest`] is sent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// One request with its own headers and body, for APIs that need more than a
/// plain GET or a JSON POST (sign-in: form bodies, bearer tokens, and answers
/// whose error status carries the explanation).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

pub trait Transport: Send + Sync + 'static {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a>;

    /// Sends exactly one HTTP hop, without following redirects. The plugin
    /// host checks each destination. Never fall back to `send` here.
    fn send_no_redirect<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::permanent(
                "this transport cannot send requests without redirects",
            ))
        })
    }

    /// Sends a request exactly as described and returns the answer whatever
    /// its status. Transports that cannot keep this default, which fails
    /// permanently.
    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        let _ = request;
        Box::pin(async {
            Err(TransportError::permanent(
                "this transport cannot send arbitrary requests",
            ))
        })
    }

    /// Sends a JSON body. Transports that cannot (local files, test doubles)
    /// keep this default, which fails permanently.
    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        let _ = (source, body);
        Box::pin(async {
            Err(TransportError::permanent(
                "POST is not supported by this transport",
            ))
        })
    }
}

pub struct TransportResponse {
    pub(super) status: u16,
    pub(super) content_length: Option<u64>,
    pub(super) body: ByteStream,
    /// Response headers, names in lower case.
    pub(super) headers: Vec<(String, String)>,
}

impl TransportResponse {
    #[must_use]
    pub fn new(status: u16, content_length: Option<u64>, body: ByteStream) -> Self {
        Self {
            status,
            content_length,
            body,
            headers: Vec::new(),
        }
    }

    /// The same answer with its headers (names are lower-cased).
    #[must_use]
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers
            .into_iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), value))
            .collect();
        self
    }

    /// A response header by name, if the transport reported it.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[must_use]
    pub fn from_bytes(status: u16, bytes: Vec<u8>) -> Self {
        let length = bytes.len() as u64;
        Self::new(
            status,
            Some(length),
            Box::pin(stream::once(async move { Ok(bytes) })),
        )
    }

    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    #[must_use]
    pub const fn content_length(&self) -> Option<u64> {
        self.content_length
    }

    pub fn into_body(self) -> ByteStream {
        self.body
    }
}

impl Debug for TransportResponse {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportResponse")
            .field("status", &self.status)
            .field("content_length", &self.content_length)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportError {
    pub(super) message: String,
    pub(super) retryable: bool,
}

impl TransportError {
    #[must_use]
    pub fn transient(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    #[must_use]
    pub fn permanent(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: false,
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }
}

impl Display for TransportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for TransportError {}
