use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::pin::Pin;

use futures_util::stream;
use futures_util::stream::Stream;

pub type TransportFuture<'a> =
    Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + Send + 'a>>;
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Vec<u8>, TransportError>> + Send + 'static>>;

/// How a [`HttpRequest`] is sent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
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

    /// Asks for the bytes `start..end` of the content (`206`), to its end when
    /// `end` is `None`, unless it no longer matches `validator` (an `ETag` or
    /// `Last-Modified`). Any other answer, such as the whole content with
    /// `200`, makes the caller start over; transports that cannot ask keep
    /// this default, which does that.
    fn get_range<'a>(
        &'a self,
        source: &'a str,
        start: u64,
        end: Option<u64>,
        validator: Option<&'a str>,
    ) -> TransportFuture<'a> {
        let _ = (start, end, validator);
        self.get(source)
    }

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

/// A shared transport, so its owner can still see it once an engine holds
/// it (a plugin host's, a test's).
impl<T: Transport + ?Sized> Transport for std::sync::Arc<T> {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        (**self).get(source)
    }

    fn get_range<'a>(
        &'a self,
        source: &'a str,
        start: u64,
        end: Option<u64>,
        validator: Option<&'a str>,
    ) -> TransportFuture<'a> {
        (**self).get_range(source, start, end, validator)
    }

    fn send_no_redirect<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        (**self).send_no_redirect(request)
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        (**self).send(request)
    }

    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        (**self).post_json(source, body)
    }
}

pub struct TransportResponse {
    pub(super) status: u16,
    pub(super) content_length: Option<u64>,
    pub(super) body: ByteStream,
    /// Response headers, names in lower case.
    pub(super) headers: Vec<(String, String)>,
    /// Where the answer came from after redirects, if the transport knows.
    pub(super) url: Option<String>,
}

impl TransportResponse {
    #[must_use]
    pub fn new(status: u16, content_length: Option<u64>, body: ByteStream) -> Self {
        Self {
            status,
            content_length,
            body,
            headers: Vec::new(),
            url: None,
        }
    }

    /// The same answer, as served from `url` after redirects.
    #[must_use]
    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// Where the answer came from after redirects, if the transport knows.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
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
