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
