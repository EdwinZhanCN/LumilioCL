use super::client::ModrinthClient;
use super::error::DiscoverError;
use super::intent::{IntentError, install_request};
use super::kinds::{ProjectKind, SortIndex, browse_page_url};
use super::project::decode_project;
use super::search::{
    Environment, SearchPage, SearchQuery, SideSupport, decode_search, environment,
};
use super::tags::{
    decode_categories, decode_game_versions, decode_owner, decode_project_summaries,
    decode_team_authors, encoded_list,
};
use super::versions::{DependencyKind, ReleaseChannel, Version, decode_versions, pick_version};
use super::{API_BASE, SITE_BASE};
use crate::instance::Loader;
use crate::transfer::Transport;
use std::path::Path;
use url::Url;

use crate::transfer::{TransportFuture, TransportResponse};
use std::collections::BTreeMap;

const SHA1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn version(id: &str, channel: &str, published: &str, games: &[&str], loaders: &[&str]) -> String {
    format!(
        r#"{{"id":"{id}","project_id":"P","name":"n","version_number":"{id}",
            "version_type":"{channel}","game_versions":{games:?},"loaders":{loaders:?},
            "date_published":"{published}",
            "files":[{{"url":"https://cdn/x/{id}.jar","filename":"{id}.jar","primary":true,
                       "size":10,"hashes":{{"sha1":"{SHA1}"}}}}],
            "dependencies":[]}}"#
    )
}

fn versions() -> Vec<Version> {
    let body = format!(
        "[{}]",
        [
            version(
                "old-release",
                "release",
                "2024-01-01T00:00:00Z",
                &["1.21.1"],
                &["fabric"]
            ),
            version(
                "new-release",
                "release",
                "2024-06-01T00:00:00Z",
                &["1.21.1"],
                &["fabric"]
            ),
            version(
                "newest-beta",
                "beta",
                "2024-09-01T00:00:00Z",
                &["1.21.1"],
                &["fabric"]
            ),
            version(
                "forge-only",
                "release",
                "2024-12-01T00:00:00Z",
                &["1.21.1"],
                &["forge"]
            ),
            version(
                "other-game",
                "release",
                "2025-01-01T00:00:00Z",
                &["1.20.4"],
                &["fabric"]
            ),
        ]
        .join(",")
    );
    decode_versions(body.as_bytes()).unwrap()
}

#[test]
fn summaries_and_team_authors_decode_with_fallbacks() {
    let summaries = decode_project_summaries(
        br#"[{"id":"AANobbMI","slug":"sodium","title":"Sodium","project_type":"mod",
              "icon_url":"https://x/i.webp","team":"4reLOAKe"},
             {"id":"","title":"dropped"},
             {"id":"B","project_type":"future","icon_url":""}]"#,
    )
    .unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].kind, Some(ProjectKind::Mod));
    assert_eq!(
        summaries[1].slug, "B",
        "the id stands in for a missing slug"
    );
    assert_eq!(summaries[1].icon_url, None);
    let authors = decode_team_authors(
        br#"[[{"role":"Original Author","team_id":"t1","user":{"username":"jelly"}},
              {"role":"Owner","team_id":"t1","user":{"username":"lead"}}],
             [{"role":"Developer","team_id":"t2","user":{"username":"solo"}}],
             [{"role":"Owner","team_id":"t3"}]]"#,
    )
    .unwrap();
    assert_eq!(authors["t1"], "lead", "the owner wins");
    assert_eq!(authors["t2"], "solo", "else the first member");
    assert!(!authors.contains_key("t3"), "no name, no author");
    assert_eq!(encoded_list(&["a b".to_owned()]), "%5B%22a+b%22%5D");
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
fn search_url_carries_facets_paging_and_sort() {
    let mut query = SearchQuery::new(ProjectKind::Mod);
    query.text = " sodium ".to_owned();
    query.game_version = Some("1.21.1".to_owned());
    query.loaders = vec!["fabric".to_owned(), " quilt ".to_owned(), String::new()];
    query.categories = vec!["optimization".to_owned(), "lightweight".to_owned()];
    query.sort = SortIndex::Downloads;
    query.page = 2;
    query.page_size = 10;
    let url = Url::parse(&query.url()).unwrap();
    assert_eq!(url.path(), "/v2/search");
    let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(pairs["query"], "sodium");
    assert_eq!(pairs["offset"], "20");
    assert_eq!(pairs["limit"], "10");
    assert_eq!(pairs["index"], "downloads");
    assert_eq!(
        pairs["facets"],
        r#"[["project_type:mod"],["versions:1.21.1"],["categories:fabric","categories:quilt"],["categories:optimization"],["categories:lightweight"]]"#
    );
}

