use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use semver::Version;
use sha2::{Digest as _, Sha256};

use crate::client::{CheckResult, UpdateClient, UpdateRelease};
use crate::manifest::AssetTarget;

/// Answers every request under `/site/` with 404 and serves `files` under
/// `/github/`, recording each path it was asked.
fn serve(files: Vec<(String, Vec<u8>)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap_or(0);
            let head = String::from_utf8_lossy(&buffer[..read]).into_owned();
            let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
            seen.lock().unwrap().push(path.clone());
            let found = files
                .iter()
                .find(|(name, _)| path == format!("/github{name}"));
            let (status, body) = match found {
                Some((_, body)) => ("200 OK", body.clone()),
                None => ("404 Not Found", Vec::new()),
            };
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .as_bytes(),
            );
            let _ = stream.write_all(&body);
        }
    });
    (base, asked)
}

fn client(dir: &std::path::Path, base: &str) -> UpdateClient {
    UpdateClient::for_test(
        dir.to_owned(),
        [format!("{base}/site"), format!("{base}/github")],
    )
}

fn release(name: &str, bytes: &[u8]) -> UpdateRelease {
    UpdateRelease {
        version: Version::new(9, 0, 0),
        file_name: name.to_owned(),
        sha256: Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

#[tokio::test]
async fn an_update_comes_from_github_when_the_site_fails() {
    let dir = tempfile::tempdir().unwrap();
    let package = b"the new launcher".to_vec();
    let name = "LumilioCL-9.0.0-test.bin";
    let (base, asked) = serve(vec![(
        format!("/releases/download/v9.0.0/{name}"),
        package.clone(),
    )]);

    let path = client(dir.path(), &base)
        .download(&release(name, &package))
        .await
        .unwrap();

    assert_eq!(std::fs::read(path).unwrap(), package);
    let asked = asked.lock().unwrap().clone();
    assert!(asked[0].starts_with("/site/"), "{asked:?}");
    assert!(asked.last().unwrap().starts_with("/github/"), "{asked:?}");
}

#[tokio::test]
async fn a_package_that_does_not_match_its_checksum_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let name = "LumilioCL-9.0.0-test.bin";
    let (base, _) = serve(vec![(
        format!("/releases/download/v9.0.0/{name}"),
        b"something else".to_vec(),
    )]);

    let result = client(dir.path(), &base)
        .download(&release(name, b"the new launcher"))
        .await;

    assert!(result.is_err());
    assert!(!dir.path().join(name).exists());
}

#[tokio::test]
async fn the_manifest_comes_from_github_when_the_site_fails() {
    let Ok(target) = AssetTarget::current() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let name = format!("LumilioCL-9.0.0{}", target.suffix);
    let manifest = format!("{}  {name}\n", "a".repeat(64));
    let (base, _) = serve(vec![(
        "/releases/latest/download/SHA256SUMS.txt".to_owned(),
        manifest.into_bytes(),
    )]);

    let checked = client(dir.path(), &base).check().await.unwrap();

    let CheckResult::Available(found) = checked else {
        panic!("expected an update");
    };
    assert_eq!(found.file_name(), name);
}
