use std::sync::Mutex as StdMutex;

use super::*;
use crate::transfer::{HttpRequest, TransportFuture, TransportResponse};

struct Scripted {
    statuses: StdMutex<Vec<u16>>,
    calls: StdMutex<usize>,
    answer: serde_json::Value,
}
impl Transport for Scripted {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        unreachable!()
    }
    fn send<'a>(&'a self, _: HttpRequest) -> TransportFuture<'a> {
        Box::pin(async move {
            *self.calls.lock().unwrap() += 1;
            let status = self.statuses.lock().unwrap().pop().unwrap_or(200);
            Ok(TransportResponse::from_bytes(
                status,
                serde_json::to_vec(&self.answer).unwrap(),
            ))
        })
    }
}

#[tokio::test]
async fn sqlite_ttl_is_seven_days_without_read_renewal_and_survives_reopen() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("translation.sqlite");
    let cache = cache::Cache::new(path.clone());
    cache
        .put("a".into(), "译文".into(), 100, cache.generation())
        .await;
    assert_eq!(cache.get("a".into(), 101).await.as_deref(), Some("译文"));
    let reopened = cache::Cache::new(path);
    assert_eq!(
        reopened
            .get("a".into(), 100 + 7 * 24 * 60 * 60 - 1)
            .await
            .as_deref(),
        Some("译文")
    );
    assert_eq!(reopened.get("a".into(), 100 + 7 * 24 * 60 * 60).await, None);
    let generation = cache.generation();
    cache.clear().await.unwrap();
    cache.put("a".into(), "late".into(), 102, generation).await;
    assert_eq!(cache.get("a".into(), 103).await, None);
}

#[test]
fn structured_html_and_markdown_preserve_links_attributes_and_code() {
    let source = "# About\n\n**Hello** [world](https://example.com/a?q=1)\n\n```sh\njava -jar server.jar\n```\n\n<p class=\"note\">Hello <em>world</em> <img src=\"a.png\"></p>";
    let mut doc = document::Document::parse(source, TextFormat::Markdown).unwrap();
    let texts = doc.texts();
    assert!(!texts.iter().any(|text| text.contains("java")));
    let result = doc
        .replace(&vec!["中文 <script>".into(); texts.len()])
        .unwrap();
    assert!(result.contains("<strong>"));
    assert!(result.contains("href=\"https://example.com/a?q=1\""));
    assert!(result.contains("class=\"note\""));
    assert!(result.contains("src=\"a.png\""));
    assert!(result.contains("java -jar server.jar"));
    assert!(result.contains("&lt;script&gt;"));
    assert!(!result.contains("<script>"));
    assert!(document::safe_summary(
        "<p>Hello <b>world</b></p>",
        "<p>你好 <b>世界</b></p>"
    ));
    assert!(!document::safe_summary(
        "<p>Hello <b>world</b></p>",
        "你好世界"
    ));
}

#[test]
fn html_bodies_are_read_as_html_not_as_markdown() {
    // CurseForge writes HTML. Read as Markdown, the indented paragraph after
    // the blank line becomes a code block and is never translated.
    let source = "<div>\n\n    <p>Indented <a href=\"https://example.com\">link</a></p>\n</div>";
    let as_markdown = document::Document::parse(source, TextFormat::Markdown)
        .unwrap()
        .texts();
    assert!(!as_markdown.iter().any(|text| text == "Indented "));
    let mut doc = document::Document::parse(source, TextFormat::Html).unwrap();
    let texts = doc.texts();
    assert_eq!(texts, ["Indented ", "link"]);
    let result = doc.replace(&["缩进 ".into(), "链接".into()]).unwrap();
    assert!(result.contains("<p>缩进 <a href=\"https://example.com\">链接</a></p>"));
    assert!(!result.contains("<pre>"));
}

