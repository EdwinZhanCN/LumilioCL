use super::super::error::ServiceError;
use super::{fabric_instance, mod_version, project_versions, sha1_hex, two_versions, world};
use crate::activity::CancellationToken;
use crate::discover::ProjectKind;
use crate::instance::Loader;

#[tokio::test]
async fn a_version_can_be_saved_where_the_person_chooses_and_can_be_retried() {
    let world = world();
    world
        .net
        .answer("/v2/project/cool/version", mod_version(&world));
    let target = world._dir.path().join("elsewhere.jar");

    let name = world
        .service
        .save_version_as("cool", "v1", &target, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(name, "cool.jar");
    assert_eq!(std::fs::read(&target).unwrap(), b"a mod");
    assert!(world.service.activity(10).finished[0].retry.is_some());

    // An unknown version is an error.
    assert!(matches!(
        world
            .service
            .save_version_as("cool", "nope", &target, CancellationToken::new())
            .await,
        Err(ServiceError::NoCompatibleVersion)
    ));
}

#[tokio::test]
async fn required_dependencies_are_listed_nearest_first_once_and_without_what_is_there() {
    let world = world();
    let record = world
        .service
        .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    // cool needs LIBX and LIBY; LIBX needs LIBY and cool back (a cycle).
    world.net.answer(
        "/v2/project/cool/version",
        project_versions("P", "1.0", &["LIBX", "LIBY"]),
    );
    world.net.answer(
        "/v2/project/LIBX/version",
        project_versions("LIBX", "1.0", &["LIBY", "P"]),
    );
    world.net.answer(
        "/v2/project/LIBY/version",
        project_versions("LIBY", "1.0", &[]),
    );

    let need = world
        .service
        .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    let ids: Vec<_> = need.iter().map(|n| n.project_id.as_str()).collect();
    assert_eq!(ids, ["LIBX", "LIBY"]);
    assert!(need.iter().all(|n| n.version.is_some()));

    // Only mods have dependencies to offer.
    assert!(
        world
            .service
            .missing_dependencies(&record.id, ProjectKind::ResourcePack, "cool", None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn optional_mods_are_suggested_and_installed_ones_it_cannot_live_with_are_named() {
    let world = world();
    let record = world
        .service
        .create_instance("Rel", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    let file = |id: &str| {
        format!(
            r#""files":[{{"url":"https://cdn.modrinth.com/{id}.jar","filename":"{id}.jar",
                "primary":true,"size":4,"hashes":{{"sha1":"{id}"}}}}]"#
        )
    };
    let version = |id: &str, deps: &str| {
        format!(
            r#"[{{"id":"v-{id}","project_id":"{id}","name":"{id}","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",{},"dependencies":[{deps}]}}]"#,
            file(id)
        )
        .into_bytes()
    };
    world.net.answer(
        "/v2/project/cool/version",
        version(
            "P",
            r#"{"project_id":"OPT","dependency_type":"optional"},
               {"project_id":"BAD","dependency_type":"incompatible"},
               {"project_id":"FINE","dependency_type":"incompatible"}"#,
        ),
    );
    world
        .net
        .answer("/v2/project/OPT/version", version("OPT", ""));
    // BAD is installed (Modrinth recognises it by hash); FINE is not.
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("bad.jar"), b"bad").unwrap();
    world.net.answer(
        "/v2/version_files",
        format!(
            r#"{{"{}":{}}}"#,
            sha1_hex(b"bad"),
            String::from_utf8(version("BAD", ""))
                .unwrap()
                .trim_start_matches('[')
                .trim_end_matches(']')
        ),
    );

    let report = world
        .service
        .dependency_report(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    assert!(report.needs.is_empty());
    let optional: Vec<_> = report
        .optional
        .iter()
        .map(|n| n.project_id.as_str())
        .collect();
    assert_eq!(optional, ["OPT"]);
    assert!(report.optional[0].version.is_some());
    assert_eq!(
        report.conflicts,
        ["BAD"],
        "only what is installed conflicts"
    );
}

#[tokio::test]
async fn a_dependency_already_in_the_game_is_not_offered_and_one_without_a_fit_is_flagged() {
    let world = world();
    let record = world
        .service
        .create_instance("Deps", Some("1.0"), Loader::Fabric, Some("0.16.0"))
        .await
        .unwrap();
    world.net.answer(
        "/v2/project/cool/version",
        project_versions("P", "1.0", &["LIBX", "OLD"]),
    );
    world.net.answer(
        "/v2/project/LIBX/version",
        project_versions("LIBX", "1.0", &[]),
    );
    world.net.answer(
        "/v2/project/OLD/version",
        project_versions("OLD", "0.9", &[]),
    );
    // The game already holds LIBX, which Modrinth recognises by hash.
    let mods = world.service.layout.game(&record.id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("libx.jar"), b"libx").unwrap();
    world.net.answer(
        "/v2/version_files",
        format!(
            r#"{{"{}":{{"id":"v-LIBX","project_id":"LIBX","name":"x","version_number":"1",
                "version_type":"release","game_versions":["1.0"],"loaders":["fabric"],
                "date_published":"2024-01-01T00:00:00Z",
                "files":[{{"url":"https://cdn.modrinth.com/l.jar","filename":"l.jar",
                           "primary":true,"size":4,"hashes":{{"sha1":"{}"}}}}],
                "dependencies":[]}}}}"#,
            sha1_hex(b"libx"),
            sha1_hex(b"libx")
        ),
    );

    let need = world
        .service
        .missing_dependencies(&record.id, ProjectKind::Mod, "cool", None)
        .await
        .unwrap();
    assert_eq!(need.len(), 1, "{need:?}");
    assert_eq!(need[0].project_id, "OLD");
    assert!(need[0].version.is_none(), "no version fits this game");
}

