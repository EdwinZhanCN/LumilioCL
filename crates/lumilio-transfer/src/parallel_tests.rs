use super::budget::ConnectionBudget;
use super::engine::TransferEngine;
use super::partial::part_path;
use super::request::{RetryPolicy, TransferEvent, TransferRequest};
use super::test_server::Server;
use crate::CancellationToken;
use sha1::{Digest, Sha1};
use std::path::Path;
use std::time::Duration;

const SOURCE: &str = "https://example.invalid/large";
const FINAL: &str = "https://cdn.example.invalid/large?signed";

fn content() -> Vec<u8> {
    (0..64 * 1024_u32)
        .map(|index| (index % 251) as u8)
        .collect()
}

/// A server that serves ranges in 1 KiB chunks a little apart, so other
/// connections have time to join.
fn server(ranges: bool) -> Server {
    let server = Server::new(&content(), Some("\"v1\""), ranges);
    {
        let mut state = server.state();
        state.chunk = 1024;
        state.delay = Duration::from_millis(2);
        state.redirect = Some(FINAL.to_owned());
    }
    server
}

fn request(destination: &Path) -> TransferRequest {
    TransferRequest::new("large", [SOURCE], destination)
        .unwrap()
        .expect_sha1(super::files::encode_lower_hex(&Sha1::digest(content())))
        .unwrap()
        .with_retry_policy(RetryPolicy::fixed(3, Duration::ZERO, Duration::ZERO).unwrap())
}

fn engine(server: &Server, budget: &ConnectionBudget) -> TransferEngine<Server> {
    TransferEngine::with_budget(server.clone(), budget.clone()).splitting(16 * 1024, 4 * 1024, 4)
}

#[tokio::test]
async fn a_large_file_is_written_over_the_free_connections() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    let server = server(true);
    // Fewer connections than one file may use.
    let budget = ConnectionBudget::new(3).unwrap();
    let engine = engine(&server, &budget);
    let mut events = engine.subscribe();

    engine
        .transfer(request(&destination), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert!(!part_path(&destination).exists());
    let most = server.most_open();
    assert!((2..=3).contains(&most), "{most} answers were open at once");
    let requests = server.requests();
    // Every range after the first answer comes from where it was served.
    assert!(requests[1..].iter().all(|asked| asked.url == FINAL));
    let mut most_reported = 0;
    while let Ok(event) = events.try_recv() {
        if let TransferEvent::Connections { count, .. } = event {
            most_reported = most_reported.max(count);
        }
    }
    assert!(most_reported >= 2);
    assert_eq!(budget.open(), 0);
}

#[tokio::test]
async fn a_large_file_beside_a_batch_never_exceeds_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    let server = server(true);
    let budget = ConnectionBudget::new(3).unwrap();
    let large = engine(&server, &budget);
    let batch = engine(&server, &budget);
    let small: Vec<TransferRequest> = (0..6)
        .map(|index| {
            TransferRequest::new(
                format!("small{index}"),
                [format!("https://example.invalid/small{index}")],
                dir.path().join(format!("small{index}")),
            )
            .unwrap()
        })
        .collect();

    let (one, many) = tokio::join!(
        large.transfer(request(&dir.path().join("large")), CancellationToken::new()),
        batch.transfer_batch(small, CancellationToken::new()),
    );

    one.unwrap();
    assert_eq!(many.succeeded(), 6);
    assert_eq!(std::fs::read(dir.path().join("large")).unwrap(), content());
    let most = server.most_open();
    assert!(most <= 3, "{most} answers were open at once");
}

#[tokio::test]
async fn a_source_that_answers_ranges_whole_is_read_over_one_connection() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    // It says it serves ranges, then answers them with the whole file.
    let server = server(false);
    server.state().advertise = true;
    let budget = ConnectionBudget::new(4).unwrap();

    let outcome = engine(&server, &budget)
        .transfer(request(&destination), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    // Those that tried to join were answered whole and let go; the file
    // still arrived in the first attempt.
    assert!(matches!(
        outcome,
        super::request::TransferOutcome::Downloaded { attempts: 1, .. }
    ));
    assert!(server.requests().len() > 1);
}

#[tokio::test]
async fn without_ranges_a_large_file_uses_one_connection() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    let server = server(false);
    let budget = ConnectionBudget::new(4).unwrap();

    engine(&server, &budget)
        .transfer(request(&destination), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn a_full_budget_leaves_a_large_file_to_one_connection() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    let server = server(true);
    let budget = ConnectionBudget::new(2).unwrap();
    // Another download holds the only other connection.
    let held = budget.acquire().await.unwrap();

    engine(&server, &budget)
        .transfer(request(&destination), CancellationToken::new())
        .await
        .unwrap();

    drop(held);
    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn a_large_file_started_on_a_full_budget_splits_once_a_connection_frees() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    let server = server(true);
    // Slow enough that much is left when the first look for help comes.
    server.state().delay = Duration::from_millis(10);
    let budget = ConnectionBudget::new(3).unwrap();
    // A batch holds the other connections as the file starts.
    let held = (
        budget.acquire().await.unwrap(),
        budget.acquire().await.unwrap(),
    );
    let release = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(held);
    };

    let engine = engine(&server, &budget);
    let (outcome, ()) = tokio::join!(
        engine.transfer(request(&destination), CancellationToken::new()),
        release,
    );

    outcome.unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), content());
    let requests = server.requests();
    assert!(requests.len() > 1, "it never split: {requests:?}");
    assert!(requests[1..].iter().all(|asked| asked.url == FINAL));
    assert!(server.most_open() <= 3);
}

#[tokio::test]
async fn a_cancelled_download_over_several_connections_is_continued_later() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("large");
    let partials = dir.path().join("partials");
    let server = server(true);
    let budget = ConnectionBudget::new(4).unwrap();
    let first = engine(&server, &budget).keeping_partials(&partials);
    let cancel = CancellationToken::new();
    let mut events = first.subscribe();
    let watcher = {
        let cancel = cancel.clone();
        async move {
            while let Ok(event) = events.recv().await {
                if let TransferEvent::Progress { completed, .. } = event
                    && completed >= 24 * 1024
                {
                    cancel.cancel();
                    return;
                }
            }
        }
    };
    let (cancelled, ()) = tokio::join!(first.transfer(request(&destination), cancel), watcher);
    assert!(cancelled.is_err());
    assert!(part_path(&destination).exists());
    let asked_before = server.requests().len();

    engine(&server, &budget)
        .keeping_partials(&partials)
        .transfer(request(&destination), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    let continued = &server.requests()[asked_before];
    assert!(continued.start > 0, "started over: {continued:?}");
    assert_eq!(continued.validator.as_deref(), Some("\"v1\""));
    assert_eq!(std::fs::read_dir(&partials).unwrap().count(), 0);
}