#[test]
fn loaders_only_filter_mods_and_modpacks_and_page_size_is_bounded() {
    let mut query = SearchQuery::new(ProjectKind::Shader);
    query.loaders = vec!["fabric".to_owned()];
    query.page_size = 5000;
    let url = Url::parse(&query.url()).unwrap();
    let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(pairs["facets"], r#"[["project_type:shader"]]"#);
    assert_eq!(pairs["limit"], "100");
}

#[test]
fn decodes_hits_and_drops_unusable_ones() {
    let page = decode_search(
        br#"{"total_hits": 45, "hits": [
            {"project_id":"A","slug":"sodium","title":"Sodium","author":"jelly",
             "project_type":"mod","categories":["fabric","optimization"],
             "display_categories":["optimization"],"downloads":9,"icon_url":""},
            {"project_id":"B","title":"Weird","project_type":"plugin"},
            {"project_type":"mod","title":"No id"}
        ]}"#,
    )
    .unwrap();
    assert_eq!(page.hits.len(), 1);
    let hit = &page.hits[0];
    assert_eq!(hit.categories, ["optimization"]);
    assert_eq!(hit.icon_url, None);
    assert_eq!(hit.page_url(), "https://modrinth.com/mod/sodium");
    assert_eq!(page.page_count(20), 3);
    assert_eq!(SearchPage::default().page_count(20), 0);
}

#[test]
fn decodes_projects_and_rejects_unsupported_ones() {
    let project = decode_project(
        br##"{"id":"A","slug":"s","title":"T","project_type":"resourcepack","body":"# hi"}"##,
    )
    .unwrap();
    assert_eq!(project.kind, ProjectKind::ResourcePack);
    assert_eq!(project.page_url(), "https://modrinth.com/resourcepack/s");
    assert!(decode_project(br#"{"id":"A","project_type":"plugin"}"#).is_err());
    assert!(decode_project(b"nope").is_err());
}

#[test]
fn versions_without_files_are_dropped_and_unknown_dependency_kinds_survive() {
    let versions = decode_versions(
        br#"[
            {"id":"empty","files":[]},
            {"id":"v","project_id":"P","version_type":"strange",
             "files":[{"url":"u","filename":"f.jar","hashes":{}}],
             "dependencies":[{"project_id":"D","dependency_type":"required"},
                             {"project_id":"E","dependency_type":"future"}]}
        ]"#,
    )
    .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].channel, ReleaseChannel::Release);
    assert_eq!(versions[0].dependencies[1].kind, DependencyKind::Other);
    let required: Vec<_> = versions[0].required_dependencies().collect();
    assert_eq!(required.len(), 1);
}

#[test]
fn the_primary_file_wins_over_the_first() {
    let version = decode_versions(
        br#"[{"id":"v","files":[
            {"url":"u1","filename":"sources.jar","primary":false,"hashes":{}},
            {"url":"u2","filename":"mod.jar","primary":true,"hashes":{}}]}]"#,
    )
    .unwrap()
    .remove(0);
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
    let list = format!(
        "[{}]",
        version(
            "rp",
            "release",
            "2024-01-01T00:00:00Z",
            &["1.21.1"],
            &["minecraft"]
        )
    );
    let all = decode_versions(list.as_bytes()).unwrap();
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

struct Routes(BTreeMap<String, Vec<u8>>);

impl Transport for Routes {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            match self.0.get(source) {
                Some(body) => Ok(TransportResponse::from_bytes(200, body.clone())),
                None => Ok(TransportResponse::from_bytes(404, Vec::new())),
            }
        })
    }
}

#[tokio::test]
async fn the_client_hits_the_documented_endpoints() {
    let mut query = SearchQuery::new(ProjectKind::Mod);
    query.text = "x".to_owned();
    let mut routes = BTreeMap::new();
    routes.insert(
        query.url().replacen(API_BASE, "http://mock", 1),
        br#"{"total_hits":1,"hits":[{"project_id":"A","title":"T","project_type":"mod"}]}"#
            .to_vec(),
    );
    routes.insert(
        "http://mock/v2/project/A".to_owned(),
        br#"{"id":"A","project_type":"mod","title":"T"}"#.to_vec(),
    );
    routes.insert(
        "http://mock/v2/project/A/version?include_changelog=false".to_owned(),
        format!(
            "[{}]",
            version(
                "v1",
                "release",
                "2024-01-01T00:00:00Z",
                &["1.21.1"],
                &["fabric"]
            )
        )
        .into_bytes(),
    );
    let client = ModrinthClient::new(Routes(routes)).with_base("http://mock/");
    assert_eq!(client.search(&query).await.unwrap().hits.len(), 1);
    assert_eq!(client.project("A").await.unwrap().title, "T");
    assert_eq!(client.versions("A").await.unwrap().len(), 1);
    assert!(matches!(
        client.project("missing").await,
        Err(DiscoverError::Fetch(_))
    ));
}

