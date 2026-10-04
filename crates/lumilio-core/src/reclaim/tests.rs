use super::*;
use crate::instance::InstanceSettings;

fn record(id: &str, game: &str, loader: Loader, loader_version: Option<&str>) -> InstanceRecord {
    InstanceRecord {
        id: id.into(),
        name: id.into(),
        game_version: game.into(),
        loader,
        loader_version: loader_version.map(str::to_owned),
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
    }
}

fn put(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

/// A small shared tree: 1.21.1 and 1.20.1 (vanilla), plus a Fabric profile
/// for 1.21.1, each with libraries, assets and natives.
fn tree() -> (tempfile::TempDir, Layout) {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path());
    let meta = layout.meta();
    let version = |id: &str, library: &str, assets: &str| {
        format!(
            r#"{{"id":"{id}","assets":"{assets}","assetIndex":{{"id":"{assets}"}},
                "logging":{{"client":{{"file":{{"id":"log-{assets}.xml"}}}}}},
                "libraries":[{{"name":"x:{library}:1","downloads":{{"artifact":{{"path":"x/{library}/1/{library}-1.jar"}}}}}},
                             {{"name":"net.fabricmc:fabric-loader:0.16.0"}}]}}"#
        )
    };
    put(
        &meta.join("versions/1.21.1/1.21.1.json"),
        &version("1.21.1", "lwjgl", "17"),
    );
    put(&meta.join("versions/1.21.1/1.21.1.jar"), "client");
    put(
        &meta.join("versions/fabric-loader-0.16.0-1.21.1/fabric-loader-0.16.0-1.21.1.json"),
        &version("fabric-loader-0.16.0-1.21.1", "lwjgl", "17"),
    );
    put(
        &meta.join("versions/1.20.1/1.20.1.json"),
        &version("1.20.1", "old", "12"),
    );
    put(&meta.join("natives/1.21.1/n.dylib"), "n");
    put(&meta.join("natives/1.20.1/n.dylib"), "n");
    for path in [
        "x/lwjgl/1/lwjgl-1.jar",
        "x/old/1/old-1.jar",
        "x/stray/1/stray-1.jar",
        "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
    ] {
        put(&meta.join("libraries").join(path), "lib");
    }
    put(
        &meta.join("assets/indexes/17.json"),
        r#"{"objects":{"a":{"hash":"aa11"},"b":{"hash":"bb22"}}}"#,
    );
    put(
        &meta.join("assets/indexes/12.json"),
        r#"{"objects":{"c":{"hash":"cc33"}}}"#,
    );
    for hash in ["aa11", "bb22", "cc33", "dd44"] {
        put(
            &meta.join("assets/objects").join(&hash[..2]).join(hash),
            hash,
        );
    }
    put(&meta.join("assets/log_configs/log-17.xml"), "x");
    put(&meta.join("assets/log_configs/log-12.xml"), "x");
    (dir, layout)
}

fn names(report: &Reclaimable, layout: &Layout) -> Vec<String> {
    let meta = layout.meta();
    report
        .unused
        .iter()
        .map(|(path, _)| {
            path.strip_prefix(&meta)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn what_no_game_needs_is_found_and_what_one_needs_is_kept() {
    let (_dir, layout) = tree();
    let games = [record("a", "1.21.1", Loader::Fabric, Some("0.16.0"))];
    let report = scan(&layout, &games).unwrap();
    let found = names(&report, &layout);
    for gone in [
        "versions/1.20.1",
        "natives/1.20.1",
        "libraries/x/old/1/old-1.jar",
        "libraries/x/stray/1/stray-1.jar",
        "assets/objects/cc/cc33",
        "assets/objects/dd/dd44",
        "assets/indexes/12.json",
        "assets/log_configs/log-12.xml",
    ] {
        assert!(
            found.iter().any(|f| f == gone),
            "{gone} should be unused: {found:?}"
        );
    }
    for kept in [
        "versions/1.21.1",
        "versions/fabric-loader-0.16.0-1.21.1",
        "natives/1.21.1",
        "libraries/x/lwjgl/1/lwjgl-1.jar",
        "libraries/net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar",
        "assets/objects/aa/aa11",
        "assets/indexes/17.json",
        "assets/log_configs/log-17.xml",
    ] {
        assert!(!found.iter().any(|f| f == kept), "{kept} is in use");
    }
    assert!(report.bytes() > 0);
}

#[test]
fn with_no_games_everything_shared_is_unused_and_removing_frees_it_and_nothing_else() {
    let (dir, layout) = tree();
    let before = crate::storage::directory_size(&layout.meta());
    put(&dir.path().join("profiles/keep/game/options.txt"), "mine");
    let report = scan(&layout, &[]).unwrap();
    let freed = remove(&layout, &report).unwrap();
    assert_eq!(freed, report.bytes());
    assert!(freed > 0 && freed <= before);
    assert_eq!(crate::storage::directory_size(&layout.meta()), 0);
    assert!(dir.path().join("profiles/keep/game/options.txt").is_file());
    // Scanning again finds nothing more.
    assert!(scan(&layout, &[]).unwrap().unused.is_empty());
}

#[test]
fn a_game_whose_version_cannot_be_read_blocks_the_whole_removal() {
    let (_dir, layout) = tree();
    fs::write(layout.versions().join("1.21.1/1.21.1.json"), "garbled").unwrap();
    let games = [record("a", "1.21.1", Loader::Vanilla, None)];
    assert!(matches!(
        scan(&layout, &games),
        Err(ReclaimError::Unreadable(name)) if name == "1.21.1"
    ));
}

#[test]
fn installer_made_loaders_keep_the_libraries_and_an_unreadable_asset_index_keeps_the_assets() {
    let (_dir, layout) = tree();
    let games = [record("a", "1.21.1", Loader::NeoForge, Some("21.1.50"))];
    let report = scan(&layout, &games).unwrap();
    assert!(
        names(&report, &layout)
            .iter()
            .all(|n| !n.starts_with("libraries/"))
    );
    assert!(report.kept.iter().any(|note| note.contains("Forge")));

    fs::remove_file(layout.assets().join("indexes/17.json")).unwrap();
    let vanilla = [record("a", "1.21.1", Loader::Vanilla, None)];
    let report = scan(&layout, &vanilla).unwrap();
    assert!(
        names(&report, &layout)
            .iter()
            .all(|n| !n.starts_with("assets/objects/"))
    );
    assert!(report.kept.iter().any(|note| note.contains("资源")));
}

#[test]
fn removal_never_follows_a_link_or_leaves_meta() {
    let (dir, layout) = tree();
    let outside = dir.path().join("outside.txt");
    fs::write(&outside, "keep me").unwrap();
    let link = layout.libraries().join("x/stray/link.jar");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let found = Reclaimable {
        unused: vec![
            (link.clone(), 1),
            (outside.clone(), 7),
            (layout.meta().join("../outside.txt"), 7),
        ],
        kept: Vec::new(),
    };
    assert_eq!(remove(&layout, &found).unwrap(), 0);
    assert!(outside.is_file() && link.symlink_metadata().is_ok());
}