#[tokio::test(start_paused = true)]
async fn retries_transient_statuses_twice_but_not_permanent_or_protocol_errors() {
    let transport = Scripted {
        statuses: StdMutex::new(vec![200, 503, 429]),
        calls: StdMutex::new(0),
        answer: serde_json::json!([{"translations":[{"text":"你好","to":"zh-Hans"}]}]),
    };
    assert_eq!(
        protocol::edge(&transport, &["Hello".into()]).await.unwrap(),
        ["你好"]
    );
    assert_eq!(*transport.calls.lock().unwrap(), 3);
    *transport.statuses.lock().unwrap() = vec![403];
    assert_eq!(
        protocol::edge(&transport, &["Hello".into()]).await,
        Err(TranslationError::Http(403))
    );
    assert_eq!(*transport.calls.lock().unwrap(), 4);
    assert_eq!(
        protocol::edge(&transport, &["one".into(), "two".into()]).await,
        Err(TranslationError::Protocol)
    );
    assert_eq!(*transport.calls.lock().unwrap(), 5);
}

#[test]
fn cache_identity_isolates_sources_content_and_providers() {
    let first = key("a", "1", "body", "edge", "hello");
    assert_ne!(first, key("b", "1", "body", "edge", "hello"));
    assert_ne!(first, key("a", "1", "body", "edge", "changed"));
    assert_ne!(first, key("a", "1", "body", "mcim", "hello"));
}

#[tokio::test]
async fn body_cache_reuses_success_across_service_restart_and_clear_requests_again() {
    let root = tempfile::tempdir().unwrap();
    let transport = Scripted {
        statuses: StdMutex::new(vec![]),
        calls: StdMutex::new(0),
        answer: serde_json::json!([{"translations":[{"text":"你好","to":"zh-Hans"}]}]),
    };
    let service = TranslationService::new(root.path().to_owned());
    let first = service
        .body(&transport, "source", "1", "Hello", TextFormat::Markdown)
        .await
        .unwrap();
    assert!(first.text.contains("你好"));
    assert!(!first.partial);
    let reopened = TranslationService::new(root.path().to_owned());
    assert_eq!(
        reopened
            .body(&transport, "source", "1", "Hello", TextFormat::Markdown)
            .await
            .unwrap(),
        first
    );
    assert_eq!(*transport.calls.lock().unwrap(), 1);
    reopened.clear().await.unwrap();
    reopened
        .body(&transport, "source", "1", "Hello", TextFormat::Markdown)
        .await
        .unwrap();
    assert_eq!(*transport.calls.lock().unwrap(), 2);
}

#[tokio::test]
async fn mcim_batch_is_correlated_by_id_and_rejects_stale_or_structurally_different_text() {
    let root = tempfile::tempdir().unwrap();
    let transport = Scripted {
        statuses: StdMutex::new(vec![]),
        calls: StdMutex::new(0),
        answer: serde_json::json!([
            {"project_id":"b","original":"Old text","translated":"旧文本"},
            {"project_id":"a","original":"Hello","translated":"你好"},
            {"project_id":"html","original":"<p>Hello <b>world</b></p>","translated":"你好世界"}
        ]),
    };
    let service = TranslationService::new(root.path().to_owned());
    let items = vec![
        ("a".to_owned(), "Hello".to_owned()),
        ("b".to_owned(), "New text".to_owned()),
        ("missing".to_owned(), "Unknown".to_owned()),
        ("html".to_owned(), "<p>Hello <b>world</b></p>".to_owned()),
    ];
    assert_eq!(
        service
            .summaries(
                &transport,
                Some(Catalog::Modrinth),
                "lumilio.modrinth",
                &items
            )
            .await,
        BTreeMap::from([("a".to_owned(), "你好".to_owned())])
    );
    assert!(
        service
            .summaries(&transport, None, "other", &items)
            .await
            .is_empty()
    );
    // MCIM, then the fallback for the three it could not answer.
    assert_eq!(*transport.calls.lock().unwrap(), 2);
}

