use super::budget::ConnectionBudget;
use super::engine::TransferEngine;
use super::request::TransferRequest;
use super::transport::{Transport, TransportFuture, TransportResponse};
use crate::CancellationToken;
use futures_util::FutureExt;
use futures_util::stream;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Serves every file slowly and records how many it served at once.
#[derive(Clone, Default)]
struct Counting {
    open: Arc<AtomicUsize>,
    most: Arc<AtomicUsize>,
    served: Arc<Mutex<Vec<String>>>,
}

struct Open(Arc<AtomicUsize>);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Transport for Counting {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            let now = self.open.fetch_add(1, Ordering::SeqCst) + 1;
            self.most.fetch_max(now, Ordering::SeqCst);
            let open = Open(self.open.clone());
            let served = self.served.clone();
            let source = source.to_owned();
            let body = stream::once(async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                served
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(source);
                drop(open);
                Ok(b"file".to_vec())
            });
            Ok(TransportResponse::new(200, Some(4), Box::pin(body)))
        })
    }
}

fn requests(dir: &Path, batch: &str, count: usize) -> Vec<TransferRequest> {
    (0..count)
        .map(|index| {
            let name = format!("{batch}{index}");
            TransferRequest::new(
                name.clone(),
                [format!("https://example.invalid/{name}")],
                dir.join(&name),
            )
            .unwrap()
        })
        .collect()
}

#[tokio::test]
async fn engines_on_one_budget_never_exceed_it_together() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Counting::default();
    let budget = ConnectionBudget::new(2).unwrap();
    let first = TransferEngine::with_budget(transport.clone(), budget.clone());
    let second = TransferEngine::with_budget(transport.clone(), budget);

    let (a, b) = tokio::join!(
        first.transfer_batch(requests(dir.path(), "a", 5), CancellationToken::new()),
        second.transfer_batch(requests(dir.path(), "b", 5), CancellationToken::new()),
    );

    assert_eq!((a.succeeded(), b.succeeded()), (5, 5));
    assert_eq!(transport.most.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_later_batch_takes_turns_instead_of_waiting_for_a_large_one() {
    let dir = tempfile::tempdir().unwrap();
    let transport = Counting::default();
    let budget = ConnectionBudget::new(2).unwrap();
    let large = TransferEngine::with_budget(transport.clone(), budget.clone());
    let small = TransferEngine::with_budget(transport.clone(), budget);

    tokio::join!(
        large.transfer_batch(requests(dir.path(), "large", 8), CancellationToken::new()),
        small.transfer_batch(requests(dir.path(), "small", 2), CancellationToken::new()),
    );

    let served = transport.served.lock().unwrap().clone();
    let last_small = served
        .iter()
        .rposition(|source| source.contains("small"))
        .unwrap();
    assert!(
        last_small < served.len() - 2,
        "the small batch finished last: {served:?}"
    );
}

#[tokio::test]
async fn growing_applies_at_once_and_shrinking_as_connections_finish() {
    let budget = ConnectionBudget::new(2).unwrap();
    let first = budget.acquire().await.unwrap();
    let second = budget.acquire().await.unwrap();

    budget.resize(3).unwrap();
    let third = budget.acquire().now_or_never().unwrap().unwrap();

    budget.resize(1).unwrap();
    drop(first);
    drop(second);
    assert!(budget.acquire().now_or_never().is_none());
    drop(third);
    let only = budget.acquire().now_or_never().unwrap().unwrap();
    assert!(budget.acquire().now_or_never().is_none());
    drop(only);
    assert_eq!(budget.size(), 1);
}

#[test]
fn a_budget_needs_at_least_one_connection() {
    assert!(ConnectionBudget::new(0).is_err());
    let budget = ConnectionBudget::new(1).unwrap();
    assert!(budget.resize(0).is_err());
    assert_eq!(budget.size(), 1);
}
