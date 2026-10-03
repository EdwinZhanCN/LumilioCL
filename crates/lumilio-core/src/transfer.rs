use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Debug, Display, Formatter};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::stream::{self, Stream, StreamExt};
use reqwest::Client;
use sha1::{Digest, Sha1};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Semaphore, broadcast};
use tokio::task::JoinSet;
use url::Url;

use crate::activity::CancellationToken;

/// Identifies the launcher to servers; some APIs (Modrinth) reject anonymous
/// clients.
pub const USER_AGENT: &str = concat!("LumilioCL/", env!("CARGO_PKG_VERSION"));

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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

pub trait SourceProvider: Send + Sync + 'static {
    fn candidates(&self, original: &str) -> Vec<String>;

    fn maximum_concurrency(&self) -> usize {
        16
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OfficialSource;

impl SourceProvider for OfficialSource {
    fn candidates(&self, original: &str) -> Vec<String> {
        vec![original.to_owned()]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrefixMirror {
    official_prefix: String,
    mirror_prefix: String,
    concurrency: usize,
}

impl PrefixMirror {
    pub fn new(
        official_prefix: impl Into<String>,
        mirror_prefix: impl Into<String>,
        concurrency: usize,
    ) -> Result<Self, TransferError> {
        let official_prefix = official_prefix.into();
        let mirror_prefix = mirror_prefix.into();
        if official_prefix.is_empty() || mirror_prefix.is_empty() {
            return Err(TransferError::InvalidRequest(
                "mirror prefixes must not be empty".to_owned(),
            ));
        }
        if concurrency == 0 {
            return Err(TransferError::InvalidRequest(
                "mirror concurrency must be positive".to_owned(),
            ));
        }
        Ok(Self {
            official_prefix,
            mirror_prefix,
            concurrency,
        })
    }
}

impl SourceProvider for PrefixMirror {
    fn candidates(&self, original: &str) -> Vec<String> {
        original
            .strip_prefix(&self.official_prefix)
            .map(|suffix| format!("{}{}", self.mirror_prefix, suffix))
            .into_iter()
            .collect()
    }

    fn maximum_concurrency(&self) -> usize {
        self.concurrency
    }
}

#[derive(Clone)]
pub struct SourceChain {
    providers: Vec<Arc<dyn SourceProvider>>,
}

impl SourceChain {
    pub fn new(
        providers: impl IntoIterator<Item = Arc<dyn SourceProvider>>,
    ) -> Result<Self, TransferError> {
        let providers = providers.into_iter().collect::<Vec<_>>();
        if providers.is_empty() {
            return Err(TransferError::InvalidRequest(
                "at least one source provider is required".to_owned(),
            ));
        }
        Ok(Self { providers })
    }

    #[must_use]
    pub fn candidates(&self, original: &str) -> Vec<String> {
        let mut seen = BTreeSet::new();
        self.providers
            .iter()
            .flat_map(|provider| provider.candidates(original))
            .filter(|source| !source.trim().is_empty())
            .filter(|source| seen.insert(source.clone()))
            .collect()
    }

    #[must_use]
    pub fn preferred_concurrency(&self) -> usize {
        self.providers[0].maximum_concurrency().max(1)
    }
}

pub struct TransportResponse {
    status: u16,
    content_length: Option<u64>,
    body: ByteStream,
}

impl TransportResponse {
    #[must_use]
    pub fn new(status: u16, content_length: Option<u64>, body: ByteStream) -> Self {
        Self {
            status,
            content_length,
            body,
        }
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
    message: String,
    retryable: bool,
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

#[derive(Clone, Debug)]
pub struct HttpTransport {
    client: Client,
}

impl HttpTransport {
    pub fn new() -> Result<Self, TransportError> {
        let client = Client::builder()
            .user_agent(USER_AGENT)
            .build()
            .map_err(|error| TransportError::permanent(error.to_string()))?;
        Ok(Self { client })
    }

    #[must_use]
    pub const fn from_client(client: Client) -> Self {
        Self { client }
    }

    #[must_use]
    pub const fn client(&self) -> &Client {
        &self.client
    }
}

impl HttpTransport {
    fn respond(
        result: Result<reqwest::Response, reqwest::Error>,
    ) -> Result<TransportResponse, TransportError> {
        let response = result.map_err(|error| {
            if error.is_timeout() || error.is_connect() {
                TransportError::transient(error.to_string())
            } else {
                TransportError::permanent(error.to_string())
            }
        })?;
        let status = response.status().as_u16();
        let content_length = response.content_length();
        let body = response.bytes_stream().map(|chunk| {
            chunk
                .map(|bytes| bytes.to_vec())
                .map_err(|error| TransportError::transient(error.to_string()))
        });
        Ok(TransportResponse::new(
            status,
            content_length,
            Box::pin(body),
        ))
    }
}

impl Transport for HttpTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move { Self::respond(self.client.get(source).send().await) })
    }

    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        Box::pin(async move {
            Self::respond(
                self.client
                    .post(source)
                    .header("content-type", "application/json")
                    .body(body)
                    .send()
                    .await,
            )
        })
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            let mut builder = match request.method {
                HttpMethod::Get => self.client.get(&request.url),
                HttpMethod::Post => self.client.post(&request.url),
            };
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            Self::respond(builder.send().await)
        })
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FileTransport;

impl Transport for FileTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            let url =
                Url::parse(source).map_err(|error| TransportError::permanent(error.to_string()))?;
            if url.scheme() != "file" {
                return Err(TransportError::permanent(format!(
                    "unsupported local source scheme: {}",
                    url.scheme()
                )));
            }
            let path = url
                .to_file_path()
                .map_err(|()| TransportError::permanent("invalid file URL"))?;
            let file = File::open(&path)
                .await
                .map_err(|error| TransportError::permanent(error.to_string()))?;
            let content_length = file
                .metadata()
                .await
                .map_err(|error| TransportError::permanent(error.to_string()))?
                .len();
            let body = stream::unfold(Some(file), |state| async move {
                let mut file = state?;
                let mut buffer = vec![0_u8; 64 * 1024];
                match file.read(&mut buffer).await {
                    Ok(0) => None,
                    Ok(read) => {
                        buffer.truncate(read);
                        Some((Ok(buffer), Some(file)))
                    }
                    Err(error) => Some((Err(TransportError::permanent(error.to_string())), None)),
                }
            });
            Ok(TransportResponse::new(
                200,
                Some(content_length),
                Box::pin(body),
            ))
        })
    }
}

