use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use lumilio_block_assets::write_bundle;
use sha2::{Digest, Sha256};

use super::super::{
    AssetPlan, BundleIndex, BundleStore, LocalCache, PlannedSource, SchematicAssetsError, load,
};
use super::rules::index_json;
use super::{baked, client_jar};
use crate::activity::CancellationToken;
use crate::transfer::{
    OfficialSource, SourceChain, TransferEngine, Transport, TransportError, TransportFuture,
    TransportResponse,
};

const URL: &str = "https://assets.test/block-assets/1/";

/// Answers the addresses it was given; anything else fails like no network.
#[derive(Clone, Default)]
struct Served {
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl Served {
    fn serve(&self, name: &str, bytes: Vec<u8>) {
        self.files
            .lock()
            .unwrap()
            .insert(format!("{URL}{name}"), bytes);
    }
}

impl Transport for Served {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        let found = self.files.lock().unwrap().get(source).cloned();
        Box::pin(async move {
            found
                .map(|bytes| TransportResponse::from_bytes(200, bytes))
                .ok_or_else(|| TransportError::permanent("no network"))
        })
    }
}

fn chain() -> SourceChain {
    SourceChain::new([Arc::new(OfficialSource) as _]).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A bundle holding a 1.20.1 pack, and its index.
fn published(dir: &std::path::Path) -> (Vec<u8>, BundleIndex) {
    let jar = client_jar(&dir.join("1.20.1.jar"), "1.20.1", "stone");
    let pack = baked(&jar);
    let bytes = write_bundle([("1.20.1", &pack)]).unwrap();
    let index =
        BundleIndex::parse(index_json(&sha256(&bytes), bytes.len() as u64).as_bytes()).unwrap();
    (bytes, index)
}

#[tokio::test]
async fn the_index_is_fetched_once_and_kept() {
    let dir = tempfile::tempdir().unwrap();
    let (bytes, _) = published(dir.path());
    let net = Served::default();
    net.serve(
        "blocks.json",
        index_json(&sha256(&bytes), bytes.len() as u64).into_bytes(),
    );
    let store = BundleStore::new(&dir.path().join("cache")).with_url(URL);
    assert!(store.cached_index().is_none());
    let fetched = store.fetch_index(&net, &chain()).await.unwrap();
    assert_eq!(store.cached_index(), Some(fetched));

    let offline = BundleStore::new(&dir.path().join("elsewhere")).with_url(URL);
    assert!(matches!(
        offline.fetch_index(&Served::default(), &chain()).await,
        Err(SchematicAssetsError::Download(_))
    ));
}

#[tokio::test]
async fn a_downloaded_bundle_is_checked_and_sliced_by_version() {
    let dir = tempfile::tempdir().unwrap();
    let (bytes, index) = published(dir.path());
    let net = Served::default();
    net.serve("blocks.dat.z", bytes);
    let store = BundleStore::new(&dir.path().join("cache")).with_url(URL);
    let file = index.file_of(index.version("1.20.1").unwrap());
    let engine = TransferEngine::new(net.clone(), 1).unwrap();
    store
        .download(&engine, &chain(), &file, CancellationToken::new())
        .await
        .unwrap();
    assert!(store.has(&file));
    let plan = AssetPlan {
        sources: vec![PlannedSource::Bundle {
            version: "1.20.1".into(),
        }],
        ..AssetPlan::default()
    };
    let cache = LocalCache::new(&dir.path().join("cache"));
    let baked = load(&plan, &cache, Some((&store, &file))).unwrap();
    assert!(
        baked
            .pack
            .assets
            .blockstates
            .contains_key("minecraft:stone")
    );
}

#[tokio::test]
async fn a_bundle_that_does_not_match_its_sha256_is_refused_and_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let (bytes, index) = published(dir.path());
    let file = index.file_of(index.version("1.20.1").unwrap());
    let store = BundleStore::new(&dir.path().join("cache")).with_url(URL);

    // Served with other bytes of the same size: refused, nothing kept, and
    // a damaged copy left from before is gone too.
    let mut tampered = bytes.clone();
    tampered[0] ^= 0xff;
    let net = Served::default();
    net.serve("blocks.dat.z", tampered.clone());
    std::fs::create_dir_all(store.path_of(&file).parent().unwrap()).unwrap();
    std::fs::write(store.path_of(&file), &tampered).unwrap();
    let engine = TransferEngine::new(net, 1).unwrap();
    let result = store
        .download(&engine, &chain(), &file, CancellationToken::new())
        .await;
    assert!(matches!(result, Err(SchematicAssetsError::Download(_))));
    assert!(!store.path_of(&file).exists());
    let leftovers: Vec<_> = std::fs::read_dir(store.path_of(&file).parent().unwrap())
        .unwrap()
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // A file that changed on disk after download fails its check on read.
    std::fs::write(store.path_of(&file), &tampered).unwrap();
    assert!(matches!(
        store.slice(&file, "1.20.1"),
        Err(SchematicAssetsError::Mismatch { .. })
    ));
    assert!(!store.path_of(&file).exists());
}

#[test]
fn overlays_are_laid_over_a_bundle_pack() {
    let dir = tempfile::tempdir().unwrap();
    let (bytes, index) = published(dir.path());
    let store = BundleStore::new(&dir.path().join("cache")).with_url(URL);
    let file = index.file_of(index.version("1.20.1").unwrap());
    std::fs::create_dir_all(store.path_of(&file).parent().unwrap()).unwrap();
    std::fs::write(store.path_of(&file), bytes).unwrap();
    let pack = dir.path().join("pack");
    std::fs::create_dir_all(pack.join("assets/minecraft/blockstates")).unwrap();
    std::fs::write(
        pack.join("assets/minecraft/blockstates/stone.json"),
        br#"{"variants":{"":{"model":"minecraft:block/stone","y":90}}}"#,
    )
    .unwrap();
    let plan = AssetPlan {
        sources: vec![
            PlannedSource::Bundle {
                version: "1.20.1".into(),
            },
            PlannedSource::Overlay(pack),
        ],
        ..AssetPlan::default()
    };
    let cache = LocalCache::new(&dir.path().join("cache"));
    let first = load(&plan, &cache, Some((&store, &file))).unwrap();
    let base = store.slice(&file, "1.20.1").unwrap();
    assert_ne!(
        first.pack.assets.blockstates["minecraft:stone"],
        base.assets.blockstates["minecraft:stone"]
    );
    assert!(
        first
            .pack
            .assets
            .models
            .contains_key("minecraft:block/stone")
    );
    assert!(!first.from_cache);
    assert!(
        load(&plan, &cache, Some((&store, &file)))
            .unwrap()
            .from_cache
    );
}

/// The review hook points the bundle at another folder address; an address
/// that is not an http(s) folder leaves the published one.
#[test]
fn the_bundle_address_takes_only_a_folder_from_the_hook() {
    use super::super::BUNDLE_URL;
    use super::super::bundle::bundle_url;
    let local = "http://127.0.0.1:8765/block-assets/1/";
    assert_eq!(bundle_url(Some(local.to_owned())), local);
    assert_eq!(bundle_url(None), BUNDLE_URL);
    for wrong in ["http://127.0.0.1:8765/blocks.json", "file:///tmp/", ""] {
        assert_eq!(bundle_url(Some(wrong.to_owned())), BUNDLE_URL, "{wrong}");
    }
}
