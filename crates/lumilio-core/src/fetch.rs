//! Small-document retrieval with ordered source fallback.
//!
//! Metadata documents (version catalogs, API answers) are small and are read
//! whole, through the transfer engine's memory target: each takes one of the
//! shared connections and is asked again when its source is busy, but no
//! task shows it (plan download-engine T4).

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::LazyLock;
use std::time::Duration;

use futures_util::StreamExt;

use crate::transfer::{RetryPolicy, TransferEngine, Transport};

#[cfg(test)]
mod decoded_tests;

/// Upper bound for a document read whole, so a misbehaving source cannot
/// exhaust memory.
pub const DOCUMENT_LIMIT: usize = 32 * 1024 * 1024;

/// A document source that is busy or out of reach is asked once more before
/// the next source; a long wait would only delay that fallback.
static DOCUMENT_RETRY: LazyLock<RetryPolicy> = LazyLock::new(|| {
    RetryPolicy::fixed(2, Duration::from_millis(250), Duration::from_millis(250))
        .expect("two attempts with a fixed delay is a valid policy")
});

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchError {
    /// No source was supplied.
    NoSources,
    /// Every source failed; one `(source, reason)` pair per attempt.
    /// `answered` says whether any source answered (an error status, or a
    /// body that would not do) rather than being out of reach.
    Exhausted {
        attempts: Vec<(String, String)>,
        answered: bool,
    },
}

impl FetchError {
    /// Whether some source answered: what was asked for is not there, as
    /// opposed to the network or the host being down.
    #[must_use]
    pub const fn answered(&self) -> bool {
        matches!(self, Self::Exhausted { answered: true, .. })
    }
}

impl Display for FetchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSources => f.write_str("no source to fetch from"),
            Self::Exhausted { attempts, .. } => {
                f.write_str("every source failed")?;
                for (source, reason) in attempts {
                    write!(f, "; {source}: {reason}")?;
                }
                Ok(())
            }
        }
    }
}

impl Error for FetchError {}

/// Reads the first source that answers with a 2xx status and a body within
/// [`DOCUMENT_LIMIT`].
pub async fn fetch_document<T, S>(
    engine: &TransferEngine<T>,
    sources: &[S],
) -> Result<Vec<u8>, FetchError>
where
    T: Transport,
    S: AsRef<str>,
{
    fetch_decoded(engine, sources, Ok::<_, std::convert::Infallible>).await
}

/// Decode each candidate before accepting it. Unusable metadata is a source
/// failure even when the response has a 2xx status.
pub async fn fetch_decoded<T, S, V, E>(
    engine: &TransferEngine<T>,
    sources: &[S],
    mut decode: impl FnMut(Vec<u8>) -> Result<V, E>,
) -> Result<V, FetchError>
where
    T: Transport,
    S: AsRef<str>,
    E: Display,
{
    if sources.is_empty() {
        return Err(FetchError::NoSources);
    }
    let (mut attempts, mut answered) = (Vec::new(), false);
    for source in sources {
        let source = source.as_ref();
        match engine.fetch(source, DOCUMENT_LIMIT, &DOCUMENT_RETRY).await {
            Ok(bytes) => match decode(bytes) {
                Ok(value) => return Ok(value),
                Err(error) => {
                    answered = true;
                    attempts.push((source.to_owned(), error.to_string()));
                }
            },
            Err(failure) => {
                answered |= failure.answered();
                attempts.push((source.to_owned(), failure.reason().to_owned()));
            }
        }
    }
    Err(FetchError::Exhausted { attempts, answered })
}

/// Sends `body` as JSON to one address and reads the answer whole, with the
/// same status and size rules as [`fetch_document`]. No fallback: a write to
/// an API is not retried against other sources.
pub async fn post_document<T: Transport + ?Sized>(
    transport: &T,
    source: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>, FetchError> {
    let response = transport
        .post_json(source, body)
        .await
        .map_err(|error| Failure::unreached(error.to_string()).into_error(source))?;
    read_response(response)
        .await
        .map_err(|failure| failure.into_error(source))
}

/// Why one source gave nothing, and whether it answered at all.
struct Failure {
    reason: String,
    answered: bool,
}

impl Failure {
    const fn unreached(reason: String) -> Self {
        Self {
            reason,
            answered: false,
        }
    }

    const fn answered(reason: String) -> Self {
        Self {
            reason,
            answered: true,
        }
    }

    fn into_error(self, source: &str) -> FetchError {
        FetchError::Exhausted {
            attempts: vec![(source.to_owned(), self.reason)],
            answered: self.answered,
        }
    }
}

async fn read_response(response: crate::transfer::TransportResponse) -> Result<Vec<u8>, Failure> {
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(Failure::answered(format!("HTTP {status}")));
    }
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        // A body cut off on the way is the connection's failure.
        let chunk = chunk.map_err(|error| Failure::unreached(error.to_string()))?;
        if bytes.len() + chunk.len() > DOCUMENT_LIMIT {
            return Err(Failure::answered(
                "document is larger than the limit".to_owned(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::{TransportError, TransportFuture, TransportResponse};

    struct Scripted;

    impl Transport for Scripted {
        fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
            Box::pin(async move {
                match source {
                    "ok://a" => Ok(TransportResponse::from_bytes(200, b"hello".to_vec())),
                    "gone://a" => Ok(TransportResponse::from_bytes(404, Vec::new())),
                    _ => Err(TransportError::transient("offline")),
                }
            })
        }
    }

    /// Unreachable sources are asked twice; the paused clock skips the wait.
    fn scripted() -> TransferEngine<Scripted> {
        TransferEngine::new(Scripted, 1).unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn falls_through_failing_sources_to_the_first_answer() {
        let bytes = fetch_document(&scripted(), &["down://a", "gone://a", "ok://a"])
            .await
            .unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[tokio::test(start_paused = true)]
    async fn reports_every_attempt_when_all_fail() {
        let error = fetch_document(&scripted(), &["down://a", "gone://a"])
            .await
            .unwrap_err();
        let FetchError::Exhausted { attempts, .. } = error else {
            panic!("expected exhaustion");
        };
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1].1, "HTTP 404");
    }

    /// A source that answers without the document is told apart from one
    /// that cannot be reached: the first means it is not there, the second
    /// that the network or the host is down.
    #[tokio::test(start_paused = true)]
    async fn an_answer_without_the_document_is_not_being_offline() {
        let gone = fetch_document(&scripted(), &["down://a", "gone://a"])
            .await
            .unwrap_err();
        assert!(gone.answered());
        let down = fetch_document(&scripted(), &["down://a", "down://b"])
            .await
            .unwrap_err();
        assert!(!down.answered());
        let undecodable = fetch_decoded(&scripted(), &["ok://a"], |_| Err::<(), _>("not json"))
            .await
            .unwrap_err();
        assert!(undecodable.answered());
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_an_empty_source_list() {
        let none: [&str; 0] = [];
        assert_eq!(
            fetch_document(&scripted(), &none).await.unwrap_err(),
            FetchError::NoSources
        );
    }
}
