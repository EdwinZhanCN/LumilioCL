use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub(super) attempts_per_source: usize,
    pub(super) initial_delay: Duration,
    pub(super) maximum_delay: Duration,
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
    pub(super) id: String,
    pub(super) sources: Vec<String>,
    pub(super) destination: PathBuf,
    pub(super) expected_size: Option<u64>,
    pub(super) expected_sha1: Option<String>,
    pub(super) cache_candidates: Vec<PathBuf>,
    pub(super) retry_policy: RetryPolicy,
    pub(super) reuse_existing: bool,
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
    pub(super) source: String,
    pub(super) attempt: usize,
    pub(super) reason: String,
    pub(super) retryable: bool,
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