#[test]
fn modpacks_filter_by_loader_too() {
    let mut query = SearchQuery::new(ProjectKind::Modpack);
    query.loaders = vec!["neoforge".to_owned()];
    let url = Url::parse(&query.url()).unwrap();
    let pairs: BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(
        pairs["facets"],
        r#"[["project_type:modpack"],["categories:neoforge"]]"#
    );
}

#[test]
fn hits_carry_stats_environment_and_split_loaders_from_categories() {
    let page = decode_search(
        br#"{"total_hits": 1, "hits": [
            {"project_id":"A","slug":"fo","title":"FO","author":"me","project_type":"modpack",
             "display_categories":["lightweight","fabric","multiplayer"],
             "downloads":17731400,"follows":4886,"date_modified":"2026-09-27T10:00:00Z",
             "client_side":"required","server_side":"optional"}]}"#,
    )
    .unwrap();
    let hit = &page.hits[0];
    assert_eq!(hit.categories, ["lightweight", "multiplayer"]);
    assert_eq!(hit.loaders, ["fabric"]);
    assert_eq!((hit.downloads, hit.follows), (17_731_400, 4886));
    assert_eq!(hit.updated, "2026-09-27T10:00:00Z");
    assert_eq!(
        environment(hit.client_side, hit.server_side),
        Some(Environment::ClientAndServer)
    );
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
        Some(Environment::ClientAndServer)
    );
    // Not knowing is not the same as not running there.
    assert_eq!(environment(Unknown, Unknown), None);
    assert_eq!(environment(Required, Unknown), None);
    assert_eq!(environment(Unsupported, Unsupported), None);
}

#[test]
fn a_project_decodes_gallery_links_license_and_dates() {
    let project = decode_project(
        br##"{"id":"P","slug":"sodium","title":"Sodium","description":"fast","body":"# hi",
            "project_type":"mod","categories":["optimization"],"loaders":["fabric"],
            "downloads":5,"followers":9,"published":"2020-01-01T00:00:00Z",
            "updated":"2026-01-01T00:00:00Z","client_side":"required","server_side":"unsupported",
            "license":{"id":"LGPL-3.0","name":"GNU LGPL v3"},
            "source_url":"https://github.com/x/y","issues_url":"","wiki_url":null,
            "gallery":[
              {"url":"https://cdn/b.png","title":"B","featured":false,"ordering":1},
              {"url":"","title":"no address"},
              {"url":"https://cdn/a.png","title":"A","description":"first","featured":true,"ordering":5},
              {"url":"https://cdn/c.png","featured":false,"ordering":0}]}"##,
    )
    .unwrap();
    assert_eq!(project.followers, 9);
    assert_eq!(project.license.as_deref(), Some("GNU LGPL v3"));
    assert_eq!(
        project.links.source.as_deref(),
        Some("https://github.com/x/y")
    );
    assert_eq!(project.links.issues, None, "an empty address is no link");
    assert_eq!(project.links.wiki, None);
    let order: Vec<_> = project.gallery.iter().map(|i| i.url.as_str()).collect();
    assert_eq!(
        order,
        [
            "https://cdn/a.png",
            "https://cdn/c.png",
            "https://cdn/b.png"
        ],
        "featured first, then the author's order; unaddressed images dropped"
    );
    assert_eq!(project.gallery[0].description, "first");
}

#[test]
fn tags_decode_and_unsupported_types_are_dropped() {
    let categories = decode_categories(
        br#"[{"icon":"<svg/>","name":"adventure","project_type":"modpack","header":"categories"},
             {"name":"plugin-thing","project_type":"plugin","header":"categories"},
             {"name":"","project_type":"mod","header":"categories"},
             {"name":"lightweight","project_type":"mod","header":"categories"}]"#,
    )
    .unwrap();
    assert_eq!(categories.len(), 2);
    assert_eq!(categories[0].kind, ProjectKind::Modpack);
    assert_eq!(categories[1].name, "lightweight");

    let versions = decode_game_versions(
        br#"[{"version":"26.3","version_type":"release","date":"2026-09-01T00:00:00Z","major":true},
             {"version":"26.4-pre1","version_type":"snapshot","date":"2026-09-20T00:00:00Z"},
             {"version":"","version_type":"release"}]"#,
    )
    .unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions[0].release);
    assert!(!versions[1].release);
}

#[test]
fn the_owner_is_the_member_with_the_owner_role() {
    let owner = decode_owner(
        br#"[{"role":"Member","user":{"username":"helper"}},
             {"role":"Owner","user":{"username":"jelly"}}]"#,
    )
    .unwrap();
    assert_eq!(owner.as_deref(), Some("jelly"));
    assert_eq!(decode_owner(b"[]").unwrap(), None);
}
