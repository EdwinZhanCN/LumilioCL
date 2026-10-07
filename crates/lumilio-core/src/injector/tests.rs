use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::{Arc, Mutex};

use super::*;
use crate::transfer::{
    OfficialSource, PrefixMirror, SourceProvider, TransportError, TransportFuture,
    TransportResponse,
};

const JAR_URL: &str = "https://example.org/artifact/authlib-injector-1.2.5.jar";

/// A jar the way authlib-injector builds it: a manifest that names it.
fn jar(title: &str, build: u32) -> Vec<u8> {
    let mut out = io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);
        writer
            .start_file::<_, ()>(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        write!(
            writer,
            "Manifest-Version: 1.0\r\nImplementation-Title: {title}\r\nImplementation-Version: 1.2.{build}\r\nBuild-Number: {build}\r\n"
        )
        .unwrap();
        writer.finish().unwrap();
    }
    out.into_inner()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Default)]
struct Web {
    documents: Mutex<BTreeMap<String, Vec<u8>>>,
    asked: Mutex<Vec<String>>,
}

impl Web {
    fn serve(&self, url: &str, bytes: Vec<u8>) {
        self.documents.lock().unwrap().insert(url.to_owned(), bytes);
    }

    fn index(&self, build: u32, checksum: Option<String>, jar_url: &str) {
        let checksums = checksum.map_or_else(
            || serde_json::json!({}),
            |sha| serde_json::json!({ "sha256": sha }),
        );
        self.serve(
            LATEST_URL,
            serde_json::json!({
                "build_number": build,
                "version": format!("1.2.{build}"),
                "download_url": jar_url,
                "checksums": checksums,
            })
            .to_string()
            .into_bytes(),
        );
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Transport for Web {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            self.asked.lock().unwrap().push(source.to_owned());
            match self.documents.lock().unwrap().get(source) {
                Some(bytes) => Ok(TransportResponse::from_bytes(200, bytes.clone())),
                None => Ok(TransportResponse::from_bytes(404, Vec::new())),
            }
        })
    }
}

fn official() -> SourceChain {
    SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap()
}

#[tokio::test]
async fn the_first_use_downloads_a_checked_jar_and_later_uses_the_file() {
    let (web, dir) = (Web::default(), tempfile::tempdir().unwrap());
    let bytes = jar("authlib-injector", 54);
    web.index(54, Some(sha256(&bytes)), JAR_URL);
    web.serve(JAR_URL, bytes);

    let found = ensure(&web, &official(), dir.path()).await.unwrap();
    assert_eq!((found.build_number, found.version.as_str()), (54, "1.2.54"));
    assert!(found.path.starts_with(dir.path()));
    assert_eq!(web.asked().len(), 2);

    // Nothing is asked of the network again while the file is there.
    let again = ensure(&web, &official(), dir.path()).await.unwrap();
    assert_eq!(again, found);
    assert_eq!(web.asked().len(), 2);
}

#[tokio::test]
async fn injector_metadata_falls_back_from_a_missing_checksum() {
    let web = Web::default();
    let mirror = "https://mirror.test/latest.json";
    web.index(54, Some("a".repeat(64)), JAR_URL);
    web.serve(mirror, serde_json::json!({"build_number":54,"version":"1.2.54","download_url":JAR_URL,"checksums":{}}).to_string().into_bytes());
    let chain = SourceChain::new([
        Arc::new(PrefixMirror::new(LATEST_URL, mirror, 16).unwrap()) as Arc<dyn SourceProvider>,
        Arc::new(OfficialSource),
    ])
    .unwrap();
    assert_eq!(latest(&web, &chain).await.unwrap().build_number, 54);
    assert_eq!(web.asked(), [mirror, LATEST_URL]);
}

