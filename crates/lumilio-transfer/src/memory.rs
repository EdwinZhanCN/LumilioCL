//! Downloads read into memory, for small documents such as version lists and
//! API answers (plan download-engine T4). They take a connection of the
//! budget like any download and are retried the same way, but are no
//! transfer of their own: they send no events, so no task shows them.

use super::engine::TransferEngine;
use super::request::RetryPolicy;
use super::transport::Transport;
use futures_util::StreamExt;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Why one source gave no document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchFailure {
    reason: String,
    /// Whether the source answered (an error status, a body too large)
    /// rather than being out of reach.
    answered: bool,
    retryable: bool,
}

impl FetchFailure {
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    #[must_use]
    pub const fn answered(&self) -> bool {
        self.answered
    }
}

impl Display for FetchFailure {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl Error for FetchFailure {}

impl<T> TransferEngine<T>
where
    T: Transport,
{
    /// Reads `source` whole, if it answers with a 2xx status and no more than
    /// `limit` bytes. A busy or unreachable source is asked again as `retry`
    /// allows; the connection is let go of between attempts.
    pub async fn fetch(
        &self,
        source: &str,
        limit: usize,
        retry: &RetryPolicy,
    ) -> Result<Vec<u8>, FetchFailure> {
        let mut attempt = 1;
        loop {
            let result = match self.budget.acquire().await {
                Ok(connection) => {
                    let result = self.read(source, limit).await;
                    drop(connection);
                    result
                }
                Err(error) => {
                    return Err(FetchFailure {
                        reason: format!("transfer scheduler is closed: {error}"),
                        answered: false,
                        retryable: false,
                    });
                }
            };
            match result {
                Err(failure) if failure.retryable && attempt < retry.attempts_per_source() => {
                    attempt += 1;
                    tokio::time::sleep(retry.delay_before_attempt(attempt)).await;
                }
                result => return result,
            }
        }
    }

    async fn read(&self, source: &str, limit: usize) -> Result<Vec<u8>, FetchFailure> {
        let response = self
            .transport
            .get(source)
            .await
            .map_err(|error| FetchFailure {
                retryable: error.is_retryable(),
                reason: error.message,
                answered: false,
            })?;
        let status = response.status();
        if !(200..300).contains(&status) {
            return Err(FetchFailure {
                reason: format!("HTTP {status}"),
                answered: true,
                retryable: status == 408 || status == 429 || status >= 500,
            });
        }
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(chunk) = body.next().await {
            // A body cut off on the way is the connection's failure.
            let chunk = chunk.map_err(|error| FetchFailure {
                retryable: error.is_retryable(),
                reason: error.message,
                answered: false,
            })?;
            if bytes.len() + chunk.len() > limit {
                return Err(FetchFailure {
                    reason: "document is larger than the limit".to_owned(),
                    answered: true,
                    retryable: false,
                });
            }
            self.budget.count(chunk.len());
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
