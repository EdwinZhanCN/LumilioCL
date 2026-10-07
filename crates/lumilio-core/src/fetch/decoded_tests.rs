use super::*;
use crate::catalog::{OFFICIAL_CATALOG_URL, VersionCatalog};
use crate::loader;
use crate::transfer::{
    OfficialSource, PrefixMirror, SourceChain, TransportFuture, TransportResponse,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Web {
    documents: BTreeMap<String, Vec<u8>>,
    asked: Mutex<Vec<String>>,
}

impl Transport for Web {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        self.asked.lock().unwrap().push(source.to_owned());
        let bytes = self.documents.get(source).cloned().unwrap();
        Box::pin(async move {
            Ok(TransportResponse::new(
                200,
                None,
                Box::pin(futures_util::stream::once(async move { Ok(bytes) })),
            ))
        })
    }
}

#[tokio::test]
async fn catalog_falls_back_from_http_200_with_bad_json_structure_or_encoding() {
    let mirror = "https://mirror.test/mc/game/version_manifest_v2.json";
    for bad in [
        b"<html>unavailable</html>".to_vec(),
        b"{\"error\":\"unavailable\"}".to_vec(),
        vec![0xff],
    ] {
        let web = Web {
            documents: BTreeMap::from([
                (mirror.to_owned(), bad),
                (OFFICIAL_CATALOG_URL.to_owned(), br#"{"latest":{"release":"1.21","snapshot":"1.21"},"versions":[{"id":"1.21","type":"release","url":"https://official.test/1.21.json","releaseTime":"2024-01-01"}]}"#.to_vec()),
            ]),
            ..Web::default()
        };
        let sources = SourceChain::new(vec![
            Arc::new(
                PrefixMirror::new(
                    "https://piston-meta.mojang.com/",
                    "https://mirror.test/",
                    16,
                )
                .unwrap(),
            ) as Arc<dyn crate::transfer::SourceProvider>,
            Arc::new(OfficialSource),
        ])
        .unwrap();
        let catalog = VersionCatalog::fetch(&web, &sources).await.unwrap();
        assert_eq!(catalog.entries()[0].id(), "1.21");
        assert_eq!(*web.asked.lock().unwrap(), [mirror, OFFICIAL_CATALOG_URL]);
    }
}

#[tokio::test]
async fn loader_falls_back_from_wrong_shape_but_accepts_a_valid_empty_list() {
    for (first, expected_requests) in [
        (
            br#"{"error":"unavailable"}"#.as_slice(),
            vec!["mirror", "official"],
        ),
        (b"[]".as_slice(), vec!["mirror"]),
    ] {
        let web = Web {
            documents: BTreeMap::from([
                ("mirror".to_owned(), first.to_vec()),
                ("official".to_owned(), b"[]".to_vec()),
            ]),
            ..Web::default()
        };
        assert!(
            loader::fetch_versions(&web, &["mirror".to_owned(), "official".to_owned()])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(*web.asked.lock().unwrap(), expected_requests);
    }
}

#[tokio::test]
async fn decoding_failure_retains_each_source_reason() {
    let web = Web {
        documents: BTreeMap::from([
            ("a".to_owned(), b"first".to_vec()),
            ("b".to_owned(), b"second".to_vec()),
        ]),
        ..Web::default()
    };
    let error = fetch_decoded(&web, &["a", "b"], |bytes| {
        Err::<(), _>(String::from_utf8(bytes).unwrap())
    })
    .await
    .unwrap_err();
    assert_eq!(
        error,
        FetchError::Exhausted(vec![
            ("a".into(), "first".into()),
            ("b".into(), "second".into())
        ])
    );
    assert_eq!(
        fetch_decoded(&web, &[] as &[&str], Ok::<_, std::convert::Infallible>).await,
        Err(FetchError::NoSources)
    );
}

#[test]
fn forge_metadata_rejects_wrong_shape_and_accepts_empty_lists() {
    assert!(
        crate::forge_meta::neoforge_versions(br#"{"error":"unavailable"}"#, None, "1.21.1")
            .is_err()
    );
    assert!(
        crate::forge_meta::neoforge_versions(br#"{"versions":[]}"#, None, "1.21.1")
            .unwrap()
            .is_empty()
    );
    assert!(
        crate::forge_meta::forge_versions(b"{}", Some(br#"{"error":"unavailable"}"#), "1.21.1")
            .is_err()
    );
    assert!(
        crate::forge_meta::forge_versions(b"{}", Some(br#"{"promos":{}}"#), "1.21.1")
            .unwrap()
            .is_empty()
    );
}
