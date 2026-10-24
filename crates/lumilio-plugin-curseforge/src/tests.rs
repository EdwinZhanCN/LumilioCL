use std::sync::Mutex;

use lumilio_plugin_api::{FetchResponse, SettingValue};
use url::Url;

use super::*;

/// A host that answers by full URL and remembers what was asked.
#[derive(Default)]
struct Context {
    routes: BTreeMap<String, FetchResponse>,
    requests: Mutex<Vec<FetchRequest>>,
    locale: Option<&'static str>,
    fail: Option<PluginError>,
}

impl Context {
    fn answer(mut self, path: &str, status: u16, body: &str) -> Self {
        self.routes.insert(
            format!("{API_BASE}{path}"),
            FetchResponse {
                status,
                body: body.as_bytes().to_vec(),
            },
        );
        self
    }

    fn english(mut self) -> Self {
        self.locale = Some("en");
        self
    }

    fn requests(&self) -> Vec<FetchRequest> {
        self.requests.lock().unwrap().clone()
    }

    fn paths(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .map(|request| request.url.trim_start_matches(API_BASE).to_owned())
            .collect()
    }
}

impl HostContext for Context {
    fn locale(&self) -> &str {
        self.locale.unwrap_or("zh-CN")
    }
    fn setting(&self, _: &str) -> Option<SettingValue> {
        None
    }
    fn read_file(&self, _: &str) -> Result<Vec<u8>, PluginError> {
        panic!("content source must not read files")
    }
    fn list_files(&self, _: &str) -> Result<Vec<String>, PluginError> {
        panic!("content source must not list files")
    }
    fn fetch(&self, url: &str) -> Result<FetchResponse, PluginError> {
        self.request(FetchRequest::get(url))
    }
    fn request(&self, request: FetchRequest) -> Result<FetchResponse, PluginError> {
        self.requests.lock().unwrap().push(request.clone());
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(self
            .routes
            .get(&request.url)
            .cloned()
            .unwrap_or(FetchResponse {
                status: 404,
                body: Vec::new(),
            }))
    }
}

fn keyed() -> CurseForge {
    CurseForge::with_key(Some("test-key"))
}

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

fn include(name: &str, any: bool) -> Pick {
    Pick {
        name: name.into(),
        stance: Stance::Include,
        any,
    }
}

fn pairs(path: &str) -> BTreeMap<String, String> {
    Url::parse(&format!("{API_BASE}{path}"))
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

/// JEI, trimmed: a mod with a blocked distribution flag flipped per test.
fn jei(allow: bool) -> String {
    format!(
        r#"{{"id":238222,"name":"Just Enough Items (JEI)","summary":"View Items and Recipes",
        "links":{{"websiteUrl":"https://www.curseforge.com/minecraft/mc-mods/jei","sourceUrl":"https://github.com/mezz/JustEnoughItems","wikiUrl":"","issuesUrl":null}},
        "downloadCount":634691435.0,"classId":6,"allowModDistribution":{allow},
        "categories":[{{"id":423,"name":"Map and Information","classId":6,"isClass":false}},{{"id":6484,"name":"Create","classId":6}}],
        "authors":[{{"name":"mezz"}}],
        "logo":{{"title":"l","description":"","thumbnailUrl":"https://media.forgecdn.net/t.jpeg","url":"https://media.forgecdn.net/l.jpeg"}},
        "screenshots":[{{"title":"Shot","description":"d","thumbnailUrl":"https://media.forgecdn.net/s-t.png","url":"https://media.forgecdn.net/s.png"}}],
        "latestFilesIndexes":[
          {{"gameVersion":"1.20.1","fileId":200,"releaseType":1,"modLoader":1}},
          {{"gameVersion":"1.20.1","fileId":300,"releaseType":2,"modLoader":1}},
          {{"gameVersion":"1.20.1","fileId":250,"releaseType":1,"modLoader":4}},
          {{"gameVersion":"1.21.1","fileId":400,"releaseType":1,"modLoader":6}}],
        "dateCreated":"2015-11-23T23:00:41.923Z","dateReleased":"2026-10-12T14:05:52.453Z","dateModified":"2026-10-12T14:09:55.927Z"}}"#
    )
}