#[tokio::test]
async fn a_jar_that_does_not_match_or_is_not_the_agent_is_never_kept() {
    let dir = tempfile::tempdir().unwrap();

    let tampered = Web::default();
    tampered.index(54, Some(sha256(b"what the index promised")), JAR_URL);
    tampered.serve(JAR_URL, jar("authlib-injector", 54));
    assert!(matches!(
        ensure(&tampered, &official(), dir.path()).await,
        Err(InjectorError::Unavailable(why)) if why.contains("checksum")
    ));

    let other = Web::default();
    let not_it = jar("something-else", 54);
    other.index(54, Some(sha256(&not_it)), JAR_URL);
    other.serve(JAR_URL, not_it);
    assert!(matches!(
        ensure(&other, &official(), dir.path()).await,
        Err(InjectorError::Unavailable(why)) if why.contains("not authlib-injector")
    ));

    let unsigned = Web::default();
    unsigned.index(54, None, JAR_URL);
    unsigned.serve(JAR_URL, jar("authlib-injector", 54));
    assert!(ensure(&unsigned, &official(), dir.path()).await.is_err());

    assert!(installed(dir.path()).is_none());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "no leftovers");
}

#[tokio::test]
async fn refreshing_replaces_only_with_a_newer_build_and_a_failure_keeps_the_old_one() {
    let (web, dir) = (Web::default(), tempfile::tempdir().unwrap());
    let old = jar("authlib-injector", 54);
    web.index(54, Some(sha256(&old)), JAR_URL);
    web.serve(JAR_URL, old);
    ensure(&web, &official(), dir.path()).await.unwrap();

    // Same build: nothing downloaded.
    let before = web.asked().len();
    refresh(&web, &official(), dir.path()).await.unwrap();
    assert_eq!(web.asked().len(), before + 1, "only the index");

    let newer = jar("authlib-injector", 55);
    web.index(55, Some(sha256(&newer)), JAR_URL);
    web.serve(JAR_URL, newer);
    assert_eq!(
        refresh(&web, &official(), dir.path())
            .await
            .unwrap()
            .build_number,
        55
    );
    assert_eq!(installed(dir.path()).unwrap().build_number, 55);

    // A broken index leaves the working jar alone.
    web.serve(LATEST_URL, b"nonsense".to_vec());
    assert!(refresh(&web, &official(), dir.path()).await.is_err());
    assert_eq!(installed(dir.path()).unwrap().build_number, 55);
}

#[tokio::test]
async fn mirrors_are_tried_in_the_order_the_chain_says() {
    let (web, dir) = (Web::default(), tempfile::tempdir().unwrap());
    let bytes = jar("authlib-injector", 54);
    let mirror = PrefixMirror::new(
        "https://authlib-injector.yushi.moe",
        "https://mirror.example/authlib-injector",
        4,
    )
    .unwrap();
    let chain = SourceChain::new([
        Arc::new(mirror) as Arc<dyn SourceProvider>,
        Arc::new(OfficialSource),
    ])
    .unwrap();
    // Only the mirror has the index; the jar's address is on the same host.
    let jar_url = "https://authlib-injector.yushi.moe/artifact/54/authlib-injector-1.2.54.jar";
    web.index(54, Some(sha256(&bytes)), jar_url);
    let index = web.documents.lock().unwrap()[LATEST_URL].clone();
    web.serve(
        "https://mirror.example/authlib-injector/artifact/latest.json",
        index,
    );
    web.serve(
        "https://mirror.example/authlib-injector/artifact/54/authlib-injector-1.2.54.jar",
        bytes,
    );
    ensure(&web, &chain, dir.path()).await.unwrap();
    assert!(
        web.asked()
            .iter()
            .all(|url| url.starts_with("https://mirror.example"))
    );
}

#[test]
fn the_jvm_arguments_load_the_agent_and_carry_the_servers_metadata() {
    let arguments = jvm_arguments(
        Path::new("/data/authlib-injector.jar"),
        "https://skin.example/api/",
        Some("{\"meta\":{}}"),
    );
    assert_eq!(
        arguments[0],
        "-javaagent:/data/authlib-injector.jar=https://skin.example/api/"
    );
    assert_eq!(arguments[1], "-Dauthlibinjector.side=client");
    assert_eq!(
        arguments[2],
        "-Dauthlibinjector.yggdrasil.prefetched=eyJtZXRhIjp7fX0="
    );
    assert_eq!(
        jvm_arguments(Path::new("/j"), "http://localhost:1", None).len(),
        2
    );
}

#[tokio::test]
async fn an_unreachable_index_is_unavailable_not_a_panic() {
    struct Down;
    impl Transport for Down {
        fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
            Box::pin(async { Err(TransportError::transient("offline")) })
        }
    }
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(
        ensure(&Down, &official(), dir.path()).await,
        Err(InjectorError::Unavailable(_))
    ));
}
