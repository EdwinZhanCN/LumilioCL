use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::FutureExt;

use super::budget::ConnectionBudget;
use super::engine::TransferEngine;
use super::request::RetryPolicy;
use super::transport::{Transport, TransportError, TransportFuture, TransportResponse};

/// Fails the first `failures` requests the way `failure` says, then answers
/// with a small document.
#[derive(Clone)]
struct Flaky {
    asked: Arc<AtomicUsize>,
    failures: usize,
    failure: fn() -> Result<TransportResponse, TransportError>,
}

impl Flaky {
    fn new(failures: usize, failure: fn() -> Result<TransportResponse, TransportError>) -> Self {
        Self {
            asked: Arc::default(),
            failures,
            failure,
        }
    }
}

impl Transport for Flaky {
    fn get<'a>(&'a self, _source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            if self.asked.fetch_add(1, Ordering::SeqCst) < self.failures {
                (self.failure)()
            } else {
                Ok(TransportResponse::from_bytes(
                    200,
                    b"{\"ok\":true}".to_vec(),
                ))
            }
        })
    }
}

fn twice() -> RetryPolicy {
    RetryPolicy::fixed(2, Duration::ZERO, Duration::ZERO).unwrap()
}

#[tokio::test]
async fn a_fetch_waits_for_a_connection_of_the_budget() {
    let budget = ConnectionBudget::new(1).unwrap();
    let engine = TransferEngine::with_budget(Flaky::new(0, || unreachable!()), budget.clone());
    let held = budget.acquire().await.unwrap();

    let retry = twice();
    let mut fetch = Box::pin(engine.fetch("https://example.invalid/a", 1024, &retry));
    assert!((&mut fetch).now_or_never().is_none());
    drop(held);

    assert_eq!(fetch.await.unwrap(), b"{\"ok\":true}");
    assert_eq!(budget.open(), 0);
}

#[tokio::test]
async fn a_fetch_asks_a_busy_source_again_and_reports_nothing() {
    let transport = Flaky::new(1, || Ok(TransportResponse::from_bytes(503, Vec::new())));
    let engine = TransferEngine::new(transport.clone(), 1).unwrap();
    let mut events = engine.subscribe();

    let bytes = engine
        .fetch("https://example.invalid/a", 1024, &twice())
        .await
        .unwrap();

    assert_eq!(bytes, b"{\"ok\":true}");
    assert_eq!(transport.asked.load(Ordering::SeqCst), 2);
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn a_missing_document_is_answered_and_not_asked_again() {
    let transport = Flaky::new(5, || Ok(TransportResponse::from_bytes(404, Vec::new())));
    let engine = TransferEngine::new(transport.clone(), 1).unwrap();

    let failure = engine
        .fetch("https://example.invalid/a", 1024, &twice())
        .await
        .unwrap_err();

    assert!(failure.answered());
    assert_eq!(failure.reason(), "HTTP 404");
    assert_eq!(transport.asked.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_unreachable_source_is_not_answered() {
    let transport = Flaky::new(5, || Err(TransportError::transient("offline")));
    let engine = TransferEngine::new(transport.clone(), 1).unwrap();

    let failure = engine
        .fetch("https://example.invalid/a", 1024, &twice())
        .await
        .unwrap_err();

    assert!(!failure.answered());
    assert_eq!(transport.asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_document_over_the_limit_is_refused() {
    let engine = TransferEngine::new(Flaky::new(0, || unreachable!()), 1).unwrap();

    let failure = engine
        .fetch("https://example.invalid/a", 4, &twice())
        .await
        .unwrap_err();

    assert!(failure.answered());
}
