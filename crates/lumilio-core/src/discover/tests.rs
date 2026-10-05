use super::SITE_BASE;
use super::convert::{loader_from_api, query_to_api};
use super::intent::{IntentError, install_request};
use super::kinds::{ProjectKind, SortIndex, browse_page_url};
use super::query::{Pick, SearchQuery, Stance};
use super::search::{Environment, SideSupport, environment};
use super::versions::{
    Dependency, DependencyKind, ReleaseChannel, Version, VersionFile, pick_version,
};
use crate::instance::Loader;
use lumilio_plugin_api::content as api;
use std::path::Path;

const SHA1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn version(
    id: &str,
    channel: ReleaseChannel,
    published: &str,
    games: &[&str],
    loaders: &[&str],
) -> Version {
    Version {
        id: id.to_owned(),
        project_id: "P".to_owned(),
        name: "n".to_owned(),
        number: id.to_owned(),
        channel,
        game_versions: games.iter().map(|game| (*game).to_owned()).collect(),
        loaders: loaders.iter().map(|loader| (*loader).to_owned()).collect(),
        published: published.to_owned(),
        files: vec![VersionFile {
            url: format!("https://cdn/x/{id}.jar"),
            filename: format!("{id}.jar"),
            primary: true,
            size: 10,
            sha1: Some(SHA1.to_owned()),
        }],
        dependencies: Vec::new(),
        downloads: 0,
        changelog: String::new(),
    }
}

fn versions() -> Vec<Version> {
    use ReleaseChannel::{Beta, Release};
    vec![
        version(
            "old-release",
            Release,
            "2024-01-01T00:00:00Z",
            &["1.21.1"],
            &["fabric"],
        ),
        version(
            "new-release",
            Release,
            "2024-06-01T00:00:00Z",
            &["1.21.1"],
            &["fabric"],
        ),
        version(
            "newest-beta",
            Beta,
            "2024-09-01T00:00:00Z",
            &["1.21.1"],
            &["fabric"],
        ),
        version(
            "forge-only",
            Release,
            "2024-12-01T00:00:00Z",
            &["1.21.1"],
            &["forge"],
        ),
        version(
            "other-game",
            Release,
            "2025-01-01T00:00:00Z",
            &["1.20.4"],
            &["fabric"],
        ),
    ]
}

#[test]
fn browse_pages_are_the_plural_listing_of_each_kind() {
    assert_eq!(
        browse_page_url(ProjectKind::Mod),
        format!("{SITE_BASE}/mods")
    );
    assert_eq!(
        browse_page_url(ProjectKind::Modpack),
        format!("{SITE_BASE}/modpacks")
    );
    assert_eq!(
        browse_page_url(ProjectKind::ResourcePack),
        format!("{SITE_BASE}/resourcepacks")
    );
    assert_eq!(
        browse_page_url(ProjectKind::Shader),
        format!("{SITE_BASE}/shaders")
    );
}

#[test]
fn required_dependencies_are_only_the_required_ones() {
    let mut version = versions().remove(0);
    version.dependencies = vec![
        Dependency {
            project_id: Some("D".into()),
            version_id: None,
            kind: DependencyKind::Required,
        },
        Dependency {
            project_id: Some("E".into()),
            version_id: None,
            kind: DependencyKind::Other,
        },
    ];
    assert_eq!(version.required_dependencies().count(), 1);
}

#[test]
fn the_primary_file_wins_over_the_first() {
    let mut version = versions().remove(0);
    let file = |name: &str, primary| VersionFile {
        url: "u".into(),
        filename: name.into(),
        primary,
        size: 0,
        sha1: None,
    };
    version.files = vec![file("sources.jar", false), file("mod.jar", true)];
    assert_eq!(version.install_file().unwrap().filename, "mod.jar");
}

