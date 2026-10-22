use super::engine::TransferEngine;
use super::partial::{part_path, sweep_partials};
use super::request::{RetryPolicy, TransferError, TransferEvent, TransferRequest};
use super::transport::{ByteStream, Transport, TransportError, TransportFuture, TransportResponse};
use crate::CancellationToken;
use futures_util::stream::{self, StreamExt};
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SOURCE: &str = "https://example.invalid/file";

/// How the next answer ends after the bytes it was asked for start.
#[derive(Clone, Copy)]
enum Ending {
    Whole,
    /// A network error after this many bytes.
    BreakAfter(usize),
    /// No more bytes until cancelled, after this many.
    StallAfter(usize),
}

#[derive(Default)]
struct State {
    content: Vec<u8>,
    etag: Option<String>,
    ranges: bool,
    endings: VecDeque<Ending>,
    /// Each request's starting byte and `If-Range`.
    asked: Vec<(u64, Option<String>)>,
}

#[derive(Clone, Default)]
struct Server(Arc<Mutex<State>>);

impl Server {
    fn new(content: &[u8], etag: Option<&str>, ranges: bool) -> Self {
        let server = Self::default();
        {
            let mut state = server.state();
            state.content = content.to_vec();
            state.etag = etag.map(str::to_owned);
            state.ranges = ranges;
        }
        server
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn then(&self, ending: Ending) -> &Self {
        self.state().endings.push_back(ending);
        self
    }

    fn asked(&self) -> Vec<(u64, Option<String>)> {
        self.state().asked.clone()
    }

    fn answer(&self, offset: u64, validator: Option<&str>) -> TransportResponse {
        let mut state = self.state();
        state.asked.push((offset, validator.map(str::to_owned)));
        let fresh = validator.is_none_or(|validator| state.etag.as_deref() == Some(validator));
        let start = if offset > 0 && state.ranges && fresh {
            offset as usize
        } else {
            0
        };
        let length = state.content.len();
        let rest = state.content[start..].to_vec();
        let ending = state.endings.pop_front().unwrap_or(Ending::Whole);
        let body: ByteStream = match ending {
            Ending::Whole => Box::pin(stream::iter([Ok(rest.clone())])),
            Ending::BreakAfter(count) => Box::pin(stream::iter([
                Ok(rest[..count].to_vec()),
                Err(TransportError::transient("connection reset")),
            ])),
            Ending::StallAfter(count) => {
                Box::pin(stream::iter([Ok(rest[..count].to_vec())]).chain(stream::pending()))
            }
        };
        let mut headers = Vec::new();
        if let Some(etag) = &state.etag {
            headers.push(("etag".to_owned(), etag.clone()));
        }
        let status = if start > 0 {
            headers.push((
                "content-range".to_owned(),
                format!("bytes {start}-{}/{length}", length - 1),
            ));
            206
        } else {
            200
        };
        TransportResponse::new(status, Some(rest.len() as u64), body).with_headers(headers)
    }
}

impl Transport for Server {
    fn get<'a>(&'a self, _source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move { Ok(self.answer(0, None)) })
    }

    fn get_from<'a>(
        &'a self,
        _source: &'a str,
        offset: u64,
        validator: Option<&'a str>,
    ) -> TransportFuture<'a> {
        Box::pin(async move { Ok(self.answer(offset, validator)) })
    }
}

fn content() -> Vec<u8> {
    (0..100_u8).collect()
}

fn sha1(bytes: &[u8]) -> String {
    super::files::encode_lower_hex(&Sha1::digest(bytes))
}

fn request(destination: &Path, digest: Option<&[u8]>) -> TransferRequest {
    let request = TransferRequest::new("file", [SOURCE], destination)
        .unwrap()
        .with_retry_policy(RetryPolicy::fixed(3, Duration::ZERO, Duration::ZERO).unwrap());
    match digest {
        Some(bytes) => request.expect_sha1(sha1(bytes)).unwrap(),
        None => request,
    }
}

fn engine(server: &Server, partials: Option<&Path>) -> TransferEngine<Server> {
    let engine = TransferEngine::new(server.clone(), 1).unwrap();
    match partials {
        Some(folder) => engine.keeping_partials(folder),
        None => engine,
    }
}

#[tokio::test]
async fn a_broken_download_continues_where_it_stopped() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::BreakAfter(40));

    engine(&server, None)
        .transfer(
            request(&destination, Some(&content())),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert_eq!(server.asked(), [(0, None), (40, Some("\"v1\"".to_owned()))]);
    assert!(!part_path(&destination).exists());
}