#[derive(Clone, Debug)]
pub struct DefaultTransport {
    http: HttpTransport,
    file: FileTransport,
}

impl DefaultTransport {
    pub fn new() -> Result<Self, TransportError> {
        Ok(Self {
            http: HttpTransport::new()?,
            file: FileTransport,
        })
    }
}

impl Transport for DefaultTransport {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        if Url::parse(source).is_ok_and(|url| url.scheme() == "file") {
            self.file.get(source)
        } else {
            self.http.get(source)
        }
    }

    fn post_json<'a>(&'a self, source: &'a str, body: Vec<u8>) -> TransportFuture<'a> {
        self.http.post_json(source, body)
    }

    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.http.send(request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    attempts_per_source: usize,
    initial_delay: Duration,
    maximum_delay: Duration,
}

impl RetryPolicy {
    pub fn fixed(
        attempts_per_source: usize,
        initial_delay: Duration,
        maximum_delay: Duration,
    ) -> Result<Self, TransferError> {
        if attempts_per_source == 0 {
            return Err(TransferError::InvalidRequest(
                "retry attempts must be positive".to_owned(),
            ));
        }
        if initial_delay > maximum_delay {
            return Err(TransferError::InvalidRequest(
                "initial retry delay exceeds maximum delay".to_owned(),
            ));
        }
        Ok(Self {
            attempts_per_source,
            initial_delay,
            maximum_delay,
        })
    }

    pub fn immediate(attempts_per_source: usize) -> Result<Self, TransferError> {
        Self::fixed(attempts_per_source, Duration::ZERO, Duration::ZERO)
    }

    #[must_use]
    pub const fn attempts_per_source(&self) -> usize {
        self.attempts_per_source
    }

