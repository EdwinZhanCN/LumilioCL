use super::*;
use crate::fetch::DOCUMENT_LIMIT;
use crate::transfer::{
    HttpMethod, HttpRequest, OfficialSource, PrefixMirror, SourceChain, Transport, TransportError,
    TransportFuture, TransportResponse,
};
use lumilio_plugin_api::{FetchRequest, FetchResponse};

#[derive(Clone, Default)]
struct Scripted {
    calls: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Transport for Scripted {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        panic!("permission-scoped reads must not use an auto-redirecting transport")
    }

    fn send_no_redirect<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.calls.lock().unwrap().push(request.clone());
        Box::pin(async move {
            let url = url::Url::parse(&request.url).unwrap();
            let path = url.path();
            if url.host_str() == Some("mirror.test") && path != "/ok" {
                return Err(TransportError::transient("mirror unavailable"));
            }
            match path {
                "/slow" => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(TransportResponse::from_bytes(200, b"ok".to_vec()))
                }
                "/foreign" => Ok(TransportResponse::from_bytes(302, Vec::new())
                    .with_headers(vec![("Location".into(), "https://evil.test/secret".into())])),
                "/relative" => Ok(TransportResponse::from_bytes(307, Vec::new())
                    .with_headers(vec![("Location".into(), "/ok".into())])),
                "/loop" => Ok(TransportResponse::from_bytes(302, Vec::new())
                    .with_headers(vec![("Location".into(), "/loop".into())])),
                "/downgrade" => Ok(TransportResponse::from_bytes(302, Vec::new())
                    .with_headers(vec![("Location".into(), "http://api.test/ok".into())])),
                "/other" => Ok(TransportResponse::from_bytes(307, Vec::new())
                    .with_headers(vec![("Location".into(), "https://other.test/ok".into())])),
                "/post-redirect" => Ok(TransportResponse::from_bytes(303, Vec::new())
                    .with_headers(vec![("Location".into(), "/ok".into())])),
                "/huge-header" => Ok(TransportResponse::new(
                    200,
                    Some(DOCUMENT_LIMIT as u64 + 1),
                    Box::pin(futures_util::stream::empty()),
                )),
                "/huge-stream" => Ok(TransportResponse::new(
                    200,
                    None,
                    Box::pin(futures_util::stream::iter([
                        Ok(vec![0; DOCUMENT_LIMIT]),
                        Ok(vec![0; 1]),
                    ])),
                )),
                "/error" => Ok(TransportResponse::from_bytes(404, b"not found".to_vec())),
                _ => Ok(TransportResponse::from_bytes(200, b"ok".to_vec())),
            }
        })
    }
}

struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.network".into(),
            name: "网络测试".into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            settings: Vec::new(),
            permissions: vec![Permission::Network {
                hosts: vec!["api.test".into(), "other.test".into()],
            }],
        }
    }
}

fn official() -> SourceChain {
    SourceChain::new([Arc::new(OfficialSource) as Arc<dyn crate::transfer::SourceProvider>])
        .unwrap()
}

fn mirrors() -> SourceChain {
    SourceChain::new([
        Arc::new(PrefixMirror::new("https://api.test", "https://mirror.test", 4).unwrap())
            as Arc<dyn crate::transfer::SourceProvider>,
        Arc::new(OfficialSource),
    ])
    .unwrap()
}

fn network_host<T: Transport>(transport: T, sources: SourceChain) -> PluginHost {
    PluginHost::new(
        vec![
            Arc::new(NetworkPlugin),
            fake("test.other", API_VERSION, true),
        ],
        BTreeMap::new(),
    )
    .with_network(transport, sources)
}

#[tokio::test]
async fn grants_match_exact_hosts_and_reject_non_http_and_credentials_before_io() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.call("test.network", |_, ctx| {
        for url in [
            "https://evil.test/",
            "https://api.test.evil.test/",
            "https://sub.api.test/",
            "file:///secret",
            "ftp://api.test/",
            "https://user:password@api.test/",
            "not a URL",
            "https://mirror.test/ok",
        ] {
            assert_eq!(ctx.fetch(url), Err(PluginError::PermissionDenied), "{url}");
        }
        Ok(())
    })
    .await
    .unwrap();
    assert!(net.calls.lock().unwrap().is_empty());
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
    assert_eq!(
        host.call("test.other", |_, ctx| {
            assert_eq!(
                ctx.fetch("https://api.test/ok"),
                Err(PluginError::PermissionDenied)
            );
            Ok(1)
        })
        .await,
        Some(1)
    );
}