#[tokio::test]
async fn a_source_that_ignores_ranges_is_downloaded_again_whole() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let server = Server::new(&content(), Some("\"v1\""), false);
    server.then(Ending::BreakAfter(40));

    engine(&server, None)
        .transfer(
            request(&destination, Some(&content())),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert_eq!(server.asked().len(), 2);
}

#[tokio::test]
async fn bytes_of_a_changed_file_are_not_continued() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::BreakAfter(40));
    let engine = engine(&server, Some(&dir.path().join("partials")));
    let cancel = CancellationToken::new();
    let once = request(&destination, None)
        .with_retry_policy(RetryPolicy::fixed(1, Duration::ZERO, Duration::ZERO).unwrap());
    assert!(engine.transfer(once, cancel).await.is_err());

    let changed: Vec<u8> = (100..200_u8).collect();
    {
        let mut state = server.state();
        state.content = changed.clone();
        state.etag = Some("\"v2\"".to_owned());
    }
    engine
        .transfer(request(&destination, None), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), changed);
    assert_eq!(server.asked()[1], (40, Some("\"v1\"".to_owned())));
}

#[tokio::test]
async fn kept_bytes_that_fail_verification_are_dropped_and_fetched_again() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let partials = dir.path().join("partials");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::BreakAfter(40));
    let engine = engine(&server, Some(&partials));
    let once = request(&destination, Some(&content()))
        .with_retry_policy(RetryPolicy::fixed(1, Duration::ZERO, Duration::ZERO).unwrap());
    assert!(
        engine
            .transfer(once, CancellationToken::new())
            .await
            .is_err()
    );
    // Something else spoiled the kept bytes.
    std::fs::write(part_path(&destination), [0xff; 40]).unwrap();

    engine
        .transfer(
            request(&destination, Some(&content())),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    let asked: Vec<u64> = server.asked().iter().map(|(offset, _)| *offset).collect();
    assert_eq!(asked, [0, 40, 0]);
}

async fn cancel_after_first_bytes(
    engine: &TransferEngine<Server>,
    destination: &Path,
) -> Result<super::request::TransferOutcome, TransferError> {
    let cancel = CancellationToken::new();
    let mut events = engine.subscribe();
    let watcher = {
        let cancel = cancel.clone();
        async move {
            while let Ok(event) = events.recv().await {
                if matches!(event, TransferEvent::Progress { .. }) {
                    cancel.cancel();
                    return;
                }
            }
        }
    };
    let (result, ()) = tokio::join!(
        engine.transfer(request(destination, Some(&content())), cancel.clone()),
        watcher
    );
    result
}

#[tokio::test]
async fn a_cancelled_download_is_continued_by_the_next_transfer() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let partials = dir.path().join("partials");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::StallAfter(30));

    let first = engine(&server, Some(&partials));
    assert!(matches!(
        cancel_after_first_bytes(&first, &destination).await,
        Err(TransferError::Cancelled)
    ));
    assert_eq!(
        std::fs::metadata(part_path(&destination)).unwrap().len(),
        30
    );

    // A new engine, as after the task is started again.
    engine(&server, Some(&partials))
        .transfer(
            request(&destination, Some(&content())),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), content());
    assert_eq!(server.asked()[1], (30, Some("\"v1\"".to_owned())));
    assert!(!part_path(&destination).exists());
    assert_eq!(std::fs::read_dir(&partials).unwrap().count(), 0);
}

#[tokio::test]
async fn without_a_partials_folder_a_cancelled_download_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::StallAfter(30));

    let result = cancel_after_first_bytes(&engine(&server, None), &destination).await;

    assert!(matches!(result, Err(TransferError::Cancelled)));
    assert!(!part_path(&destination).exists());
}

#[tokio::test]
async fn sweeping_removes_kept_downloads_older_than_the_age() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("file");
    let partials: PathBuf = dir.path().join("partials");
    let server = Server::new(&content(), Some("\"v1\""), true);
    server.then(Ending::StallAfter(30));
    let _ = cancel_after_first_bytes(&engine(&server, Some(&partials)), &destination).await;

    assert_eq!(sweep_partials(&partials, Duration::from_secs(3600)), 0);
    assert!(part_path(&destination).exists());
    assert_eq!(sweep_partials(&partials, Duration::ZERO), 1);
    assert!(!part_path(&destination).exists());
    assert_eq!(std::fs::read_dir(&partials).unwrap().count(), 0);
}