    #[must_use]
    pub fn delay_before_attempt(&self, attempt: usize) -> Duration {
        if attempt <= 1 || self.initial_delay.is_zero() {
            return Duration::ZERO;
        }
        let exponent = u32::try_from(attempt.saturating_sub(2)).unwrap_or(u32::MAX);
        self.initial_delay
            .checked_mul(2_u32.saturating_pow(exponent))
            .unwrap_or(self.maximum_delay)
            .min(self.maximum_delay)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts_per_source: 5,
            initial_delay: Duration::from_millis(200),
            maximum_delay: Duration::from_secs(3),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransferRequest {
    id: String,
    sources: Vec<String>,
    destination: PathBuf,
    expected_size: Option<u64>,
    expected_sha1: Option<String>,
    cache_candidates: Vec<PathBuf>,
    retry_policy: RetryPolicy,
    reuse_existing: bool,
}

impl TransferRequest {
    pub fn new<I, S>(
        id: impl Into<String>,
        sources: I,
        destination: impl Into<PathBuf>,
    ) -> Result<Self, TransferError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(TransferError::InvalidRequest(
                "transfer id must not be empty".to_owned(),
            ));
        }
        let mut seen = BTreeSet::new();
        let sources = sources
            .into_iter()
            .map(Into::into)
            .filter(|source| !source.trim().is_empty())
            .filter(|source| seen.insert(source.clone()))
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return Err(TransferError::InvalidRequest(
                "at least one non-empty source is required".to_owned(),
            ));
        }
        let destination = destination.into();
        if destination.as_os_str().is_empty() {
            return Err(TransferError::InvalidRequest(
                "destination must not be empty".to_owned(),
            ));
        }
        Ok(Self {
            id,
            sources,
            destination,
            expected_size: None,
            expected_sha1: None,
            cache_candidates: Vec::new(),
            retry_policy: RetryPolicy::default(),
            reuse_existing: true,
        })
    }

    #[must_use]
    pub fn expect_size(mut self, expected_size: u64) -> Self {
        self.expected_size = Some(expected_size);
        self
    }

    pub fn expect_sha1(mut self, digest: impl Into<String>) -> Result<Self, TransferError> {
        let digest = digest.into();
        if digest.len() != 40 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(TransferError::InvalidRequest(format!(
                "invalid SHA-1 digest: {digest}"
            )));
        }
        self.expected_sha1 = Some(digest.to_ascii_lowercase());
        Ok(self)
    }

    #[must_use]
    pub fn with_cache_candidate(mut self, candidate: impl Into<PathBuf>) -> Self {
        let candidate = candidate.into();
        if !self.cache_candidates.contains(&candidate) {
            self.cache_candidates.push(candidate);
        }
        self
    }

    #[must_use]
    pub fn with_retry_policy(mut self, retry_policy: RetryPolicy) -> Self {
        self.retry_policy = retry_policy;
        self
    }

    #[must_use]
    pub fn force_refresh(mut self) -> Self {
        self.reuse_existing = false;
        self
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    #[must_use]
    pub const fn expected_size(&self) -> Option<u64> {
        self.expected_size
    }

    #[must_use]
    pub fn expected_sha1(&self) -> Option<&str> {
        self.expected_sha1.as_deref()
    }

    #[must_use]
    pub fn cache_candidates(&self) -> &[PathBuf] {
        &self.cache_candidates
    }

    #[must_use]
    pub const fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    #[must_use]
    pub const fn reuses_existing(&self) -> bool {
        self.reuse_existing
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferOutcome {
    ReusedExisting,
    ReusedCache {
        source: PathBuf,
    },
    Downloaded {
        source: String,
        attempts: usize,
        bytes: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFailure {
    source: String,
    attempt: usize,
    reason: String,
    retryable: bool,
}

impl SourceFailure {
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn attempt(&self) -> usize {
        self.attempt
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        self.retryable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferError {
    InvalidRequest(String),
    Cancelled,
    FileSystem {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    AllSourcesFailed {
        id: String,
        failures: Vec<SourceFailure>,
    },
}

impl Display for TransferError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => {
                write!(formatter, "invalid transfer request: {message}")
            }
            Self::Cancelled => formatter.write_str("transfer was cancelled"),
            Self::FileSystem {
                operation,
                path,
                message,
            } => write!(
                formatter,
                "failed to {operation} {}: {message}",
                path.display()
            ),
            Self::AllSourcesFailed { id, failures } => {
                write!(formatter, "all sources failed for {id}")?;
                if let Some(last) = failures.last() {
                    write!(formatter, ": {} ({})", last.source, last.reason)?;
                }
                Ok(())
            }
        }
    }
}

impl Error for TransferError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransferEvent {
    Started {
        id: String,
        destination: PathBuf,
    },
    Progress {
        id: String,
        completed: u64,
        total: Option<u64>,
    },
    Retrying {
        id: String,
        source: String,
        next_attempt: usize,
    },
    Completed {
        id: String,
        outcome: TransferOutcome,
    },
    Failed {
        id: String,
        message: String,
    },
}

pub struct TransferEngine<T> {
    transport: Arc<T>,
    concurrency: Arc<Semaphore>,
    events: broadcast::Sender<TransferEvent>,
}

impl<T> Clone for TransferEngine<T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport.clone(),
            concurrency: self.concurrency.clone(),
            events: self.events.clone(),
        }
    }
}

impl<T> TransferEngine<T>
where
    T: Transport,
{
    pub fn new(transport: T, concurrency: usize) -> Result<Self, TransferError> {
        if concurrency == 0 {
            return Err(TransferError::InvalidRequest(
                "transfer concurrency must be positive".to_owned(),
            ));
        }
        let (events, _) = broadcast::channel(512);
        Ok(Self {
            transport: Arc::new(transport),
            concurrency: Arc::new(Semaphore::new(concurrency)),
            events,
        })
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<TransferEvent> {
        self.events.subscribe()
    }

    pub async fn transfer(
        &self,
        request: TransferRequest,
        cancellation: CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        let id = request.id.clone();
        let _ = self.events.send(TransferEvent::Started {
            id: id.clone(),
            destination: request.destination.clone(),
        });

        let paths = [request.destination.clone()];
        let _resource = tokio::select! {
            biased;
            () = cancellation.cancelled() => return self.finish_error(&id, TransferError::Cancelled),
            acquired = crate::resource_lock::acquire(&paths) => match acquired {
                Ok(guard) => guard,
                Err(error) => return self.finish_error(&id, TransferError::FileSystem {
                    operation: "lock download destination", path: request.destination.clone(), message: error.to_string(),
                }),
            },
        };

        let permit = tokio::select! {
            () = cancellation.cancelled() => return self.finish_error(&id, TransferError::Cancelled),
            permit = self.concurrency.acquire() => permit.map_err(|error| {
                TransferError::InvalidRequest(format!("transfer scheduler is closed: {error}"))
            })?,
        };
        let result = self.transfer_with_permit(&request, &cancellation).await;
        drop(permit);

        match result {
            Ok(outcome) => {
                let _ = self.events.send(TransferEvent::Completed {
                    id,
                    outcome: outcome.clone(),
                });
                Ok(outcome)
            }
            Err(error) => self.finish_error(&id, error),
        }
    }

    fn finish_error<R>(&self, id: &str, error: TransferError) -> Result<R, TransferError> {
        let _ = self.events.send(TransferEvent::Failed {
            id: id.to_owned(),
            message: error.to_string(),
        });
        Err(error)
    }

    async fn transfer_with_permit(
        &self,
        request: &TransferRequest,
        cancellation: &CancellationToken,
    ) -> Result<TransferOutcome, TransferError> {
        if cancellation.is_cancelled() {
            return Err(TransferError::Cancelled);
        }
        if request.reuse_existing
            && verify_file(
                &request.destination,
                request.expected_size,
                request.expected_sha1.as_deref(),
            )
            .await?
        {
            return Ok(TransferOutcome::ReusedExisting);
        }

        for candidate in &request.cache_candidates {
            if cancellation.is_cancelled() {
                return Err(TransferError::Cancelled);
            }
            let candidate_is_valid = verify_file(
                candidate,
                request.expected_size,
                request.expected_sha1.as_deref(),
            )
            .await
            .unwrap_or(false);
            if candidate_is_valid && copy_verified_candidate(candidate, request).await.is_ok() {
                return Ok(TransferOutcome::ReusedCache {
                    source: candidate.clone(),
                });
            }
        }

        let mut failures = Vec::new();
        let mut total_attempts = 0;
        for source in &request.sources {
            for attempt in 1..=request.retry_policy.attempts_per_source {
                if cancellation.is_cancelled() {
                    return Err(TransferError::Cancelled);
                }
                total_attempts += 1;
                match self.download_once(request, source, cancellation).await {
                    Ok(bytes) => {
                        return Ok(TransferOutcome::Downloaded {
                            source: source.clone(),
                            attempts: total_attempts,
                            bytes,
                        });
                    }
                    Err(AttemptFailure::Cancelled) => return Err(TransferError::Cancelled),
                    Err(AttemptFailure::Fatal(error)) => return Err(error),
                    Err(AttemptFailure::Source { reason, retryable }) => {
                        failures.push(SourceFailure {
                            source: source.clone(),
                            attempt,
                            reason,
                            retryable,
                        });
                        if !retryable || attempt == request.retry_policy.attempts_per_source {
                            break;
                        }
                        let next_attempt = attempt + 1;
                        let _ = self.events.send(TransferEvent::Retrying {
                            id: request.id.clone(),
                            source: source.clone(),
                            next_attempt,
                        });
                        let delay = request.retry_policy.delay_before_attempt(next_attempt);
                        if !delay.is_zero() {
                            tokio::select! {
                                () = cancellation.cancelled() => return Err(TransferError::Cancelled),
                                () = tokio::time::sleep(delay) => {}
                            }
                        }
                    }
                }
            }
        }

        Err(TransferError::AllSourcesFailed {
            id: request.id.clone(),
            failures,
        })
    }

    async fn download_once(
        &self,
        request: &TransferRequest,
        source: &str,
        cancellation: &CancellationToken,
    ) -> Result<u64, AttemptFailure> {
        let response = tokio::select! {
            () = cancellation.cancelled() => return Err(AttemptFailure::Cancelled),
            response = self.transport.get(source) => response.map_err(|error| AttemptFailure::Source {
                reason: error.message,
                retryable: error.retryable,
            })?,
        };
        let status = response.status();
        if !(200..300).contains(&status) {
            let retryable = status == 408 || status == 429 || status >= 500;
            return Err(AttemptFailure::Source {
                reason: format!("HTTP status {status}"),
                retryable,
            });
        }

        let response_length = response.content_length();
        let (temporary_path, mut temporary_file) = create_temporary_sibling(&request.destination)
            .await
            .map_err(AttemptFailure::Fatal)?;
        let result = async {
            let mut body = response.into_body();
            let mut bytes_written = 0_u64;
            let mut digest = Sha1::new();
            loop {
                let chunk = tokio::select! {
                    () = cancellation.cancelled() => {
                        return Err(AttemptFailure::Cancelled);
                    }
                    chunk = body.next() => chunk,
                };
                let Some(chunk) = chunk else {
                    break;
                };
                let chunk = chunk.map_err(|error| AttemptFailure::Source {
                    reason: error.message,
                    retryable: error.retryable,
                })?;
                temporary_file.write_all(&chunk).await.map_err(|error| {
                    AttemptFailure::Fatal(file_system_error(
                        "write temporary file",
                        &temporary_path,
                        error,
                    ))
                })?;
                digest.update(&chunk);
                bytes_written = bytes_written.saturating_add(chunk.len() as u64);
                let _ = self.events.send(TransferEvent::Progress {
                    id: request.id.clone(),
                    completed: bytes_written,
                    total: request
                        .expected_size
                        .or(response_length)
                        .filter(|total| bytes_written <= *total),
                });
            }

            if let Some(response_length) = response_length
                && bytes_written != response_length
            {
                return Err(AttemptFailure::source(format!(
                    "response length was {bytes_written}, expected {response_length}"
                )));
            }
            if let Some(expected_size) = request.expected_size
                && bytes_written != expected_size
            {
                return Err(AttemptFailure::source(format!(
                    "file size was {bytes_written}, expected {expected_size}"
                )));
            }
            if let Some(expected) = &request.expected_sha1 {
                let actual = encode_lower_hex(&digest.finalize());
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(AttemptFailure::source(format!(
                        "SHA-1 was {actual}, expected {expected}"
                    )));
                }
            }

            temporary_file.flush().await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error(
                    "flush temporary file",
                    &temporary_path,
                    error,
                ))
            })?;
            temporary_file.sync_all().await.map_err(|error| {
                AttemptFailure::Fatal(file_system_error(
                    "sync temporary file",
                    &temporary_path,
                    error,
                ))
            })?;
            Ok(bytes_written)
        }
        .await;

        drop(temporary_file);
        match result {
            Ok(bytes_written) => {
                if let Err(error) = publish_temporary(&temporary_path, &request.destination).await {
                    let _ = fs::remove_file(&temporary_path).await;
                    Err(AttemptFailure::Fatal(error))
                } else {
                    Ok(bytes_written)
                }
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path).await;
                Err(error)
            }
        }
    }

    pub async fn transfer_batch(
        &self,
        requests: Vec<TransferRequest>,
        cancellation: CancellationToken,
    ) -> TransferBatchReport {
        let mut duplicate_ids = BTreeSet::new();
        let mut seen_ids = BTreeSet::new();
        for request in &requests {
            if !seen_ids.insert(request.id.clone()) {
                duplicate_ids.insert(request.id.clone());
            }
        }

        let mut results = BTreeMap::new();
        let mut join_set = JoinSet::new();
        for request in requests {
            if duplicate_ids.contains(&request.id) {
                results.insert(
                    request.id.clone(),
                    Err(TransferError::InvalidRequest(format!(
                        "duplicate transfer id: {}",
                        request.id
                    ))),
                );
                continue;
            }
            let id = request.id.clone();
            let engine = self.clone();
            let cancellation = cancellation.clone();
            join_set.spawn(async move { (id, engine.transfer(request, cancellation).await) });
        }

        while let Some(joined) = join_set.join_next().await {
            match joined {
                Ok((id, result)) => {
                    results.insert(id, result);
                }
                Err(error) => {
                    results.insert(
                        format!("worker-{}", results.len()),
                        Err(TransferError::InvalidRequest(format!(
                            "transfer worker failed: {error}"
                        ))),
                    );
                }
            }
        }
        TransferBatchReport { results }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TransferBatchReport {
    results: BTreeMap<String, Result<TransferOutcome, TransferError>>,
}

impl TransferBatchReport {
    #[must_use]
    pub fn results(&self) -> &BTreeMap<String, Result<TransferOutcome, TransferError>> {
        &self.results
    }

    #[must_use]
    pub fn result(&self, id: &str) -> Option<&Result<TransferOutcome, TransferError>> {
        self.results.get(id)
    }

    #[must_use]
    pub fn succeeded(&self) -> usize {
        self.results
            .values()
            .filter(|result| result.is_ok())
            .count()
    }

    #[must_use]
    pub fn failed(&self) -> usize {
        self.results
            .values()
            .filter(|result| result.is_err())
            .count()
    }
}

#[derive(Debug)]
enum AttemptFailure {
    Source { reason: String, retryable: bool },
    Cancelled,
    Fatal(TransferError),
}

impl AttemptFailure {
    fn source(reason: String) -> Self {
        Self::Source {
            reason,
            retryable: true,
        }
    }
}

async fn verify_file(
    path: &Path,
    expected_size: Option<u64>,
    expected_sha1: Option<&str>,
) -> Result<bool, TransferError> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(file_system_error("inspect", path, error)),
    };
    if !metadata.is_file() {
        return Ok(false);
    }
    if expected_size.is_some_and(|expected| metadata.len() != expected) {
        return Ok(false);
    }
    let Some(expected_sha1) = expected_sha1 else {
        return Ok(true);
    };

    let mut file = File::open(path)
        .await
        .map_err(|error| file_system_error("open", path, error))?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| file_system_error("read", path, error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(encode_lower_hex(&hasher.finalize()).eq_ignore_ascii_case(expected_sha1))
}

