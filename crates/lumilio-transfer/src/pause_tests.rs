use std::path::Path;
use std::time::Duration;

use sha1::{Digest, Sha1};

use super::budget::ConnectionBudget;
use super::engine::TransferEngine;
use super::request::{RetryPolicy, TransferError, TransferEvent, TransferRequest};
use super::test_server::{Ending, Server};
use crate::CancellationToken;

fn content() -> Vec<u8> {
    (0..100_u8).collect()
}

fn request(id: &str, destination: &Path) -> TransferRequest {
    TransferRequest::new(id, [format!("https://example.invalid/{id}")], destination)
        .unwrap()
        .expect_sha1(super::files::encode_lower_hex(&Sha1::digest(content())))
        .unwrap()
        .with_retry_policy(RetryPolicy::fixed(2, Duration::ZERO, Duration::ZERO).unwrap())
}

/// Waits until `check` holds, for at most two seconds.
async fn eventually(check: impl Fn() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the condition never held");
}

/// Pauses `pause` once the engine reports its first bytes.
async fn pause_after_first_bytes(engine: &TransferEngine<Server>, pause: CancellationToken) {
    let mut events = engine.subscribe();
    while let Ok(event) = events.recv().await {
        if matches!(event, TransferEvent::Progress { .. }) {
            pause.pause();
            return;
        }
    }
}

#[tokio::test]
async fn a_paused_download_lets_its_connection_go_and_continues_when_resumed() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::StallAfter(30));
    let budget = ConnectionBudget::new(1).unwrap();
    let paused = TransferEngine::with_budget(server.clone(), budget.clone());
    let other = TransferEngine::with_budget(server.clone(), budget.clone());
    let control = CancellationToken::new();
    let mut reported = paused.subscribe();

    let first = {
        let paused = paused.clone();
        let control = control.clone();
        let destination = dir.path().join("first");
        tokio::spawn(async move {
            paused
                .transfer(request("first", &destination), control)
                .await
        })
    };
    pause_after_first_bytes(&paused, control.clone()).await;
    eventually(|| budget.open() == 0).await;
    let mut connections = None;
    while let Ok(event) = reported.try_recv() {
        if let TransferEvent::Connections { count, .. } = event {
            connections = Some(count);
        }
    }
    assert_eq!(connections, Some(0), "a paused file says it holds none");

    // While it is paused, the only connection serves another download.
    other
        .transfer(
            request("second", &dir.path().join("second")),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!first.is_finished());

    control.resume();
    first.await.unwrap().unwrap();
    assert_eq!(std::fs::read(dir.path().join("first")).unwrap(), content());
    let asked = server.asked();
    assert_eq!(asked[0], (0, None));
    assert_eq!(asked[2], (30, Some("\"v1\"".to_owned())));
}

#[tokio::test]
async fn a_paused_download_can_still_be_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::StallAfter(30));
    let engine = TransferEngine::new(server, 1).unwrap();
    let control = CancellationToken::new();

    let transfer = {
        let engine = engine.clone();
        let control = control.clone();
        let destination = dir.path().join("file");
        tokio::spawn(async move {
            engine
                .transfer(request("file", &destination), control)
                .await
        })
    };
    pause_after_first_bytes(&engine, control.clone()).await;
    eventually(|| engine.budget().open() == 0).await;
    control.cancel();

    assert_eq!(
        transfer.await.unwrap().unwrap_err(),
        TransferError::Cancelled
    );
}

#[tokio::test]
async fn a_download_paused_before_it_starts_waits_without_a_connection() {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::new(&content(), Some("\"v1\""), true);
    let engine = TransferEngine::new(server.clone(), 1).unwrap();
    let control = CancellationToken::new();
    control.pause();

    let transfer = {
        let engine = engine.clone();
        let control = control.clone();
        let destination = dir.path().join("file");
        tokio::spawn(async move {
            engine
                .transfer(request("file", &destination), control)
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(server.asked().is_empty());
    assert_eq!(engine.budget().open(), 0);

    control.resume();
    transfer.await.unwrap().unwrap();
    assert_eq!(server.asked().len(), 1);
}