#[tokio::test]
async fn authorized_get_uses_mirrors_then_falls_back_and_preserves_http_answers() {
    let net = Scripted::default();
    let host = network_host(net.clone(), mirrors());
    let answer = host
        .call("test.network", |_, ctx| ctx.fetch("https://api.test/ok"))
        .await
        .unwrap();
    assert_eq!(
        answer,
        FetchResponse {
            status: 200,
            body: b"ok".to_vec()
        }
    );
    assert_eq!(net.calls.lock().unwrap()[0].url, "https://mirror.test/ok");
    let answer = host
        .call("test.network", |_, ctx| ctx.fetch("https://api.test/error"))
        .await
        .unwrap();
    assert_eq!(answer.status, 404);
    assert_eq!(answer.body, b"not found");
    assert_eq!(
        net.calls
            .lock()
            .unwrap()
            .iter()
            .map(|call| call.url.as_str())
            .collect::<Vec<_>>(),
        [
            "https://mirror.test/ok",
            "https://mirror.test/error",
            "https://api.test/error"
        ]
    );
}

#[tokio::test]
async fn post_and_credentialed_get_stay_on_the_original_address() {
    let net = Scripted::default();
    let host = network_host(net.clone(), mirrors());
    host.call("test.network", |_, ctx| {
        let answer = ctx.request(FetchRequest::json("https://api.test/ok", b"{}".to_vec()))?;
        assert_eq!(answer.status, 200);
        let mut get = FetchRequest::get("https://api.test/ok");
        get.headers.push(("x-api-key".into(), "test-key".into()));
        ctx.request(get)?;
        Ok(())
    })
    .await
    .unwrap();
    let calls = net.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|call| call.url == "https://api.test/ok"));
    assert_eq!(calls[0].method, HttpMethod::Post);
    assert_eq!(calls[0].body.as_deref(), Some(b"{}".as_slice()));
    assert!(
        calls[0]
            .headers
            .contains(&("content-type".into(), "application/json".into()))
    );
}

#[tokio::test]
async fn every_redirect_is_checked_and_relative_redirects_are_allowed() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.call("test.network", |_, ctx| {
        for url in ["https://api.test/foreign", "https://api.test/downgrade"] {
            assert_eq!(ctx.fetch(url), Err(PluginError::PermissionDenied));
        }
        assert_eq!(ctx.fetch("https://api.test/relative")?.body, b"ok");
        Ok(())
    })
    .await
    .unwrap();
    let calls = net.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert!(
        calls
            .iter()
            .all(|call| call.url.starts_with("https://api.test/"))
    );
}

#[tokio::test]
async fn credentials_are_stripped_on_an_authorized_cross_origin_redirect() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.call("test.network", |_, ctx| {
        let mut request = FetchRequest::get("https://api.test/other");
        request.headers = vec![
            ("x-api-key".into(), "test-key".into()),
            ("accept".into(), "application/json".into()),
        ];
        ctx.request(request)
    })
    .await
    .unwrap();
    let calls = net.calls.lock().unwrap();
    assert_eq!(calls[1].url, "https://other.test/ok");
    assert_eq!(
        calls[1].headers,
        [("accept".into(), "application/json".into())]
    );
}

#[tokio::test]
async fn a_303_redirect_changes_post_to_get_and_drops_the_body() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.call("test.network", |_, ctx| {
        ctx.request(FetchRequest::json(
            "https://api.test/post-redirect",
            b"{}".to_vec(),
        ))
    })
    .await
    .unwrap();
    let calls = net.calls.lock().unwrap();
    assert_eq!(calls[1].method, HttpMethod::Get);
    assert_eq!(calls[1].body, None);
    assert!(calls[1].headers.is_empty());
}

#[tokio::test]
async fn redirect_loops_stop_and_only_the_calling_plugin_fails() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    assert_eq!(
        host.call("test.network", |_, ctx| ctx.fetch("https://api.test/loop"))
            .await,
        None
    );
    assert_eq!(net.calls.lock().unwrap().len(), 11);
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
    assert_eq!(host.call("test.other", |_, _| Ok(7)).await, Some(7));
}

#[tokio::test]
async fn oversized_declared_and_chunked_responses_are_rejected() {
    for path in ["huge-header", "huge-stream"] {
        let host = network_host(Scripted::default(), official());
        host.call("test.network", move |_, ctx| {
            assert!(matches!(ctx.fetch(&format!("https://api.test/{path}")), Err(PluginError::Unavailable(message)) if message.contains("larger than")));
            Ok(())
        }).await.unwrap();
    }
}

#[tokio::test]
async fn routing_headers_and_oversized_requests_fail_before_io() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.call("test.network", |_, ctx| {
        for name in [
            "Host",
            "Connection",
            "content-length",
            "transfer-encoding",
            "proxy-authorization",
            "bad\nname",
        ] {
            let mut request = FetchRequest::get("https://api.test/ok");
            request.headers.push((name.into(), "value".into()));
            assert!(matches!(
                ctx.request(request),
                Err(PluginError::InvalidInput(_))
            ));
        }
        let request = FetchRequest::json("https://api.test/ok", vec![0; 1024 * 1024 + 1]);
        assert!(matches!(
            ctx.request(request),
            Err(PluginError::InvalidInput(_))
        ));
        let mut request = FetchRequest::get("https://api.test/ok");
        request
            .headers
            .push(("accept".into(), "x".repeat(16 * 1024)));
        assert!(matches!(
            ctx.request(request),
            Err(PluginError::InvalidInput(_))
        ));
        Ok(())
    })
    .await
    .unwrap();
    assert!(net.calls.lock().unwrap().is_empty());
}

