//! Small-document retrieval over a [`Transport`] with ordered source fallback.
//!
//! Metadata documents (version catalogs, API answers) are small and are read
//! whole; large files go through the transfer engine instead.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use futures_util::StreamExt;

use crate::transfer::Transport;

#[cfg(test)]
mod decoded_tests;

/// Upper bound for a document read whole, so a misbehaving source cannot
/// exhaust memory.
pub const DOCUMENT_LIMIT: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FetchError {
    /// No source was supplied.
    NoSources,
    /// Every source failed; one `(source, reason)` pair per attempt.
    Exhausted(Vec<(String, String)>),
}

impl Display for FetchError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSources => f.write_str("no source to fetch from"),
            Self::Exhausted(attempts) => {
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
pub async fn fetch_document<T, S>(transport: &T, sources: &[S]) -> Result<Vec<u8>, FetchError>
where
    T: Transport + ?Sized,
    S: AsRef<str>,
{
    fetch_decoded(transport, sources, Ok::<_, std::convert::Infallible>).await
}

/// Decode each candidate before accepting it. Unusable metadata is a source
/// failure even when the response has a 2xx status.
pub async fn fetch_decoded<T, S, V, E>(
    transport: &T,
    sources: &[S],
    mut decode: impl FnMut(Vec<u8>) -> Result<V, E>,
) -> Result<V, FetchError>
where
    T: Transport + ?Sized,
    S: AsRef<str>,
    E: Display,
{
    if sources.is_empty() {
        return Err(FetchError::NoSources);
    }
    let mut attempts = Vec::new();
    for source in sources {
        let source = source.as_ref();
        match read_one(transport, source).await {
            Ok(bytes) => match decode(bytes) {
                Ok(value) => return Ok(value),
                Err(error) => attempts.push((source.to_owned(), error.to_string())),
            },
            Err(reason) => attempts.push((source.to_owned(), reason)),
        }
    }
    Err(FetchError::Exhausted(attempts))
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
        .map_err(|error| FetchError::Exhausted(vec![(source.to_owned(), error.to_string())]))?;
    read_response(response)
        .await
        .map_err(|reason| FetchError::Exhausted(vec![(source.to_owned(), reason)]))
}

async fn read_one<T: Transport + ?Sized>(transport: &T, source: &str) -> Result<Vec<u8>, String> {
    let response = transport
        .get(source)
        .await
        .map_err(|error| error.to_string())?;
    read_response(response).await
}

async fn read_response(response: crate::transfer::TransportResponse) -> Result<Vec<u8>, String> {
    let status = response.status();
    if !(200..300).contains(&status) {
        return Err(format!("HTTP {status}"));
    }
    let mut body = response.into_body();
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if bytes.len() + chunk.len() > DOCUMENT_LIMIT {
            return Err("document is larger than the limit".to_owned());
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

    #[tokio::test]
    async fn falls_through_failing_sources_to_the_first_answer() {
        let bytes = fetch_document(&Scripted, &["down://a", "gone://a", "ok://a"])
            .await
            .unwrap();
        assert_eq!(bytes, b"hello");
    }

    #[tokio::test]
    async fn reports_every_attempt_when_all_fail() {
        let error = fetch_document(&Scripted, &["down://a", "gone://a"])
            .await
            .unwrap_err();
        let FetchError::Exhausted(attempts) = error else {
            panic!("expected exhaustion");
        };
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[1].1, "HTTP 404");
    }

    #[tokio::test]
    async fn rejects_an_empty_source_list() {
        let none: [&str; 0] = [];
        assert_eq!(
            fetch_document(&Scripted, &none).await.unwrap_err(),
            FetchError::NoSources
        );
    }
}
