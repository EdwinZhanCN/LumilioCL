use std::collections::{BTreeMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use lumilio_core::{
    ActivityGraph, ActivityNode, ActivityScheduler, ActivityState, CancellationToken,
    FileTransport, OfficialSource, PrefixMirror, RetryPolicy, SourceChain, SourceProvider,
    TransferEngine, TransferError, TransferEvent, TransferOutcome, TransferRequest, Transport,
    TransportError, TransportResponse,
};
use sha1::{Digest, Sha1};
use tempfile::tempdir;

#[derive(Clone, Debug)]
enum Reply {
    Body { status: u16, bytes: Vec<u8> },
    Failure(String),
}

#[derive(Clone, Default)]
struct ScriptedTransport {
    replies: Arc<Mutex<BTreeMap<String, VecDeque<Reply>>>>,
    calls: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Copy)]
struct NeverTransport;

impl Transport for NeverTransport {
    fn get<'a>(
        &'a self,
        _source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + Send + 'a>> {
        Box::pin(std::future::pending())
    }
}

#[derive(Clone, Copy)]
struct PartialTransport;

impl Transport for PartialTransport {
    fn get<'a>(
        &'a self,
        _source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + Send + 'a>> {
        Box::pin(async {
            let first = stream::once(async { Ok(b"partial".to_vec()) });
            let never = stream::pending::<Result<Vec<u8>, TransportError>>();
            Ok(TransportResponse::new(
                200,
                Some(14),
                Box::pin(first.chain(never)),
            ))
        })
    }
}

impl ScriptedTransport {
    fn push(&self, source: &str, reply: Reply) {
        self.replies
            .lock()
            .unwrap()
            .entry(source.to_owned())
            .or_default()
            .push_back(reply);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Transport for ScriptedTransport {
    fn get<'a>(
        &'a self,
        source: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<TransportResponse, TransportError>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(source.to_owned());
            let reply = self
                .replies
                .lock()
                .unwrap()
                .get_mut(source)
                .and_then(VecDeque::pop_front)
                .unwrap_or_else(|| Reply::Failure("no scripted response".to_owned()));
            match reply {
                Reply::Body { status, bytes } => Ok(TransportResponse::from_bytes(status, bytes)),
                Reply::Failure(message) => Err(TransportError::transient(message)),
            }
        })
    }
}

fn sha1(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[tokio::test]
async fn activity_graph_orders_dependencies_and_blocks_failed_descendants() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let mut graph = ActivityGraph::new();

    let prepare_order = order.clone();
    graph
        .add(ActivityNode::new("prepare", "Prepare", move |context| {
            let order = prepare_order.clone();
            async move {
                context.report_progress(1, Some(1))?;
                order.lock().unwrap().push("prepare");
                Ok::<(), String>(())
            }
        }))
        .unwrap();

    let install_order = order.clone();
    graph
        .add(
            ActivityNode::new("install", "Install", move |_context| {
                let order = install_order.clone();
                async move {
                    order.lock().unwrap().push("install");
                    Err("installer rejected input".to_owned())
                }
            })
            .after("prepare"),
        )
        .unwrap();

    let launch_order = order.clone();
    graph
        .add(
            ActivityNode::new("launch", "Launch", move |_context| {
                let order = launch_order.clone();
                async move {
                    order.lock().unwrap().push("launch");
                    Ok::<(), String>(())
                }
            })
            .after("install"),
        )
        .unwrap();

    let scheduler = ActivityScheduler::new(2).unwrap();
    let mut events = scheduler.subscribe();
    let report = scheduler
        .run(graph, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(*order.lock().unwrap(), ["prepare", "install"]);
    assert_eq!(report.state("prepare"), Some(ActivityState::Succeeded));
    assert_eq!(
        report.records()["prepare"].progress().unwrap().completed(),
        1
    );
    assert_eq!(report.state("install"), Some(ActivityState::Failed));
    assert_eq!(report.state("launch"), Some(ActivityState::Blocked));
    assert_eq!(report.failure("install"), Some("installer rejected input"));
    let mut terminal_ids = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let lumilio_core::ActivityEvent::Finished { id, .. } = event {
            terminal_ids.push(id);
        }
    }
    terminal_ids.sort();
    assert_eq!(terminal_ids, ["install", "launch", "prepare"]);
}

