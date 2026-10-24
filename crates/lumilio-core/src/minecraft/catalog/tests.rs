use std::sync::Arc;

use super::fetch_catalog;
use crate::transfer::{FileTransport, OfficialSource, SourceChain, SourceProvider, TransferEngine};

const DOC: &str = r#"{
        "latest": {"release": "1.21.1", "snapshot": "24w40a"},
        "versions": [
            {"id": "1.20.4", "type": "release", "url": "https://x/1.20.4.json",
             "releaseTime": "2023-12-07T12:56:20+00:00", "sha1": "aa"},
            {"id": "1.21.1", "type": "release", "url": "https://x/1.21.1.json",
             "releaseTime": "2024-08-08T12:24:45+00:00"}
        ]
    }"#;

#[tokio::test]
async fn fetches_through_a_local_source() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.json");
    std::fs::write(&path, DOC).unwrap();
    // A mirror that rewrites the official address onto the local file.
    struct Local(String);
    impl SourceProvider for Local {
        fn candidates(&self, _: &str) -> Vec<String> {
            vec![self.0.clone()]
        }
    }
    let url = url::Url::from_file_path(&path).unwrap().to_string();
    let chain = SourceChain::new([
        Arc::new(Local(url)) as Arc<dyn SourceProvider>,
        Arc::new(OfficialSource),
    ])
    .unwrap();
    let catalog = fetch_catalog(&TransferEngine::new(FileTransport, 1).unwrap(), &chain)
        .await
        .unwrap();
    assert_eq!(catalog.entries().len(), 2);
}