/// Answers by host: what each summary service has, and whether it is up.
struct Services {
    mcim: Option<serde_json::Value>,
    lumilio: serde_json::Value,
    sent: StdMutex<Vec<(String, serde_json::Value)>>,
}
impl Transport for Services {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        unreachable!()
    }
    fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        let body = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
        self.sent.lock().unwrap().push((request.url.clone(), body));
        let answer = if request.url.starts_with("https://mod.mcimirror.top/") {
            self.mcim.clone()
        } else {
            assert!(
                request
                    .url
                    .starts_with("https://launcher.lumilio.org/translate/")
            );
            Some(self.lumilio.clone())
        };
        Box::pin(async move {
            Ok(match answer {
                Some(answer) => {
                    TransportResponse::from_bytes(200, serde_json::to_vec(&answer).unwrap())
                }
                // Not retried: the request fails as a refused service would.
                None => TransportResponse::from_bytes(403, Vec::new()),
            })
        })
    }
}

#[tokio::test]
async fn the_fallback_only_hears_of_what_mcim_could_not_answer() {
    let root = tempfile::tempdir().unwrap();
    let transport = Services {
        mcim: Some(serde_json::json!([
            {"project_id":"a","original":"Hello","translated":"你好"},
            {"project_id":"b","original":"Old text","translated":"旧文本"}
        ])),
        lumilio: serde_json::json!([
            {"project_id":"b","original":"New text","translated":"新文本"},
            {"project_id":"c","original":"Third","translated":"第三"}
        ]),
        sent: StdMutex::new(Vec::new()),
    };
    let service = TranslationService::new(root.path().to_owned());
    let items = vec![
        ("a".to_owned(), "Hello".to_owned()),
        ("b".to_owned(), "New text".to_owned()),
        ("c".to_owned(), "Third".to_owned()),
    ];
    let expected = BTreeMap::from([
        ("a".to_owned(), "你好".to_owned()),
        ("b".to_owned(), "新文本".to_owned()),
        ("c".to_owned(), "第三".to_owned()),
    ]);
    let ask = || {
        service.summaries(
            &transport,
            Some(Catalog::Modrinth),
            "lumilio.modrinth",
            &items,
        )
    };
    assert_eq!(ask().await, expected);
    assert_eq!(
        *transport.sent.lock().unwrap(),
        [
            (
                "https://mod.mcimirror.top/translate/modrinth".to_owned(),
                serde_json::json!({"project_ids": ["a", "b", "c"]})
            ),
            (
                "https://launcher.lumilio.org/translate/modrinth".to_owned(),
                serde_json::json!({"project_ids": ["b", "c"]})
            ),
        ]
    );
    // Both answers are kept; c is known missing at MCIM for a while.
    transport.sent.lock().unwrap().clear();
    assert_eq!(ask().await, expected);
    assert_eq!(
        *transport.sent.lock().unwrap(),
        [(
            "https://mod.mcimirror.top/translate/modrinth".to_owned(),
            serde_json::json!({"project_ids": ["b"]})
        )]
    );
}

#[tokio::test]
async fn with_mcim_unreachable_every_summary_is_asked_of_the_fallback() {
    let root = tempfile::tempdir().unwrap();
    let transport = Services {
        mcim: None,
        lumilio: serde_json::json!([{"modid": 238222, "original": "View items", "translated": "查看物品"}]),
        sent: StdMutex::new(Vec::new()),
    };
    let service = TranslationService::new(root.path().to_owned());
    let items = vec![("238222".to_owned(), "View items".to_owned())];
    assert_eq!(
        service
            .summaries(
                &transport,
                Some(Catalog::CurseForge),
                "lumilio.curseforge",
                &items
            )
            .await,
        BTreeMap::from([("238222".to_owned(), "查看物品".to_owned())])
    );
    let sent = transport.sent.lock().unwrap();
    assert_eq!(sent.len(), 2);
    assert_eq!(
        sent[1].0,
        "https://launcher.lumilio.org/translate/curseforge"
    );
    assert_eq!(sent[1].1, serde_json::json!({"modids": [238222]}));
}

