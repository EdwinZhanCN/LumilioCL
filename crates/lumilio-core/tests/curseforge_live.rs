//! Manual check of the CurseForge source against the real API.
//!
//! Ignored by default: it needs the network and a build with `CF_API_KEY`
//! (the test passes trivially without one). Run with
//! `cargo nextest run -p lumilio-core --test curseforge_live --run-ignored only --no-capture`.
//! `LUMILIO_LIVE_PACK=<CurseForge project ID>` also installs that modpack's
//! newest version and prints what had to be left for the person;
//! `LUMILIO_LIVE_HOME=<folder>` keeps it there.

use std::sync::Arc;

use lumilio_core::{
    CancellationToken, ContentLookup, DefaultTransport, LauncherService, ProjectKind, SearchQuery,
};
use lumilio_plugin_curseforge::{CurseForge, ID};

#[tokio::test]
#[ignore = "talks to the real CurseForge API"]
async fn curseforge_answers_as_the_launcher_expects() {
    if !CurseForge::default().has_key() {
        println!("this build has no CurseForge key; nothing to check");
        return;
    }
    // `LUMILIO_LIVE_HOME` keeps what was installed, to look at it in the app.
    let scratch = tempfile::tempdir().unwrap();
    let home = std::env::var("LUMILIO_LIVE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| scratch.path().to_owned());
    let service = LauncherService::open(
        &home,
        DefaultTransport::new().unwrap(),
        vec![
            Arc::new(lumilio_plugin_modrinth::Modrinth),
            Arc::new(CurseForge::default()),
        ],
    )
    .unwrap();

    let sources = service.content_sources().await;
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[1].id, ID);

    let page = service
        .search(
            ID,
            &SearchQuery {
                text: "jei".to_owned(),
                ..SearchQuery::new(ProjectKind::Mod)
            },
        )
        .await
        .expect("CurseForge search");
    println!("search: {} hits", page.total_hits);
    assert!(page.hits.iter().any(|hit| hit.project_id == "238222"));

    let filters = service.discover_filters(ID).await.expect("filter choices");
    assert!(!filters.categories.is_empty() && !filters.game_versions.is_empty());

    let detail = service
        .project_detail(ID, "238222")
        .await
        .expect("JEI's detail");
    println!(
        "JEI: {} versions, body {} bytes",
        detail.versions.len(),
        detail.project.body.len()
    );
    assert!(detail.versions.len() > 1000, "every page of files was read");
    assert!(!detail.project.body.is_empty());
    let newest = &detail.versions[0];
    let changelog = service
        .version_changelog(ID, "238222", &newest.id)
        .await
        .expect("one changelog");
    println!(
        "newest {} changelog: {} bytes",
        newest.number,
        changelog.len()
    );

    if let Ok(pack) = std::env::var("LUMILIO_LIVE_PACK") {
        let record = service
            .install_modpack(ID, &pack, None, CancellationToken::new())
            .await
            .expect("the pack installs");
        let pending = service.pending_downloads(&record.id).await.unwrap();
        println!(
            "installed {} ({} {:?}); {} file(s) to add by hand:",
            record.name,
            record.game_version,
            record.loader,
            pending.len()
        );
        for file in pending {
            println!("  {} -> {:?} ({:?})", file.file_name, file.path, file.page);
        }
        let started = std::time::Instant::now();
        let list = service
            .content_details(&record.id, ProjectKind::Mod, ContentLookup::Fresh)
            .await
            .expect("the pack's mods");
        let mut by_source = std::collections::BTreeMap::<String, usize>::new();
        for entry in &list.entries {
            let name = entry
                .source
                .as_ref()
                .map_or("unknown".to_owned(), |source| source.source_name.clone());
            *by_source.entry(name).or_default() += 1;
        }
        println!(
            "mods recognized in {:?}: {by_source:?}; updates {}; note {:?}",
            started.elapsed(),
            list.entries
                .iter()
                .filter(|entry| entry.update.is_some())
                .count(),
            list.source_note
        );
    }
}