fn file(id: u64, mod_id: u64, extra: &str) -> String {
    format!(
        r#"{{"id":{id},"modId":{mod_id},"displayName":"{id} for Forge","fileName":"f-{id}.jar","releaseType":1,
        "hashes":[{{"value":"E514CD6BC50B816487D9C25CD457DE5E0BD5B07F","algo":1}},{{"value":"872176fd31ee2615bfed17d1ba3765c8","algo":2}}],
        "fileDate":"2026-10-04T12:38:50.89Z","fileLength":2232177,"downloadCount":3.0,
        "downloadUrl":"https://edge.forgecdn.net/files/{id}/f-{id}.jar",
        "gameVersions":["Client","1.20.1","Forge","Server"],
        "sortableGameVersions":[{{"gameVersionName":"Client","gameVersion":""}},{{"gameVersionName":"1.20.1","gameVersion":"1.20.1"}},{{"gameVersionName":"Forge","gameVersion":""}}],
        "dependencies":[{{"modId":1,"relationType":3}},{{"modId":2,"relationType":2}},{{"modId":3,"relationType":5}},{{"modId":4,"relationType":6}}],
        "fileFingerprint":4164405417{extra}}}"#
    )
}

#[test]
fn manifest_needs_a_key_to_be_on_and_grants_only_the_api_host() {
    let manifest = keyed().manifest();
    assert_eq!(manifest.id, ID);
    assert_eq!(manifest.api, API_VERSION);
    assert!(manifest.default_enabled);
    assert_eq!(
        manifest.permissions,
        vec![Permission::Network {
            hosts: vec!["api.curseforge.com".into()]
        }]
    );
    let keyless = CurseForge::with_key(Some("  ")).manifest();
    assert!(!keyless.default_enabled, "a blank key is no key");
    assert!(keyless.description.get("zh-CN").contains("密钥"));
    assert!(keyless.description.get("en").contains("API key"));
}

#[test]
fn capabilities_speak_curseforge_ids_html_and_murmur2() {
    let capabilities = keyed().capabilities();
    assert_eq!(capabilities.catalog, Some(Catalog::CurseForge));
    assert_eq!(capabilities.text_format, TextFormat::Html);
    assert_eq!(capabilities.fingerprints, vec![FingerprintKind::Murmur2]);
    let mods = &capabilities.kinds[0];
    assert!(
        !mods.sorts.contains(&Sort::Follows),
        "CurseForge counts no followers"
    );
    assert_eq!(
        mods.browse_url.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods")
    );
    let shaders = capabilities
        .kinds
        .iter()
        .find(|kind| kind.kind == ProjectKind::Shader)
        .unwrap();
    assert!(
        shaders.filters.loaders.is_none(),
        "shaders cannot be filtered by loader"
    );
    let mut excluded = query(ProjectKind::Mod);
    excluded.categories = vec![Pick {
        stance: Stance::Exclude,
        ..include("423", false)
    }];
    assert!(excluded.validate(&capabilities).is_err(), "no exclusions");
}