#[derive(Clone)]
struct Stalled {
    started: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    dropped: Arc<AtomicUsize>,
}

struct DropSignal(Arc<AtomicUsize>);
impl Drop for DropSignal {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

impl Transport for Stalled {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        panic!("unscoped request")
    }
    fn send_no_redirect<'a>(&'a self, _: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            let _guard = DropSignal(self.dropped.clone());
            self.started
                .lock()
                .unwrap()
                .take()
                .unwrap()
                .send(())
                .unwrap();
            std::future::pending().await
        })
    }
}

#[tokio::test]
async fn a_network_timeout_cancels_the_future_and_releases_the_worker() {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut host = network_host(
        Stalled {
            started: Arc::new(Mutex::new(Some(started_tx))),
            dropped: dropped.clone(),
        },
        official(),
    );
    host.list().await;
    host.timeout = Duration::from_millis(50);
    let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
    let call = host.call("test.network", |_, ctx| {
        let result = ctx.fetch("https://api.test/ok");
        finished_tx.send(()).unwrap();
        result
    });
    let (result, ()) = tokio::join!(call, async {
        started_rx.await.unwrap();
    });
    assert_eq!(result, None);
    tokio::time::timeout(Duration::from_secs(1), finished_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
    assert_eq!(host.call("test.other", |_, _| Ok(1)).await, Some(1));
}

#[tokio::test]
async fn disabled_network_plugins_never_send_requests() {
    let net = Scripted::default();
    let host = network_host(net.clone(), official());
    host.set_state(
        "test.network".into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    assert_eq!(
        host.call("test.network", |_, ctx| ctx.fetch("https://api.test/ok"))
            .await,
        None
    );
    assert!(net.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn network_reads_share_an_extended_call_budget() {
    let mut host = network_host(Scripted::default(), official());
    host.list().await;
    host.timeout = Duration::from_millis(5);
    let answer = host
        .call_in_with_timeout("test.network", None, Duration::from_secs(1), |_, ctx| {
            ctx.fetch("https://api.test/slow")
        })
        .await
        .unwrap();
    assert_eq!(answer.body, b"ok");
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
}

#[tokio::test]
async fn service_publishes_mirrors_only_after_successful_persistence() {
    let net = Scripted::default();
    let dir = tempfile::tempdir().unwrap();
    let service =
        crate::LauncherService::open(dir.path(), net.clone(), vec![Arc::new(NetworkPlugin)])
            .unwrap();
    let rules = vec![crate::settings::MirrorRule {
        official_prefix: "https://api.test".into(),
        mirror_prefix: "https://mirror.test".into(),
    }];
    service.set_mirrors(rules, true).await.unwrap();
    service
        .plugins
        .call("test.network", |_, ctx| ctx.fetch("https://api.test/ok"))
        .await
        .unwrap();
    assert_eq!(
        net.calls.lock().unwrap().last().unwrap().url,
        "https://mirror.test/ok"
    );
    std::fs::create_dir(dir.path().join("settings.json.tmp")).unwrap();
    assert!(service.set_mirrors(Vec::new(), false).await.is_err());
    service
        .plugins
        .call("test.network", |_, ctx| ctx.fetch("https://api.test/ok"))
        .await
        .unwrap();
    assert_eq!(
        net.calls.lock().unwrap().last().unwrap().url,
        "https://mirror.test/ok"
    );
}

#[tokio::test]
async fn invalid_saved_mirrors_do_not_prevent_startup_and_can_be_corrected() {
    let net = Scripted::default();
    let dir = tempfile::tempdir().unwrap();
    let settings = serde_json::json!({
        "schema": 1, "mirrors": [{ "official_prefix": "", "mirror_prefix": "https://mirror.test" }],
        "prefer_mirrors": true,
    });
    std::fs::write(dir.path().join("settings.json"), settings.to_string()).unwrap();
    let service =
        crate::LauncherService::open(dir.path(), net.clone(), vec![Arc::new(NetworkPlugin)])
            .unwrap();
    assert!(service.library().await.instances.is_empty());
    service.plugins.call("test.network", |_, ctx| {
        assert!(matches!(ctx.fetch("https://api.test/ok"), Err(PluginError::Unavailable(message)) if message.contains("mirror")));
        Ok(())
    }).await.unwrap();
    assert!(net.calls.lock().unwrap().is_empty());
    service.set_mirrors(Vec::new(), false).await.unwrap();
    service
        .plugins
        .call("test.network", |_, ctx| ctx.fetch("https://api.test/ok"))
        .await
        .unwrap();
    assert_eq!(net.calls.lock().unwrap().len(), 1);
}