#[test]
fn picks_the_newest_stable_compatible_version() {
    let all = versions();
    let picked = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).unwrap();
    // The newer beta and the forge/other-game builds do not qualify.
    assert_eq!(picked.id, "new-release");
    let forge = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Forge).unwrap();
    assert_eq!(forge.id, "forge-only");
    assert!(pick_version(&all, ProjectKind::Mod, "1.19", Loader::Fabric).is_none());
    // Without a stable build the beta is used.
    let betas: Vec<_> = all
        .iter()
        .filter(|v| v.id == "newest-beta")
        .cloned()
        .collect();
    assert_eq!(
        pick_version(&betas, ProjectKind::Mod, "1.21.1", Loader::Fabric)
            .unwrap()
            .id,
        "newest-beta"
    );
}

#[test]
fn loaders_do_not_filter_resource_packs() {
    let all = vec![version(
        "rp",
        ReleaseChannel::Release,
        "2024-01-01T00:00:00Z",
        &["1.21.1"],
        &["minecraft"],
    )];
    assert!(pick_version(&all, ProjectKind::ResourcePack, "1.21.1", Loader::Fabric).is_some());
    assert!(pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).is_none());
}

#[test]
fn install_requests_target_the_right_folder_and_verify_the_file() {
    let all = versions();
    let version = pick_version(&all, ProjectKind::Mod, "1.21.1", Loader::Fabric).unwrap();
    let game = Path::new("/lib/instances/x/game");
    let request = install_request(
        ProjectKind::Mod,
        version,
        game,
        vec!["https://cdn/a".into()],
    )
    .unwrap();
    assert_eq!(request.destination(), game.join("mods/new-release.jar"));
    assert_eq!(request.expected_sha1(), Some(SHA1));
    assert_eq!(request.id(), "content:P:new-release");
    let shader = install_request(
        ProjectKind::Shader,
        version,
        game,
        vec!["https://cdn/a".into()],
    )
    .unwrap();
    assert_eq!(
        shader.destination(),
        game.join("shaderpacks/new-release.jar")
    );
    assert_eq!(
        install_request(ProjectKind::Modpack, version, game, vec!["u".into()]).unwrap_err(),
        IntentError::NotAFileKind
    );
}

#[test]
fn unsafe_file_names_are_refused() {
    for name in [
        "../evil.jar",
        "a/b.jar",
        "a\\b.jar",
        "..",
        "c:evil.jar",
        " pad.jar",
    ] {
        let mut version = versions().remove(0);
        version.files[0].filename = name.to_owned();
        assert_eq!(
            install_request(
                ProjectKind::Mod,
                &version,
                Path::new("/g"),
                vec!["u".into()]
            )
            .unwrap_err(),
            IntentError::UnsafeFileName(name.to_owned()),
            "{name}"
        );
    }
}

#[test]
fn environment_reads_the_two_side_flags() {
    use SideSupport::*;
    assert_eq!(
        environment(Required, Unsupported),
        Some(Environment::ClientOnly)
    );
    assert_eq!(
        environment(Unsupported, Required),
        Some(Environment::ServerOnly)
    );
    assert_eq!(
        environment(Optional, Optional),
        Some(Environment::ClientOrServer)
    );
    assert_eq!(
        environment(Required, Required),
        Some(Environment::ClientAndServer)
    );
    assert_eq!(
        environment(Required, Optional),
        Some(Environment::ClientOnly)
    );
    // Not knowing is not the same as not running there.
    assert_eq!(environment(Unknown, Unknown), None);
    assert_eq!(environment(Required, Unknown), None);
    assert_eq!(environment(Unsupported, Unsupported), None);
}