fn encode_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

async fn copy_verified_candidate(
    candidate: &Path,
    request: &TransferRequest,
) -> Result<(), TransferError> {
    let (temporary_path, temporary_file) = create_temporary_sibling(&request.destination).await?;
    drop(temporary_file);
    let result = async {
        fs::copy(candidate, &temporary_path)
            .await
            .map_err(|error| file_system_error("copy cache candidate", candidate, error))?;
        if !verify_file(
            &temporary_path,
            request.expected_size,
            request.expected_sha1.as_deref(),
        )
        .await?
        {
            return Err(TransferError::InvalidRequest(format!(
                "cache copy failed verification: {}",
                candidate.display()
            )));
        }
        publish_temporary(&temporary_path, &request.destination).await
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path).await;
    }
    result
}

async fn create_temporary_sibling(destination: &Path) -> Result<(PathBuf, File), TransferError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .await
        .map_err(|error| file_system_error("create destination directory", parent, error))?;
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");

    for _ in 0..128 {
        let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{file_name}.lumilio.{}.{}.part",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(file_system_error("create temporary file", &path, error)),
        }
    }
    Err(TransferError::FileSystem {
        operation: "create unique temporary file",
        path: destination.to_owned(),
        message: "temporary name space exhausted".to_owned(),
    })
}

