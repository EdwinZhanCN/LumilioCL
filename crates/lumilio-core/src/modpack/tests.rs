use super::*;
use crate::transfer::{FileTransport, OfficialSource, SourceProvider};
use sha1::{Digest, Sha1};
use std::io::Write;
use std::sync::Arc;
use zip::write::SimpleFileOptions;

fn sha1_of(text: &str) -> String {
    Sha1::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn chain() -> SourceChain {
    SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap()
}

fn any_source(_: &str) -> bool {
    true
}

fn index_json(files: &str, deps: &str) -> String {
    format!(
        r#"{{"formatVersion":1,"game":"minecraft","versionId":"1.0","name":"Cool Pack",
            "summary":"s","files":[{files}],"dependencies":{{{deps}}}}}"#
    )
}

fn write_pack(path: &Path, index: &str, extra: &[(&str, &str)]) {
    let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default();
    writer.start_file(INDEX, options).unwrap();
    writer.write_all(index.as_bytes()).unwrap();
    for (name, body) in extra {
        writer.start_file(*name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap();
}

#[test]
fn parses_loader_files_and_environments() {
    let json = index_json(
        r#"{"path":"mods/a.jar","hashes":{"sha1":"aa"},"downloads":["https://cdn.modrinth.com/a"],"fileSize":5},
           {"path":"mods/b.jar","hashes":{"sha1":"bb"},"env":{"client":"unsupported"},"downloads":[]},
           {"path":"mods/c.jar","hashes":{"sha1":"cc"},"env":{"client":"optional"},"downloads":[]}"#,
        r#""minecraft":"1.21.1","fabric-loader":"0.16.0""#,
    );
    let index = parse_index(&json).unwrap();
    assert_eq!(
        (index.name.as_str(), index.minecraft.as_str()),
        ("Cool Pack", "1.21.1")
    );
    assert_eq!(
        (index.loader, index.loader_version.as_deref()),
        (Loader::Fabric, Some("0.16.0"))
    );
    assert_eq!(index.files.len(), 3);
    assert_eq!(index.files[0].client, ClientSupport::Required);
    assert_eq!(index.files[1].client, ClientSupport::Unsupported);
    assert_eq!(index.files[2].client, ClientSupport::Optional);
}

#[test]
fn a_pack_without_a_loader_is_vanilla_and_two_loaders_are_refused() {
    let vanilla = parse_index(&index_json("", r#""minecraft":"1.20.1""#)).unwrap();
    assert_eq!(
        (vanilla.loader, vanilla.loader_version),
        (Loader::Vanilla, None)
    );
    let two = index_json("", r#""minecraft":"1.20.1","forge":"1","neoforge":"2""#);
    assert!(matches!(parse_index(&two), Err(ModpackError::Invalid(_))));
}

#[test]
fn malformed_indexes_are_rejected_with_a_reason() {
    for json in [
        "nope".to_owned(),
        r#"{"formatVersion":2,"game":"minecraft","name":"x","dependencies":{"minecraft":"1"}}"#
            .to_owned(),
        r#"{"formatVersion":1,"game":"terraria","name":"x","dependencies":{"minecraft":"1"}}"#
            .to_owned(),
        r#"{"formatVersion":1,"game":"minecraft","name":"x","dependencies":{}}"#.to_owned(),
        r#"{"formatVersion":1,"game":"minecraft","name":" ","dependencies":{"minecraft":"1"}}"#
            .to_owned(),
        index_json(
            r#"{"path":"mods/a.jar","hashes":{},"downloads":[]}"#,
            r#""minecraft":"1""#,
        ),
    ] {
        assert!(
            matches!(parse_index(&json), Err(ModpackError::Invalid(_))),
            "{json}"
        );
    }
}

#[test]
fn unsafe_paths_are_refused() {
    for path in [
        "../evil.jar",
        "/abs.jar",
        "a/../b.jar",
        "a\\b.jar",    // a real backslash
        "a\u{8}b.jar", // a control character
        "a\0b.jar",
        "c:/x.jar",
        "a//b.jar",
        "./a.jar",
        "",
    ] {
        // Encode with serde so the path reaches the parser exactly as written.
        let encoded = serde_json::to_string(path).unwrap();
        let json = index_json(
            &format!(r#"{{"path":{encoded},"hashes":{{"sha1":"aa"}},"downloads":[]}}"#),
            r#""minecraft":"1""#,
        );
        assert!(
            matches!(parse_index(&json), Err(ModpackError::UnsafePath(_))),
            "{path:?}"
        );
    }
}

#[test]
fn trusted_sources_are_https_and_on_the_list() {
    assert!(is_trusted_source(
        "https://cdn.modrinth.com/data/x/versions/y/a.jar"
    ));
    assert!(is_trusted_source(
        "https://raw.githubusercontent.com/o/r/main/a.jar"
    ));
    assert!(!is_trusted_source("http://cdn.modrinth.com/a.jar"));
    assert!(!is_trusted_source("https://evil.example/a.jar"));
    assert!(!is_trusted_source(
        "https://cdn.modrinth.com.evil.example/a.jar"
    ));
    assert!(!is_trusted_source("file:///etc/passwd"));
}

#[test]
fn planning_skips_unsupported_files_and_refuses_untrusted_only_sources() {
    let index = parse_index(&index_json(
        r#"{"path":"mods/a.jar","hashes":{"sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
            "downloads":["https://evil.example/a","https://cdn.modrinth.com/a"],"fileSize":5},
           {"path":"mods/skip.jar","hashes":{"sha1":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},
            "env":{"client":"unsupported"},"downloads":[]}"#,
        r#""minecraft":"1""#,
    ))
    .unwrap();
    let requests = plan(&index, Path::new("/g"), &chain(), is_trusted_source).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].sources(), ["https://cdn.modrinth.com/a"]);
    assert_eq!(requests[0].destination(), Path::new("/g/mods/a.jar"));

    let bad = parse_index(&index_json(
        r#"{"path":"mods/a.jar","hashes":{"sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
            "downloads":["https://evil.example/a"]}"#,
        r#""minecraft":"1""#,
    ))
    .unwrap();
    assert!(matches!(
        plan(&bad, Path::new("/g"), &chain(), is_trusted_source),
        Err(ModpackError::UntrustedSource(_))
    ));
}

#[test]
fn overrides_extract_with_client_overrides_winning_and_no_escapes() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("p.mrpack");
    write_pack(
        &pack,
        &index_json("", r#""minecraft":"1""#),
        &[
            ("overrides/config/a.toml", "base"),
            ("overrides/options.txt", "o"),
            ("client-overrides/config/a.toml", "client"),
            ("overrides/../escape.txt", "no"),
            ("overrides/a/../../escape2.txt", "no"),
            ("other/ignored.txt", "no"),
        ],
    );
    let game = dir.path().join("game");
    fs::create_dir_all(&game).unwrap();
    let written = extract_overrides(&pack, &game).unwrap();
    assert_eq!(written, 3);
    assert_eq!(
        fs::read_to_string(game.join("config/a.toml")).unwrap(),
        "client"
    );
    assert_eq!(fs::read_to_string(game.join("options.txt")).unwrap(), "o");
    assert!(!dir.path().join("escape.txt").exists());
    assert!(!game.join("ignored.txt").exists());
}

fn server_with(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, body).unwrap();
    url::Url::from_file_path(path).unwrap().to_string()
}

#[tokio::test]
async fn an_already_cancelled_empty_pack_never_creates_an_instance() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let pack = dir.path().join("empty.mrpack");
    write_pack(
        &pack,
        &index_json("", r#""minecraft":"1.21.1""#),
        &[("overrides/config/a.txt", "value")],
    );
    let store = tokio::sync::Mutex::new(InstanceStore::open(&root).unwrap());
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        import(&store, &engine, &chain(), &pack, 10, |_| true, cancel).await,
        Err(ModpackError::Cancelled)
    ));
    assert!(store.lock().await.instances().is_empty());
    assert!(!root.join("profiles").exists());
}

