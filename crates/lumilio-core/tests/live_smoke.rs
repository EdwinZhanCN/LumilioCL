//! Manual check against the real services (Mojang, Fabric meta, Modrinth).
//!
//! Ignored by default: it needs the network, downloads the game, and starts it.
//! Run with
//! `LUMILIO_LIVE_HOME=/some/folder cargo test -p lumilio-core --test live_smoke -- --ignored --nocapture`.
//! The game is stopped as soon as it reports running.

use std::time::Duration;

use lumilio_core::{
    CancellationToken, DefaultTransport, LaunchPhase, LaunchSignal, LaunchUpdate, LauncherService,
    Loader, ProjectKind, SearchQuery,
};

#[tokio::test]
#[ignore = "talks to the real services and downloads the game"]
async fn the_whole_spine_against_the_real_services() {
    let home = std::env::var("LUMILIO_LIVE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("lumilio-live"));
    println!("launcher root: {}", home.display());
    // Content comes from the Modrinth source plugin, as in the app.
    let service = LauncherService::open(
        &home,
        DefaultTransport::new().unwrap(),
        vec![std::sync::Arc::new(lumilio_plugin_modrinth::Modrinth)],
    )
    .unwrap();

    // Search (Modrinth).
    let page = service
        .search(&SearchQuery {
            text: "sodium".to_owned(),
            ..SearchQuery::new(ProjectKind::Mod)
        })
        .await
        .expect("Modrinth search");
    println!(
        "search: {} hits, first {:?}",
        page.total_hits,
        page.hits.first().map(|h| &h.slug)
    );
    assert!(page.hits.iter().any(|hit| hit.slug == "sodium"));

    // Filters, paging, sorting and the detail bundle (Modrinth tags + project).
    let filters = service.discover_filters().await.expect("tag lists");
    let release = filters
        .game_versions
        .iter()
        .find(|tag| tag.release)
        .expect("a release version");
    let optimization = filters
        .categories
        .iter()
        .find(|tag| tag.kind == ProjectKind::Mod && tag.name == "optimization")
        .expect("the optimization category");
    println!(
        "{} categories, {} game versions (newest release {})",
        filters.categories.len(),
        filters.game_versions.len(),
        release.version
    );
    let filtered = service
        .search(&SearchQuery {
            game_versions: vec![release.version.clone()],
            loaders: vec![lumilio_core::Pick::include("fabric").any_of()],
            categories: vec![lumilio_core::Pick::include(optimization.name.clone())],
            sort: lumilio_core::SortIndex::Downloads,
            page: 1,
            page_size: 10,
            ..SearchQuery::new(ProjectKind::Mod)
        })
        .await
        .expect("a filtered, sorted, second-page search");
    println!(
        "filtered search: {} hits total, {} on page 2",
        filtered.total_hits,
        filtered.hits.len()
    );
    assert!(filtered.hits.len() <= 10);
    assert!(
        filtered
            .hits
            .iter()
            .all(|hit| hit.loaders.iter().any(|l| l == "fabric"))
    );
    let detail = service
        .project_detail("sodium")
        .await
        .expect("project detail");
    println!(
        "detail: {} versions, {} gallery images, owner {:?}",
        detail.versions.len(),
        detail.project.gallery.len(),
        detail.owner
    );
    assert!(!detail.project.body.is_empty() && !detail.versions.is_empty());

    // Create with the newest release and the recommended Fabric loader (Mojang + Fabric meta).
    let fabric = service
        .create_instance("Live Fabric", None, Loader::Fabric, None)
        .await
        .expect("create a Fabric instance");
    println!(
        "created {} = {} + fabric {:?}",
        fabric.id, fabric.game_version, fabric.loader_version
    );

    // Install Sodium into it (Modrinth versions + CDN).
    match service
        .install_content(
            &fabric.id,
            ProjectKind::Mod,
            "sodium",
            CancellationToken::new(),
        )
        .await
    {
        Ok(file) => println!("installed {file}"),
        Err(error) => println!("install skipped: {error}"),
    }

    // Launch until the game reports running, then stop it.
    let cancel = CancellationToken::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let launch = service.launch(&fabric.id, tx, cancel.clone());
    let watcher = async {
        while let Some(update) = rx.recv().await {
            match update {
                LaunchUpdate::Signal(LaunchSignal::Phase(phase)) => println!("phase {phase:?}"),
                LaunchUpdate::Signal(LaunchSignal::Running) => {
                    println!("running; stopping in 3s");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    cancel.cancel();
                }
                LaunchUpdate::Signal(LaunchSignal::Failed(failure)) => {
                    println!("failed: {failure:?}")
                }
                LaunchUpdate::Log { text, .. } if text.contains("ERROR") => println!("log: {text}"),
                _ => {}
            }
        }
    };
    let (result, ()) = tokio::join!(launch, watcher);
    println!("launch result: {result:?}");
    let exit = result.expect("the launch ended cleanly");
    assert!(exit.was_running, "the game never got as far as running");
    let _ = LaunchPhase::Starting;
}

#[tokio::test]
#[ignore = "talks to Mojang and downloads a Java runtime"]
async fn a_java_runtime_installs_from_the_real_index_and_runs() {
    let home = std::env::temp_dir().join(format!("lumilio-java-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let service =
        LauncherService::open(&home, DefaultTransport::new().unwrap(), Vec::new()).unwrap();
    let java = service
        .install_java(Some(17), CancellationToken::new())
        .await
        .expect("Java 17 from Mojang");
    println!("installed {} at {}", java.version(), java.home().display());
    assert_eq!(java.major(), 17);
    let output = std::process::Command::new(java.executable())
        .arg("-version")
        .output()
        .expect("the installed Java starts");
    let text = String::from_utf8_lossy(&output.stderr);
    println!("{text}");
    assert!(output.status.success() && text.contains("17"));
    let _ = std::fs::remove_dir_all(&home);
}