#[test]
fn requests_carry_the_key_and_ask_for_json() {
    let context = Context::default().answer(
        "/v1/mods/238222",
        200,
        &format!(r#"{{"data":{}}}"#, jei(true)),
    );
    keyed().project(&context, "238222").unwrap();
    let request = &context.requests()[0];
    assert!(
        request
            .headers
            .contains(&("x-api-key".into(), "test-key".into()))
    );
    assert!(
        request
            .headers
            .contains(&("accept".into(), "application/json".into()))
    );

    let context = Context::default();
    let error = CurseForge::with_key(None)
        .project(&context.answer("/v1/mods/1", 403, ""), "1")
        .unwrap_err();
    assert!(
        matches!(&error, PluginError::Transient(message) if message.contains("API key")),
        "a refused keyless build says why without stopping the plugin: {error:?}"
    );
}

#[test]
fn network_failures_fail_the_call_not_the_plugin() {
    let context = Context {
        fail: Some(PluginError::Unavailable(
            "plugin network deadline expired".into(),
        )),
        ..Context::default()
    };
    assert!(matches!(
        keyed().project(&context, "1"),
        Err(PluginError::Transient(_))
    ));
    let context = Context {
        fail: Some(PluginError::PermissionDenied),
        ..Context::default()
    };
    assert_eq!(
        keyed().project(&context, "1"),
        Err(PluginError::PermissionDenied)
    );
    assert!(
        matches!(
            keyed().project(&Context::default(), "sodium"),
            Err(PluginError::Transient(_))
        ),
        "another source's ID is refused without stopping the plugin"
    );
    let garbled = Context::default().answer("/v1/mods/1", 200, "<html>");
    assert!(matches!(
        keyed().project(&garbled, "1"),
        Err(PluginError::Transient(_))
    ));
}

#[test]
fn search_path_carries_kind_sort_paging_and_filters() {
    let mut search = query(ProjectKind::Mod);
    search.text = " create ".into();
    search.game_versions = vec!["1.20.1".into()];
    search.loaders = vec![include("forge", true), include("neoforge", true)];
    search.categories = vec![include("423", false), include("6484", false)];
    let pairs = pairs(&query::search_path(&search, 40, 20));
    assert_eq!(pairs["gameId"], "432");
    assert_eq!(pairs["classId"], "6");
    assert_eq!(pairs["sortField"], "1", "relevance with words");
    assert_eq!(pairs["searchFilter"], "create");
    assert_eq!(
        (pairs["index"].as_str(), pairs["pageSize"].as_str()),
        ("40", "20")
    );
    assert_eq!(pairs["gameVersion"], "1.20.1");
    assert_eq!(pairs["modLoaderTypes"], "[1,6]");
    assert_eq!(pairs["categoryIds"], "[423,6484]");

    let mut browse = query(ProjectKind::ResourcePack);
    browse.game_versions = vec!["1.20.1".into(), "1.21.1".into()];
    browse.categories = vec![include("393", false)];
    let pairs = super::tests::pairs(&query::search_path(&browse, 0, 20));
    assert_eq!(pairs["classId"], "12");
    assert_eq!(pairs["sortField"], "2", "popularity without words");
    assert_eq!(pairs["gameVersions"], r#"["1.20.1","1.21.1"]"#);
    assert_eq!(pairs["categoryId"], "393");
    assert!(!pairs.contains_key("searchFilter"));

    for (sort, field) in [
        (Sort::Downloads, "6"),
        (Sort::Newest, "11"),
        (Sort::Updated, "3"),
    ] {
        let mut sorted = query(ProjectKind::Shader);
        sorted.sort = sort;
        assert_eq!(
            super::tests::pairs(&query::search_path(&sorted, 0, 5))["sortField"],
            field
        );
    }
}

#[test]
fn a_large_page_is_read_fifty_at_a_time_and_totals_stop_at_ten_thousand() {
    let hits = |count: usize| {
        let hit = jei(true);
        let list = vec![hit; count].join(",");
        format!(r#"{{"data":[{list}],"pagination":{{"totalCount":123456}}}}"#)
    };
    let mut search = query(ProjectKind::Mod);
    search.page = 1;
    search.page_size = 100;
    let first = query::search_path(&search, 100, 50);
    let second = query::search_path(&search, 150, 50);
    let context = Context::default()
        .answer(&first, 200, &hits(50))
        .answer(&second, 200, &hits(50));
    let page = keyed().search(&context, &search).unwrap();
    assert_eq!(page.hits.len(), 100);
    assert_eq!(page.total_hits, 10_000);
    assert_eq!(context.paths(), vec![first, second]);

    search.page = 100;
    let context = Context::default();
    let page = keyed().search(&context, &search).unwrap();
    assert!(page.hits.is_empty());
    assert!(
        context.requests().is_empty(),
        "past the limit nothing is asked"
    );

    search.page = 99;
    assert_eq!(query::window(&search), Some((9900, 100)));
}

#[test]
fn search_hits_name_categories_in_the_launchers_language() {
    let body = format!(
        r#"{{"data":[{},{{"id":9,"classId":17}}],"pagination":{{"totalCount":2}}}}"#,
        jei(false)
    );
    let path = query::search_path(&query(ProjectKind::Mod), 0, 20);
    let context = Context::default().answer(&path, 200, &body);
    let page = keyed().search(&context, &query(ProjectKind::Mod)).unwrap();
    assert_eq!(page.hits.len(), 1, "a world is not one of the four kinds");
    let hit = &page.hits[0];
    assert_eq!(
        (hit.project_id.as_str(), hit.slug.as_str()),
        ("238222", "238222")
    );
    assert_eq!(hit.kind, ProjectKind::Mod);
    assert_eq!(hit.author, "mezz");
    assert_eq!(hit.categories, vec!["地图与信息", "Create"]);
    assert_eq!(hit.loaders, vec!["fabric", "forge", "neoforge"]);
    assert_eq!(hit.downloads, 634_691_435);
    assert_eq!(hit.follows, None);
    assert_eq!(
        hit.page_url,
        "https://www.curseforge.com/minecraft/mc-mods/jei"
    );
    assert_eq!(
        hit.icon_url.as_deref(),
        Some("https://media.forgecdn.net/t.jpeg")
    );
    assert_eq!(hit.updated, "2026-10-12T14:05:52.453Z");
    assert!(hit.manual_download);

    let context = Context::default().answer(&path, 200, &body).english();
    let page = keyed().search(&context, &query(ProjectKind::Mod)).unwrap();
    assert_eq!(page.hits[0].categories[0], "Map and Information");
}

#[test]
fn a_project_has_its_html_body_gallery_and_links() {
    let context = Context::default()
        .answer(
            "/v1/mods/238222",
            200,
            &format!(r#"{{"data":{}}}"#, jei(true)),
        )
        .answer(
            "/v1/mods/238222/description",
            200,
            r#"{"data":"<p>Hello</p>"}"#,
        );
    let project = keyed().project(&context, "238222").unwrap();
    assert_eq!(project.body, "<p>Hello</p>");
    assert_eq!(project.author.as_deref(), Some("mezz"));
    assert_eq!(project.followers, None);
    assert_eq!(
        project.links.source.as_deref(),
        Some("https://github.com/mezz/JustEnoughItems")
    );
    assert_eq!(project.links.wiki, None, "an empty link is no link");
    assert_eq!(project.gallery[0].url, "https://media.forgecdn.net/s-t.png");
    assert_eq!(
        project.gallery[0].full_url,
        "https://media.forgecdn.net/s.png"
    );
    assert_eq!(project.game_versions, vec!["1.20.1", "1.21.1"]);
    assert!(!project.manual_download);

    let no_body = Context::default().answer(
        "/v1/mods/238222",
        200,
        &format!(r#"{{"data":{}}}"#, jei(true)),
    );
    assert_eq!(
        keyed().project(&no_body, "238222").unwrap().body,
        "",
        "the body is decoration"
    );
}

#[test]
fn versions_read_every_page_and_separate_versions_from_loaders() {
    let page = |files: &[String], total: u64| {
        format!(
            r#"{{"data":[{}],"pagination":{{"totalCount":{total}}}}}"#,
            files.join(",")
        )
    };
    let first: Vec<String> = (1..=1000).map(|id| file(id, 238222, "")).collect();
    let blocked = file(1001, 238222, "").replace(
        r#""downloadUrl":"https://edge.forgecdn.net/files/1001/f-1001.jar""#,
        r#""downloadUrl":null"#,
    );
    let context = Context::default()
        .answer(
            "/v1/mods/238222",
            200,
            &format!(r#"{{"data":{}}}"#, jei(false)),
        )
        .answer(
            "/v1/mods/238222/files?index=0&pageSize=1000",
            200,
            &page(&first, 1001),
        )
        .answer(
            "/v1/mods/238222/files?index=1000&pageSize=1000",
            200,
            &page(&[blocked], 1001),
        );
    let versions = keyed().versions(&context, "238222", true).unwrap();
    assert_eq!(versions.len(), 1001);
    let version = &versions[0];
    assert_eq!(
        version.game_versions,
        vec!["1.20.1"],
        "Client, Forge and Server are not versions"
    );
    assert_eq!(version.loaders, vec!["forge"]);
    assert_eq!(
        version.files[0].sha1.as_deref(),
        Some("e514cd6bc50b816487d9c25cd457de5e0bd5b07f")
    );
    assert_eq!(version.changelog, "", "changelogs are read one at a time");
    assert_eq!(
        version
            .dependencies
            .iter()
            .map(|dependency| dependency.kind)
            .collect::<Vec<_>>(),
        vec![
            DependencyKind::Required,
            DependencyKind::Optional,
            DependencyKind::Incompatible,
            DependencyKind::Other
        ]
    );
    let last = versions.last().unwrap();
    assert_eq!(
        last.files[0].url, "",
        "the author allows no launcher downloads"
    );
    assert_eq!(
        last.files[0].download_page.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/jei/files/1001")
    );
}

#[test]
fn an_old_file_that_names_no_loader_is_a_forge_mod() {
    let old = file(5, 6, "")
        .replace(r#"["Client","1.20.1","Forge","Server"]"#, r#"["1.12.2"]"#)
        .replace(r#""sortableGameVersions":[{"gameVersionName":"Client","gameVersion":""},{"gameVersionName":"1.20.1","gameVersion":"1.20.1"},{"gameVersionName":"Forge","gameVersion":""}],"#, "");
    let raw: protocol::RawFile = serde_json::from_str(&old).unwrap();
    assert_eq!(raw.minecraft_versions(), vec!["1.12.2"]);
    assert_eq!(raw.loaders(), vec!["forge"]);
    let modern = old.replace("1.12.2", "1.20.1");
    let raw: protocol::RawFile = serde_json::from_str(&modern).unwrap();
    assert!(
        raw.loaders().is_empty(),
        "after 1.14 a missing loader is unknown"
    );
}

fn murmur(key: &str, value: u32) -> FileIdentity {
    FileIdentity {
        key: key.into(),
        fingerprints: vec![Fingerprint::Sha1("aa".into()), Fingerprint::Murmur2(value)],
    }
}

#[test]
fn files_are_identified_by_their_murmur2_fingerprint() {
    let context = Context::default().answer(
        "/v1/fingerprints/432",
        200,
        &format!(
            r#"{{"data":{{"exactMatches":[{{"id":238222,"file":{}}}]}}}}"#,
            file(200, 238222, "")
        ),
    );
    let found = keyed()
        .identify(&context, &[murmur("a", 4_164_405_417), murmur("b", 7)])
        .unwrap();
    assert_eq!(found.keys().collect::<Vec<_>>(), vec!["a"]);
    assert_eq!(found["a"].project_id, "238222");
    let request = &context.requests()[0];
    assert_eq!(
        String::from_utf8(request.body.clone().unwrap()).unwrap(),
        r#"{"fingerprints":[7,4164405417]}"#
    );
    let sha1_only = FileIdentity {
        key: "c".into(),
        fingerprints: vec![Fingerprint::Sha1("aa".into())],
    };
    assert!(matches!(
        keyed().identify(&Context::default(), &[sha1_only]),
        Err(PluginError::InvalidInput(_))
    ));
}

#[test]
fn updates_are_newer_compatible_and_no_less_stable() {
    let installed = file(200, 238222, "");
    let context = Context::default()
        .answer(
            "/v1/fingerprints/432",
            200,
            &format!(r#"{{"data":{{"exactMatches":[{{"id":238222,"file":{installed}}}]}}}}"#),
        )
        .answer("/v1/mods", 200, &format!(r#"{{"data":[{}]}}"#, jei(true)))
        .answer(
            "/v1/mods/files",
            200,
            &format!(r#"{{"data":[{}]}}"#, file(250, 238222, "")),
        );
    let fabric = Compatibility {
        game_version: "1.20.1".into(),
        loader: Some("fabric".into()),
    };
    let latest = keyed()
        .latest_for(&context, &[murmur("a", 4_164_405_417)], &fabric)
        .unwrap();
    assert_eq!(latest["a"].id, "250");
    assert_eq!(
        String::from_utf8(context.requests()[2].body.clone().unwrap()).unwrap(),
        r#"{"fileIds":[250]}"#,
        "the Fabric release; the newer Forge beta is another loader"
    );

    let project: protocol::RawMod = serde_json::from_str(&jei(true)).unwrap();
    let current: protocol::RawFile = serde_json::from_str(&installed).unwrap();
    let forge = Compatibility {
        game_version: "1.20.1".into(),
        loader: Some("forge".into()),
    };
    assert_eq!(
        query::newest_compatible(&project, &current, &forge),
        None,
        "a release is not offered a beta"
    );
    let beta: protocol::RawFile =
        serde_json::from_str(&installed.replace(r#""releaseType":1"#, r#""releaseType":2"#))
            .unwrap();
    assert_eq!(query::newest_compatible(&project, &beta, &forge), Some(300));
    let other_game = Compatibility {
        game_version: "1.21.1".into(),
        loader: Some("forge".into()),
    };
    assert_eq!(
        query::newest_compatible(&project, &current, &other_game),
        None
    );
}

#[test]
fn pack_files_resolve_with_their_folder_kind_and_download_page() {
    let blocked = file(11, 238222, "").replace(
        r#""downloadUrl":"https://edge.forgecdn.net/files/11/f-11.jar""#,
        r#""downloadUrl":null"#,
    );
    let pack = format!(
        r#"{{"data":[{blocked},{},{}]}}"#,
        file(12, 777, ""),
        file(13, 999, "")
    );
    let shader = r#"{"id":777,"name":"BSL","classId":6552,"links":{"websiteUrl":"https://www.curseforge.com/minecraft/shaders/bsl"}}"#;
    let context = Context::default()
        .answer("/v1/mods/files", 200, &pack)
        .answer(
            "/v1/mods",
            200,
            &format!(r#"{{"data":[{},{shader}]}}"#, jei(false)),
        );
    let references = [
        VersionRef {
            project_id: "238222".into(),
            version_id: "11".into(),
        },
        VersionRef {
            project_id: "777".into(),
            version_id: "12".into(),
        },
        VersionRef {
            project_id: "5".into(),
            version_id: "13".into(),
        },
        VersionRef {
            project_id: "x".into(),
            version_id: "y".into(),
        },
    ];
    let resolved = keyed().resolve_files(&context, &references).unwrap();
    assert_eq!(
        resolved.len(),
        2,
        "a file of another project is not the one asked for"
    );
    assert_eq!(resolved[0].kind, PackFileKind::Mod);
    assert_eq!(resolved[0].title, "Just Enough Items (JEI)");
    assert_eq!(resolved[0].file.url, "");
    assert_eq!(
        resolved[0].file.download_page.as_deref(),
        Some("https://www.curseforge.com/minecraft/mc-mods/jei/files/11")
    );
    assert_eq!(resolved[1].kind, PackFileKind::Shader);
    assert!(
        resolved[1]
            .file
            .url
            .starts_with("https://edge.forgecdn.net/")
    );
}

#[test]
fn filter_choices_group_resolutions_and_keep_curseforge_order() {
    let categories = r#"{"data":[
        {"id":6,"name":"Mods","classId":6,"isClass":true},
        {"id":423,"name":"Map and Information","classId":6},
        {"id":393,"name":"16x","classId":12},
        {"id":398,"name":"512x and Higher","classId":12},
        {"id":404,"name":"Animated","classId":12},
        {"id":134,"name":"Teleportation","classId":5}]}"#;
    let versions = r#"{"data":[{"versionString":"26.3"},{"versionString":"1.21.6-snapshot"},{"versionString":""}]}"#;
    let context = Context::default()
        .answer("/v1/categories?gameId=432", 200, categories)
        .answer("/v1/minecraft/version", 200, versions);
    let choices = keyed().filter_choices(&context).unwrap();
    let headers: Vec<(&str, &str, &str)> = choices
        .categories
        .iter()
        .map(|tag| (tag.id.as_str(), tag.name.as_str(), tag.header.as_str()))
        .collect();
    assert_eq!(
        headers,
        vec![
            ("423", "地图与信息", "categories"),
            ("393", "16x", "resolutions"),
            ("398", "512x 及更高", "resolutions"),
            ("404", "动态", "categories"),
        ]
    );
    let versions: Vec<(&str, bool)> = choices
        .game_versions
        .iter()
        .map(|tag| (tag.version.as_str(), tag.snapshot))
        .collect();
    assert_eq!(versions, vec![("26.3", false), ("1.21.6-snapshot", true)]);
    assert_eq!(
        choices
            .loaders
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>(),
        vec!["fabric", "forge", "neoforge", "quilt"]
    );
}

#[test]
fn changelogs_and_single_files_are_read_by_reference() {
    let reference = VersionRef {
        project_id: "238222".into(),
        version_id: "200".into(),
    };
    let context = Context::default()
        .answer(
            "/v1/mods/238222/files/200/changelog",
            200,
            r#"{"data":"<ul><li>Fix</li></ul>"}"#,
        )
        .answer(
            "/v1/mods/238222/files/200",
            200,
            &format!(r#"{{"data":{}}}"#, file(200, 238222, "")),
        );
    assert_eq!(
        keyed().changelog(&context, &reference).unwrap(),
        "<ul><li>Fix</li></ul>"
    );
    let files = keyed().version_files(&context, &reference).unwrap();
    assert_eq!(files[0].filename, "f-200.jar");
    let elsewhere = VersionRef {
        project_id: "1".into(),
        version_id: "200".into(),
    };
    let wrong = Context::default().answer(
        "/v1/mods/1/files/200",
        200,
        &format!(r#"{{"data":{}}}"#, file(200, 238222, "")),
    );
    assert!(keyed().version_files(&wrong, &elsewhere).is_err());
}