#[test]
fn the_search_request_keeps_the_page_semantics_when_handed_to_a_source() {
    let mut query = SearchQuery::new(ProjectKind::Mod);
    query.sort = SortIndex::Downloads;
    query.page_size = 500;
    query.client = true;
    query.loaders = vec![
        Pick::include("fabric"),
        Pick::exclude("forge"),
        Pick::include(" "),
    ];
    query.categories = vec![Pick::include("adventure"), Pick::include("1x").any_of()];
    let sent = query_to_api(&query);
    assert_eq!(sent.kind, api::ProjectKind::Mod);
    assert_eq!(sent.sort, api::Sort::Downloads);
    assert_eq!(sent.page_size, 100, "bounded like the Modrinth page");
    assert!(sent.client && !sent.server);
    // Included loaders were always "any of"; blank picks are dropped.
    assert_eq!(sent.loaders.len(), 2);
    assert!(sent.loaders[0].any && sent.loaders[0].stance == api::Stance::Include);
    assert!(!sent.loaders[1].any && sent.loaders[1].stance == api::Stance::Exclude);
    assert!(!sent.categories[0].any && sent.categories[1].any);
    let capabilities = lumilio_plugin_modrinth::Modrinth;
    let capabilities = lumilio_plugin_api::ContentSource::capabilities(&capabilities);
    sent.validate(&capabilities)
        .expect("the stock source accepts what the page can ask for");
}

#[test]
fn loaders_and_environment_only_count_for_kinds_that_have_them() {
    let mut query = SearchQuery::new(ProjectKind::ResourcePack);
    query.loaders = vec![Pick::include("fabric")];
    query.client = true;
    query.server = true;
    let sent = query_to_api(&query);
    assert!(sent.loaders.is_empty());
    assert!(!sent.client && !sent.server);
    assert_eq!(query.open_source, None);
    query.open_source = Some(Stance::Exclude);
    assert_eq!(query_to_api(&query).open_source, Some(api::Stance::Exclude));
}

#[test]
fn a_loaders_other_project_types_stay_visible_to_the_filters() {
    let tag = loader_from_api(api::LoaderTag {
        name: "sponge".into(),
        kinds: vec![api::ProjectKind::Mod],
        other_kinds: vec!["plugin".into()],
    });
    assert_eq!(tag.project_types, ["mod", "plugin"]);
}

fn game(version: &str, kind: &str, date: &str) -> super::tags::GameVersionTag {
    super::tags::GameVersionTag {
        version: version.to_owned(),
        release: kind == "release",
        snapshot: kind == "snapshot",
        published: date.to_owned(),
    }
}

fn labels(groups: &[super::version_groups::VersionGroup]) -> Vec<&str> {
    groups.iter().map(|group| group.label.as_str()).collect()
}

#[test]
fn game_versions_collapse_into_ranges_like_modrinth_app() {
    use super::version_groups::version_groups;
    // Newest first, as Modrinth lists them.
    let all = vec![
        game("1.21-pre1", "snapshot", "2024-06-01"),
        game("1.21", "release", "2024-06-13"),
        game("1.20.6", "release", "2024-04-29"),
        game("1.20.5", "release", "2024-04-23"),
        game("1.20.4", "release", "2023-12-07"),
        game("1.20.2", "release", "2023-09-21"),
        game("1.20.1", "release", "2023-06-12"),
        game("1.20", "release", "2023-06-07"),
        game("b1.8.1", "beta", "2011-09-18"),
    ];
    let of = |names: &[&str]| {
        let owned: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
        version_groups(&owned, &all)
    };
    // Every 1.20 patch the game has: "1.20.x"; a gap in the project's own list splits it.
    assert_eq!(
        labels(&of(&[
            "1.20", "1.20.1", "1.20.2", "1.20.4", "1.20.5", "1.20.6"
        ])),
        ["1.20.4–1.20.6", "1.20–1.20.2"]
    );
    assert_eq!(
        labels(&of(&[
            "1.20", "1.20.1", "1.20.2", "1.20.4", "1.20.5", "1.20.6", "1.21"
        ])),
        ["1.21", "1.20.4–1.20.6", "1.20–1.20.2"],
    );
    assert_eq!(labels(&of(&["1.20.1"])), ["1.20.1"]);
    // A snapshot newer than the newest supported release is named first.
    assert_eq!(
        labels(&of(&["1.21-pre1", "1.20.1"])),
        ["1.21-pre1", "1.20.1"]
    );
    // A project with only a snapshot shows it; legacy versions follow in a range.
    assert_eq!(labels(&of(&["1.21-pre1"])), ["1.21-pre1"]);
    assert_eq!(labels(&of(&["1.20.1", "b1.8.1"])), ["1.20.1", "b1.8.1"]);
}