#[tokio::test]
async fn curseforge_summaries_ask_by_numeric_mod_id() {
    struct Recording {
        sent: StdMutex<Vec<HttpRequest>>,
    }
    impl Transport for Recording {
        fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
            unreachable!()
        }
        fn send<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
            self.sent.lock().unwrap().push(request);
            Box::pin(async {
                Ok(TransportResponse::from_bytes(
                    200,
                    serde_json::to_vec(&serde_json::json!([
                        {"modid": 238222, "original": "View items", "translated": "查看物品"}
                    ]))
                    .unwrap(),
                ))
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let transport = Recording {
        sent: StdMutex::new(Vec::new()),
    };
    let service = TranslationService::new(root.path().to_owned());
    let items = vec![("238222".to_owned(), "View items".to_owned())];
    assert_eq!(
        service
            .summaries(
                &transport,
                Some(Catalog::CurseForge),
                "lumilio.curseforge",
                &items
            )
            .await,
        BTreeMap::from([("238222".to_owned(), "查看物品".to_owned())])
    );
    let sent = transport.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].url,
        "https://mod.mcimirror.top/translate/curseforge"
    );
    let body: serde_json::Value = serde_json::from_slice(sent[0].body.as_ref().unwrap()).unwrap();
    assert_eq!(body, serde_json::json!({"modids": [238222]}));
}

#[test]
fn unicode_segmentation_respects_utf16_limits_without_splitting_emoji() {
    let original = "🎮中文".repeat(4000);
    let doc = document::Document::parse(&original, TextFormat::Markdown).unwrap();
    let texts = doc.texts();
    assert_eq!(texts.concat(), original);
    for batch in document::batches(&texts) {
        assert!(
            batch
                .iter()
                .map(|text| text.encode_utf16().count())
                .sum::<usize>()
                <= 4000
        );
    }
}

#[tokio::test]
async fn partial_failure_keeps_original_and_retry_only_requests_failed_segments() {
    let root = tempfile::tempdir().unwrap();
    let transport = Scripted {
        statuses: StdMutex::new(vec![403, 200]),
        calls: StdMutex::new(0),
        answer: serde_json::json!([{"translations":[{"text":"中文","to":"zh-Hans"}]}]),
    };
    let service = TranslationService::new(root.path().to_owned());
    let original = format!("{}\n\nb", "a".repeat(4000));
    let partial = service
        .body(&transport, "source", "1", &original, TextFormat::Markdown)
        .await
        .unwrap();
    assert!(partial.partial && partial.text.contains("<p>b</p>"));
    assert_eq!(*transport.calls.lock().unwrap(), 2);
    let retried = service
        .body(&transport, "source", "1", &original, TextFormat::Markdown)
        .await
        .unwrap();
    assert!(!retried.partial && !retried.text.contains("<p>b</p>"));
    assert_eq!(
        *transport.calls.lock().unwrap(),
        3,
        "successful segment remains cached"
    );
}

#[tokio::test]
async fn invalidating_preferences_cancels_an_inflight_translation() {
    struct Pending {
        entered: Arc<tokio::sync::Notify>,
    }
    impl Transport for Pending {
        fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
            unreachable!()
        }
        fn send<'a>(&'a self, _: HttpRequest) -> TransportFuture<'a> {
            Box::pin(async move {
                self.entered.notify_one();
                std::future::pending().await
            })
        }
    }
    let root = tempfile::tempdir().unwrap();
    let service = Arc::new(TranslationService::new(root.path().to_owned()));
    let entered = Arc::new(tokio::sync::Notify::new());
    let job_service = service.clone();
    let job_entered = entered.clone();
    let task = tokio::spawn(async move {
        job_service
            .body(
                &Pending {
                    entered: job_entered,
                },
                "source",
                "1",
                "Hello",
                TextFormat::Markdown,
            )
            .await
    });
    entered.notified().await;
    service.invalidate();
    assert_eq!(task.await.unwrap(), Err(TranslationError::Disabled));
}