#[tokio::test]
async fn filters_are_fetched_once_and_failures_are_not_remembered() {
    let world = world();
    assert!(matches!(
        world.service.discover_filters().await,
        Err(ServiceError::Remote(_))
    ));
    world.net.answer(
        "/v2/tag/category",
        r#"[{"name":"adventure","project_type":"mod","header":"categories"}]"#,
    );
    world.net.answer(
        "/v2/tag/game_version",
        r#"[{"version":"1.21.1","version_type":"release","date":"2024-08-08T00:00:00Z"}]"#,
    );
    let filters = world.service.discover_filters().await.unwrap();
    assert_eq!(filters.categories.len(), 1);
    assert_eq!(filters.game_versions[0].version, "1.21.1");
    // Served from memory: the network can vanish.
    world.net.forget_all();
    assert_eq!(world.service.discover_filters().await.unwrap(), filters);
}

#[tokio::test]
async fn detail_bundles_project_versions_newest_first_and_survives_a_missing_owner() {
    let world = world();
    world.net.answer(
        "/v2/project/cool/version",
        r#"[{"id":"old","project_id":"P","name":"Old","version_number":"1","version_type":"release",
             "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2023-01-01T00:00:00Z",
             "files":[{"url":"https://x/o.jar","filename":"o.jar","primary":true,"size":1,"hashes":{}}]},
            {"id":"new","project_id":"P","name":"New","version_number":"2","version_type":"release",
             "game_versions":["1.0"],"loaders":["fabric"],"date_published":"2024-01-01T00:00:00Z",
             "files":[{"url":"https://x/n.jar","filename":"n.jar","primary":true,"size":1,"hashes":{}}]}]"#,
    );
    world
        .net
        .answer("/v2/project/cool/members", "not json at all");
    world.net.answer(
        "/v2/project/cool",
        r##"{"id":"P","slug":"cool","title":"Cool","project_type":"mod","body":"# Hello"}"##,
    );
    let detail = world.service.project_detail("cool").await.unwrap();
    assert_eq!(detail.project.title, "Cool");
    assert_eq!(detail.project.body, "# Hello");
    let order: Vec<_> = detail.versions.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(order, ["new", "old"]);
    assert_eq!(detail.owner, None);
}

#[tokio::test]
async fn a_chosen_older_version_is_installed_instead_of_the_newest() {
    let world = world();
    let record = fabric_instance(&world, "1.0").await;
    world
        .net
        .answer("/v2/project/cool/version", two_versions(&world));
    let file = world
        .service
        .install_version(
            &record.id,
            ProjectKind::Mod,
            "cool",
            "v1",
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(file, "v1.jar");
    let path = world.service.layout().game(&record.id).join("mods/v1.jar");
    assert_eq!(std::fs::read(path).unwrap(), b"one");
}

#[tokio::test]
async fn a_version_for_another_game_or_an_unknown_id_is_refused_before_any_download() {
    let world = world();
    let record = fabric_instance(&world, "1.0").await;
    world
        .net
        .answer("/v2/project/cool/version", two_versions(&world));
    for id in ["v2", "ghost"] {
        let result = world
            .service
            .install_version(
                &record.id,
                ProjectKind::Mod,
                "cool",
                id,
                CancellationToken::new(),
            )
            .await;
        assert!(
            matches!(result, Err(ServiceError::NoCompatibleVersion)),
            "{id}"
        );
    }
    assert!(
        !world
            .service
            .layout()
            .game(&record.id)
            .join("mods")
            .exists()
    );
}