#[tokio::test]
async fn cancellation_prevents_queued_activity_execution() {
    let executed = Arc::new(Mutex::new(false));
    let marker = executed.clone();
    let mut graph = ActivityGraph::new();
    graph
        .add(ActivityNode::new(
            "cancelled",
            "Cancelled",
            move |_context| {
                let marker = marker.clone();
                async move {
                    *marker.lock().unwrap() = true;
                    Ok::<(), String>(())
                }
            },
        ))
        .unwrap();

    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let report = ActivityScheduler::new(1)
        .unwrap()
        .run(graph, cancellation)
        .await
        .unwrap();

    assert!(!*executed.lock().unwrap());
    assert_eq!(report.state("cancelled"), Some(ActivityState::Cancelled));
}

#[tokio::test]
async fn corrupted_source_falls_back_without_publishing_partial_bytes() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("libraries/example.jar");
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    std::fs::write(&destination, b"old-valid-file").unwrap();

    let transport = ScriptedTransport::default();
    transport.push(
        "https://mirror.invalid/example.jar",
        Reply::Body {
            status: 200,
            bytes: b"corrupt".to_vec(),
        },
    );
    transport.push(
        "https://origin.invalid/example.jar",
        Reply::Body {
            status: 200,
            bytes: b"verified-content".to_vec(),
        },
    );

    let request = TransferRequest::new(
        "example-library",
        [
            "https://mirror.invalid/example.jar",
            "https://origin.invalid/example.jar",
        ],
        &destination,
    )
    .unwrap()
    .expect_size(16)
    .expect_sha1(sha1(b"verified-content"))
    .unwrap()
    .with_retry_policy(RetryPolicy::immediate(1).unwrap());
    let outcome = TransferEngine::new(transport.clone(), 2)
        .unwrap()
        .transfer(request, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(std::fs::read(&destination).unwrap(), b"verified-content");
    assert_eq!(
        transport.calls(),
        [
            "https://mirror.invalid/example.jar",
            "https://origin.invalid/example.jar"
        ]
    );
    assert_eq!(
        outcome,
        TransferOutcome::Downloaded {
            source: "https://origin.invalid/example.jar".to_owned(),
            attempts: 2,
            bytes: 16,
        }
    );
    assert!(
        std::fs::read_dir(destination.parent().unwrap())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".part"))
    );
}

#[tokio::test]
async fn exhausted_sources_leave_existing_destination_untouched() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("client.jar");
    std::fs::write(&destination, b"previous-client").unwrap();

    let transport = ScriptedTransport::default();
    transport.push(
        "https://origin.invalid/client.jar",
        Reply::Body {
            status: 200,
            bytes: b"wrong".to_vec(),
        },
    );
    let request = TransferRequest::new(
        "client",
        ["https://origin.invalid/client.jar"],
        &destination,
    )
    .unwrap()
    .expect_sha1(sha1(b"expected"))
    .unwrap()
    .with_retry_policy(RetryPolicy::immediate(1).unwrap());

    let error = TransferEngine::new(transport, 1)
        .unwrap()
        .transfer(request, CancellationToken::new())
        .await
        .unwrap_err();

    assert!(matches!(error, TransferError::AllSourcesFailed { .. }));
    assert_eq!(std::fs::read(destination).unwrap(), b"previous-client");
}