async fn publish_temporary(temporary: &Path, destination: &Path) -> Result<(), TransferError> {
    fs::rename(temporary, destination)
        .await
        .map_err(|error| file_system_error("publish verified file", destination, error))
}

fn file_system_error(operation: &'static str, path: &Path, error: std::io::Error) -> TransferError {
    TransferError::FileSystem {
        operation,
        path: path.to_owned(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RetryPolicy;

    #[test]
    fn retry_delay_is_exponential_and_capped() {
        let policy =
            RetryPolicy::fixed(6, Duration::from_millis(100), Duration::from_millis(350)).unwrap();

        assert_eq!(policy.delay_before_attempt(1), Duration::ZERO);
        assert_eq!(policy.delay_before_attempt(2), Duration::from_millis(100));
        assert_eq!(policy.delay_before_attempt(3), Duration::from_millis(200));
        assert_eq!(policy.delay_before_attempt(4), Duration::from_millis(350));
    }
}

#[cfg(test)]
mod user_agent_tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[tokio::test]
    async fn http_requests_identify_the_launcher() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8_lossy(&buffer[..read]).into_owned()
        });
        let transport = HttpTransport::new().unwrap();
        let response = transport.get(&format!("http://{address}/")).await.unwrap();
        assert_eq!(response.status(), 200);
        let head = server.join().unwrap().to_ascii_lowercase();
        assert!(
            head.contains(&format!("user-agent: {}", USER_AGENT.to_ascii_lowercase())),
            "request head was: {head}"
        );
        assert!(USER_AGENT.starts_with("LumilioCL/"));
    }

    #[tokio::test]
    async fn post_json_sends_the_body_with_a_json_content_type() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            // Headers and body may arrive in separate reads; read the lot.
            let mut received = Vec::new();
            let mut buffer = [0_u8; 4096];
            while !received.ends_with(b"}") {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "connection closed early");
                received.extend_from_slice(&buffer[..read]);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        let transport = HttpTransport::new().unwrap();
        let response = transport
            .post_json(&format!("http://{address}/x"), br#"{"a":1}"#.to_vec())
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let head = server.join().unwrap().to_ascii_lowercase();
        assert!(head.starts_with("post /x "), "{head}");
        assert!(head.contains("content-type: application/json"), "{head}");
        assert!(head.ends_with(r#"{"a":1}"#), "{head}");
    }

    #[tokio::test]
    async fn send_carries_headers_and_body_and_returns_error_statuses_with_their_text() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            let mut buffer = [0_u8; 4096];
            while !received.ends_with(b"a=1&b=2") {
                let read = stream.read(&mut buffer).unwrap();
                assert!(read > 0, "connection closed early");
                received.extend_from_slice(&buffer[..read]);
            }
            let body = br#"{"error":"authorization_pending"}"#;
            let head = format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(head.as_bytes()).unwrap();
            stream.write_all(body).unwrap();
            String::from_utf8_lossy(&received).into_owned()
        });
        let transport = HttpTransport::new().unwrap();
        let response = transport
            .send(HttpRequest {
                method: HttpMethod::Post,
                url: format!("http://{address}/token"),
                headers: vec![
                    (
                        "content-type".into(),
                        "application/x-www-form-urlencoded".into(),
                    ),
                    ("authorization".into(), "Bearer abc".into()),
                ],
                body: Some(b"a=1&b=2".to_vec()),
            })
            .await
            .unwrap();
        // A refusal is an answer, not a transport error: its text explains it.
        assert_eq!(response.status(), 400);
        let mut body = response.into_body();
        let mut text = Vec::new();
        while let Some(chunk) = body.next().await {
            text.extend(chunk.unwrap());
        }
        assert!(String::from_utf8_lossy(&text).contains("authorization_pending"));
        let head = server.join().unwrap().to_ascii_lowercase();
        assert!(head.starts_with("post /token "), "{head}");
        assert!(head.contains("authorization: bearer abc"), "{head}");
        assert!(head.contains("application/x-www-form-urlencoded"), "{head}");
    }

    #[tokio::test]
    async fn transports_without_send_support_fail_permanently() {
        let error = FileTransport
            .send(HttpRequest {
                method: HttpMethod::Get,
                url: "file:///x".into(),
                headers: Vec::new(),
                body: None,
            })
            .await
            .unwrap_err();
        assert!(!error.is_retryable());
    }

    #[tokio::test]
    async fn transports_without_post_support_fail_permanently() {
        let error = FileTransport
            .post_json("file:///x", Vec::new())
            .await
            .unwrap_err();
        assert!(!error.message().is_empty());
    }
}
