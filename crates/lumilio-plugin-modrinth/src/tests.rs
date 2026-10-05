use super::project::decode_project;
use super::search::decode_search;
use super::tags::{
    decode_categories, decode_game_versions, decode_owner, decode_project_summaries,
    decode_team_authors, encoded_list,
};
use super::versions::decode_versions;
use super::*;
use url::Url;

mod calls;

fn query(kind: ProjectKind) -> SearchQuery {
    SearchQuery {
        text: String::new(),
        kind,
        sort: Sort::Relevance,
        page: 0,
        page_size: 20,
        game_versions: Vec::new(),
        loaders: Vec::new(),
        categories: Vec::new(),
        client: false,
        server: false,
        open_source: None,
        hidden_projects: Vec::new(),
        excluded_disclosures: Vec::new(),
        excluded_types: Vec::new(),
    }
}

fn include(name: &str) -> Pick {
    Pick {
        name: name.into(),
        stance: Stance::Include,
        any: false,
    }
}

fn exclude(name: &str) -> Pick {
    Pick {
        stance: Stance::Exclude,
        ..include(name)
    }
}

trait AnyOf {
    fn any_of(self) -> Self;
}

impl AnyOf for Pick {
    fn any_of(mut self) -> Self {
        self.any = true;
        self
    }
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

fn pairs(query: &SearchQuery) -> BTreeMap<String, String> {
    Url::parse(&query.url())
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

#[test]
fn search_url_carries_filters_paging_and_sort() {
    let mut query = query(ProjectKind::Mod);
    query.text = " sodium ".to_owned();
    query.game_versions = vec!["1.21.1".to_owned()];
    query.loaders = vec![
        include("fabric").any_of(),
        include(" quilt ").any_of(),
        include(""),
    ];
    query.categories = vec![include("optimization"), include("lightweight")];
    query.sort = Sort::Downloads;
    query.page = 2;
    query.page_size = 10;
    let url = Url::parse(&query.url()).unwrap();
    assert_eq!(url.path(), "/v3/search");
    let pairs = pairs(&query);
    assert_eq!(pairs["query"], "sodium");
    assert_eq!(pairs["offset"], "20");
    assert_eq!(pairs["limit"], "10");
    assert_eq!(pairs["index"], "downloads");
    assert_eq!(
        pairs["new_filters"],
        "categories = `optimization` AND categories = `lightweight` \
         AND game_versions = `1.21.1` AND categories IN [`fabric`, ` quilt `] \
         AND project_types = `mod`"
    );
}

#[test]
fn the_first_page_and_empty_text_send_neither_offset_nor_query() {
    let pairs = pairs(&query(ProjectKind::Shader));
    assert!(!pairs.contains_key("offset"));
    assert!(!pairs.contains_key("query"));
    assert_eq!(pairs["new_filters"], "project_types = `shader`");
}

#[test]
fn loaders_skip_kinds_without_them_and_page_size_is_bounded() {
    let mut query = query(ProjectKind::ResourcePack);
    query.loaders = vec![include("fabric")];
    query.page_size = 5000;
    assert_eq!(query.expression(), "project_types = `resourcepack`");
    assert_eq!(pairs(&query)["limit"], "100");
    query.kind = ProjectKind::Shader;
    assert_eq!(
        query.expression(),
        "categories = `fabric` AND project_types = `shader`"
    );
}

#[test]
fn excluded_picks_collect_into_one_not_in_clause() {
    let mut query = query(ProjectKind::Mod);
    query.categories = vec![exclude("cursed"), include("magic")];
    query.loaders = vec![exclude("forge"), include("fabric")];
    query.open_source = Some(Stance::Exclude);
    query.hidden_projects = vec!["AANobbMI".to_owned(), "P7dR8mSH".to_owned()];
    query.excluded_disclosures = vec!["epilepsy_triggers".to_owned()];
    query.excluded_types = vec!["plugin".to_owned(), "datapack".to_owned()];
    assert_eq!(
        query.expression(),
        "categories = `magic` AND categories = `fabric` \
         AND categories NOT IN [`cursed`, `forge`] AND open_source NOT IN [true] \
         AND project_id NOT IN [`AANobbMI`, `P7dR8mSH`] \
         AND disclosure_types NOT IN [`epilepsy_triggers`] \
         AND project_types = `mod` AND all_project_types NOT IN [`plugin`, `datapack`]"
    );
}

#[test]
fn any_of_categories_match_when_one_does_and_open_source_includes() {
    let mut query = query(ProjectKind::ResourcePack);
    query.categories = vec![
        include("16x").any_of(),
        include("32x").any_of(),
        include("themed"),
    ];
    query.open_source = Some(Stance::Include);
    assert_eq!(
        query.expression(),
        "categories = `themed` AND categories IN [`16x`, `32x`] AND open_source = true \
         AND project_types = `resourcepack`"
    );
}

#[test]
fn environment_asks_for_the_values_that_work_on_that_side() {
    let mut query = query(ProjectKind::Mod);
    query.client = true;
    assert_eq!(
        query.expression(),
        "(environment = `client_only` OR environment = `client_only_server_optional` \
         OR environment = `client_or_server_prefers_both` OR environment = `client_or_server`) \
         AND project_types = `mod`"
    );
    query.client = false;
    query.server = true;
    assert!(
        query
            .expression()
            .contains("environment = `dedicated_server_only`")
    );
    query.client = true;
    assert!(
        query
            .expression()
            .contains("environment = `client_and_server`")
    );
    assert!(!query.expression().contains("`dedicated_server_only`"));
    // Resource packs have no environment to ask about.
    query.kind = ProjectKind::ResourcePack;
    assert_eq!(query.expression(), "project_types = `resourcepack`");
}

#[test]
fn decodes_hits_and_drops_unusable_ones() {
    let page = decode_search(
        br#"{"total_hits": 45, "hits": [
            {"project_id":"A","slug":"sodium","name":"Sodium","author":"jelly",
             "project_types":["mod"],"categories":["fabric","optimization"],
             "display_categories":["optimization"],"loaders":["fabric"],
             "downloads":9,"icon_url":""},
            {"project_id":"B","name":"Weird","project_types":["plugin"]},
            {"project_types":["mod"],"name":"No id"}
        ]}"#,
    )
    .unwrap();
    assert_eq!(page.hits.len(), 1);
    let hit = &page.hits[0];
    assert_eq!(hit.categories, ["optimization"]);
    assert_eq!(hit.loaders, ["fabric"]);
    assert_eq!(hit.icon_url, None);
    assert_eq!(hit.page_url, "https://modrinth.com/mod/sodium");
}