#[tokio::test]
async fn verified_cache_candidate_avoids_network_access() {
    let temp = tempdir().unwrap();
    let cache = temp.path().join("cache/object");
    let destination = temp.path().join("assets/object");
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, b"cached-content").unwrap();

    let transport = ScriptedTransport::default();
    let request = TransferRequest::new("asset", ["https://origin.invalid/asset"], &destination)
        .unwrap()
        .expect_size(14)
        .expect_sha1(sha1(b"cached-content"))
        .unwrap()
        .with_cache_candidate(&cache);

    let outcome = TransferEngine::new(transport.clone(), 1)
        .unwrap()
        .transfer(request, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(outcome, TransferOutcome::ReusedCache { source: cache });
    assert_eq!(std::fs::read(destination).unwrap(), b"cached-content");
    assert!(transport.calls().is_empty());
}

#[tokio::test]
async fn batch_report_preserves_success_when_a_sibling_fails() {
    let temp = tempdir().unwrap();
    let transport = ScriptedTransport::default();
    transport.push(
        "https://origin.invalid/good",
        Reply::Body {
            status: 200,
            bytes: b"good".to_vec(),
        },
    );
    transport.push(
        "https://origin.invalid/missing",
        Reply::Body {
            status: 404,
            bytes: Vec::new(),
        },
    );
    let good = TransferRequest::new(
        "good",
        ["https://origin.invalid/good"],
        temp.path().join("good"),
    )
    .unwrap()
    .expect_size(4);
    let missing = TransferRequest::new(
        "missing",
        ["https://origin.invalid/missing"],
        temp.path().join("missing"),
    )
    .unwrap()
    .with_retry_policy(
        RetryPolicy::fixed(3, Duration::from_millis(1), Duration::from_millis(1)).unwrap(),
    );

    let report = TransferEngine::new(transport, 2)
        .unwrap()
        .transfer_batch(vec![good, missing], CancellationToken::new())
        .await;

    assert_eq!(report.succeeded(), 1);
    assert_eq!(report.failed(), 1);
    assert!(report.result("good").unwrap().is_ok());
    assert!(report.result("missing").unwrap().is_err());
}

#[tokio::test]
async fn cancellation_interrupts_a_transport_wait() {
    let temp = tempdir().unwrap();
    let request = TransferRequest::new(
        "waiting",
        ["https://origin.invalid/waiting"],
        temp.path().join("waiting"),
    )
    .unwrap();
    let cancellation = CancellationToken::new();
    let trigger = cancellation.clone();
    tokio::spawn(async move {
        tokio::task::yield_now().await;
        trigger.cancel();
    });

    let error = TransferEngine::new(NeverTransport, 1)
        .unwrap()
        .transfer(request, cancellation)
        .await
        .unwrap_err();

    assert_eq!(error, TransferError::Cancelled);
}

#[tokio::test]
async fn cancellation_removes_a_partial_file_and_preserves_the_destination() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("partial.bin");
    std::fs::write(&destination, b"previous").unwrap();
    let request = TransferRequest::new("partial", ["https://origin.invalid/partial"], &destination)
        .unwrap()
        .expect_size(14);
    let engine = TransferEngine::new(PartialTransport, 1).unwrap();
    let mut events = engine.subscribe();
    let cancellation = CancellationToken::new();
    let running_engine = engine.clone();
    let running_cancellation = cancellation.clone();
    let transfer =
        tokio::spawn(async move { running_engine.transfer(request, running_cancellation).await });

    loop {
        if matches!(events.recv().await.unwrap(), TransferEvent::Progress { .. }) {
            break;
        }
    }
    cancellation.cancel();
    assert_eq!(
        transfer.await.unwrap().unwrap_err(),
        TransferError::Cancelled
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"previous");
    assert!(
        std::fs::read_dir(destination.parent().unwrap())
            .unwrap()
            .all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".part"))
    );
}