#[tokio::test]
async fn importing_creates_the_instance_downloads_files_and_applies_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let server = dir.path().join("server");
    fs::create_dir_all(&server).unwrap();
    let url = server_with(&server, "a.jar", "mod-a");
    let index = index_json(
        &format!(
            r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{}"}},"downloads":["{url}"],"fileSize":5}}"#,
            sha1_of("mod-a")
        ),
        r#""minecraft":"1.21.1","fabric-loader":"0.16.0""#,
    );
    let pack = dir.path().join("p.mrpack");
    write_pack(&pack, &index, &[("overrides/config/x.toml", "cfg")]);

    let store = tokio::sync::Mutex::new(InstanceStore::open(&root).unwrap());
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let record = import(
        &store,
        &engine,
        &chain(),
        &pack,
        10,
        any_source,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(record.name, "Cool Pack");
    assert_eq!(
        (record.loader, record.game_version.as_str()),
        (Loader::Fabric, "1.21.1")
    );
    let game = store.lock().await.directories(&record).game().to_path_buf();
    assert_eq!(
        fs::read_to_string(game.join("mods/a.jar")).unwrap(),
        "mod-a"
    );
    assert_eq!(
        fs::read_to_string(game.join("config/x.toml")).unwrap(),
        "cfg"
    );
    assert!(
        InstanceStore::open(&root)
            .unwrap()
            .get(&record.id)
            .is_some()
    );
}

#[tokio::test]
async fn a_failed_import_leaves_no_instance_or_files_behind() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let server = dir.path().join("server");
    fs::create_dir_all(&server).unwrap();
    let url = server_with(&server, "a.jar", "TAMPERED");
    let index = index_json(
        &format!(
            r#"{{"path":"mods/a.jar","hashes":{{"sha1":"{}"}},"downloads":["{url}"]}}"#,
            sha1_of("mod-a")
        ),
        r#""minecraft":"1.21.1""#,
    );
    let pack = dir.path().join("p.mrpack");
    write_pack(&pack, &index, &[("overrides/config/x.toml", "cfg")]);

    let store = tokio::sync::Mutex::new(InstanceStore::open(&root).unwrap());
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let result = import(
        &store,
        &engine,
        &chain(),
        &pack,
        10,
        any_source,
        CancellationToken::new(),
    )
    .await;
    assert!(result.is_err());
    assert!(store.lock().await.instances().is_empty());
    assert!(InstanceStore::open(&root).unwrap().instances().is_empty());
    assert!(!root.join("profiles").join("cool-pack").exists());
}

#[tokio::test]
async fn an_untrusted_source_fails_before_anything_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("launcher");
    let index = index_json(
        r#"{"path":"mods/a.jar","hashes":{"sha1":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
            "downloads":["https://evil.example/a.jar"]}"#,
        r#""minecraft":"1.21.1""#,
    );
    let pack = dir.path().join("p.mrpack");
    write_pack(&pack, &index, &[]);
    let store = tokio::sync::Mutex::new(InstanceStore::open(&root).unwrap());
    let engine = TransferEngine::new(FileTransport, 2).unwrap();
    let result = import(
        &store,
        &engine,
        &chain(),
        &pack,
        1,
        is_trusted_source,
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(result, Err(ModpackError::UntrustedSource(_))));
    assert!(store.lock().await.instances().is_empty());
}