#[test]
fn decodes_projects_and_rejects_unsupported_ones() {
    let project = decode_project(
        br##"{"id":"A","slug":"s","title":"T","project_type":"resourcepack","body":"# hi"}"##,
    )
    .unwrap();
    assert_eq!(project.kind, ProjectKind::ResourcePack);
    assert_eq!(project.page_url, "https://modrinth.com/resourcepack/s");
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
    let required: Vec<_> = versions[0]
        .dependencies
        .iter()
        .filter(|dependency| dependency.kind == DependencyKind::Required)
        .collect();
    assert_eq!(required.len(), 1);
}

#[test]
fn hits_carry_stats_environment_and_the_packs_own_loaders() {
    let page = decode_search(
        br#"{"total_hits": 1, "hits": [
            {"project_id":"A","slug":"fo","name":"FO","author":"me","organization":"The Team",
             "project_types":["modpack"],"loaders":["mrpack"],
             "categories":["fabric","lightweight","multiplayer","optimization"],
             "display_categories":["lightweight","multiplayer"],
             "downloads":17731400,"follows":4886,
             "date_created":"2022-02-10T06:28:19Z","date_modified":"2026-09-27T10:00:00Z",
             "project_loader_fields":{"environment":["client_only"],"mrpack_loaders":["fabric"]}}]}"#,
    )
    .unwrap();
    let hit = &page.hits[0];
    assert_eq!(
        hit.author, "The Team",
        "an organization stands for its projects"
    );
    assert_eq!(hit.categories, ["lightweight", "multiplayer"]);
    assert_eq!(
        hit.all_categories,
        ["lightweight", "multiplayer", "optimization"]
    );
    assert_eq!(hit.loaders, ["fabric"]);
    assert_eq!((hit.downloads, hit.follows), (17_731_400, 4886));
    assert_eq!(hit.published, "2022-02-10T06:28:19Z");
    assert_eq!(hit.updated, "2026-09-27T10:00:00Z");
    assert_eq!(hit.environment, Some(Environment::ClientOnly));
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
              {"url":"https://cdn/a_350.webp","raw_url":"https://cdn/a.png","title":"A","description":"first","featured":true,"ordering":5},
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
    // The preview is for the grid, the original for the large view; a missing
    // original falls back to the preview.
    assert_eq!(project.gallery[0].url, "https://cdn/a_350.webp");
    assert_eq!(project.gallery[0].full_url, "https://cdn/a.png");
    assert_eq!(project.gallery[1].full_url, project.gallery[1].url);
    let order: Vec<_> = project
        .gallery
        .iter()
        .map(|i| i.full_url.as_str())
        .collect();
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