#[tokio::test]
async fn production_file_transport_streams_local_sources() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("source.bin");
    let destination = temp.path().join("destination.bin");
    std::fs::write(&source, b"local-source").unwrap();
    let source_url = url::Url::from_file_path(&source).unwrap().to_string();
    let request = TransferRequest::new("local", [source_url], &destination)
        .unwrap()
        .expect_size(12)
        .expect_sha1(sha1(b"local-source"))
        .unwrap();

    let outcome = TransferEngine::new(FileTransport, 1)
        .unwrap()
        .transfer(request, CancellationToken::new())
        .await
        .unwrap();

    assert!(matches!(
        outcome,
        TransferOutcome::Downloaded { bytes: 12, .. }
    ));
    assert_eq!(std::fs::read(destination).unwrap(), b"local-source");
}

#[test]
fn source_chain_preserves_provider_priority_and_removes_duplicates() {
    let mirror: Arc<dyn SourceProvider> = Arc::new(
        PrefixMirror::new(
            "https://libraries.minecraft.net/",
            "https://mirror.invalid/maven/",
            8,
        )
        .unwrap(),
    );
    let official: Arc<dyn SourceProvider> = Arc::new(OfficialSource);
    let duplicate_official: Arc<dyn SourceProvider> = Arc::new(OfficialSource);
    let chain = SourceChain::new([mirror, official, duplicate_official]).unwrap();

    assert_eq!(
        chain.candidates("https://libraries.minecraft.net/org/example/client.jar"),
        [
            "https://mirror.invalid/maven/org/example/client.jar",
            "https://libraries.minecraft.net/org/example/client.jar"
        ]
    );
    assert_eq!(chain.preferred_concurrency(), 8);
}

#[derive(Clone)]
struct GateTransport {
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Semaphore>,
    shared_calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl Transport for GateTransport {
    fn get<'a>(&'a self, source: &'a str) -> lumilio_core::TransportFuture<'a> {
        Box::pin(async move {
            if source.ends_with("shared") {
                let call = self
                    .shared_calls
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if call == 0 {
                    self.started.notify_one();
                    self.release.acquire().await.unwrap().forget();
                }
            }
            Ok(TransportResponse::from_bytes(200, b"resource".to_vec()))
        })
    }
}

#[tokio::test]
async fn separate_engines_coordinate_one_destination_without_blocking_other_files() {
    let dir = tempdir().unwrap();
    let transport = GateTransport {
        started: Arc::new(tokio::sync::Notify::new()),
        release: Arc::new(tokio::sync::Semaphore::new(0)),
        shared_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
    };
    let engine = TransferEngine::new(transport.clone(), 1).unwrap();
    let second_engine = TransferEngine::new(transport.clone(), 1).unwrap();
    let request = TransferRequest::new(
        "first",
        vec!["https://example.invalid/shared".to_owned()],
        dir.path().join("shared"),
    )
    .unwrap()
    .expect_sha1(sha1(b"resource"))
    .unwrap();
    let first_request = request.clone();
    let first = tokio::spawn(async move {
        engine
            .transfer(first_request, CancellationToken::new())
            .await
    });
    transport.started.notified().await;
    let cancel = CancellationToken::new();
    let mut waiting = Box::pin(second_engine.transfer(request.clone(), cancel.clone()));
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut waiting)
            .await
            .is_err()
    );
    let other = TransferRequest::new(
        "other",
        vec!["https://example.invalid/other".to_owned()],
        dir.path().join("other"),
    )
    .unwrap();
    tokio::time::timeout(
        Duration::from_secs(2),
        second_engine.transfer(other, CancellationToken::new()),
    )
    .await
    .unwrap()
    .unwrap();
    cancel.cancel();
    assert_eq!(waiting.await, Err(TransferError::Cancelled));
    assert_eq!(
        transport
            .shared_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    transport.release.add_permits(1);
    first.await.unwrap().unwrap();
    assert_eq!(
        second_engine
            .transfer(request, CancellationToken::new())
            .await
            .unwrap(),
        TransferOutcome::ReusedExisting
    );
    assert_eq!(
        transport
            .shared_calls
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        std::fs::read(dir.path().join("shared")).unwrap(),
        b"resource"
    );
}
