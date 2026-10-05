use super::*;
use lumilio_plugin_api::{FetchMethod, FetchResponse, SettingValue};
use std::sync::Mutex;

#[derive(Default)]
struct Context {
    routes: BTreeMap<String, FetchResponse>,
    requests: Mutex<Vec<FetchRequest>>,
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

    fn requests(&self) -> Vec<FetchRequest> {
        self.requests.lock().unwrap().clone()
    }
}

impl HostContext for Context {
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
        let reply = self
            .routes
            .get(&request.url)
            .cloned()
            .unwrap_or(FetchResponse {
                status: 404,
                body: Vec::new(),
            });
        self.requests.lock().unwrap().push(request);
        Ok(reply)
    }
}

const VERSION: &str = r#"{"id":"v","project_id":"P","version_type":"beta","version_number":"2","changelog":"Changes",
    "game_versions":["1.21.1"],"loaders":["fabric"],"date_published":"2024-08-08T00:00:00Z",
    "files":[{"url":"https://cdn.modrinth.com/a.jar","filename":"a.jar","primary":true,"size":10,"hashes":{"sha1":"aa"}}],
    "dependencies":[{"project_id":"D","version_id":"d","dependency_type":"required"}]}"#;

fn file(key: &str, hash: &str) -> FileIdentity {
    FileIdentity {
        key: key.into(),
        fingerprints: vec![Fingerprint::Sha1(hash.into())],
    }
}

#[test]
fn manifest_exposes_only_the_api_network_grant_and_defaults_to_enabled() {
    let manifest = Modrinth.manifest();
    assert_eq!(manifest.id, ID);
    assert_eq!(manifest.api, API_VERSION);
    assert!(manifest.default_enabled);
    assert_eq!(
        manifest.permissions,
        vec![Permission::Network {
            hosts: vec!["api.modrinth.com".into()]
        }]
    );
    assert!(Modrinth.content_source().is_some());
}

#[test]
fn search_through_the_context_preserves_query_and_decodes_cards() {
    let mut search = query(ProjectKind::Mod);
    search.text = "sodium".into();
    search.categories = vec![include("optimization"), exclude("cursed")];
    let mut ctx = Context::default();
    ctx.routes.insert(search.url(), FetchResponse { status: 200, body: br#"{"total_hits":1,"hits":[{"project_id":"A","slug":"sodium","name":"Sodium","project_types":["mod"],"loaders":["fabric"]}]}"#.to_vec() });
    let page = Modrinth.search(&ctx, &search).unwrap();
    assert_eq!(page.hits[0].title, "Sodium");
    assert_eq!(page.hits[0].loaders, ["fabric"]);
    assert_eq!(page.hits[0].page_url, "https://modrinth.com/mod/sodium");
    let request = &ctx.requests()[0];
    assert_eq!(request.method, FetchMethod::Get);
    assert_eq!(request.url, search.url());
}

#[test]
fn unsupported_filters_and_sorts_are_refused_before_any_request() {
    let ctx = Context::default();
    let mut search = query(ProjectKind::ResourcePack);
    for invalid in 0..3 {
        search.sort = if invalid == 0 {
            Sort::Name
        } else {
            Sort::Relevance
        };
        search.client = invalid == 1;
        search.loaders = if invalid == 2 {
            vec![include("fabric")]
        } else {
            Vec::new()
        };
        assert!(matches!(
            Modrinth.search(&ctx, &search),
            Err(PluginError::InvalidInput(_))
        ));
    }
    assert!(ctx.requests().is_empty());
}

#[test]
fn loader_all_and_any_groups_have_distinct_semantics() {
    let mut search = query(ProjectKind::Mod);
    search.loaders = vec![include("fabric"), include("quilt")];
    assert_eq!(
        search.expression(),
        "categories = `fabric` AND categories = `quilt` AND project_types = `mod`"
    );
    search.loaders.iter_mut().for_each(|pick| pick.any = true);
    assert_eq!(
        search.expression(),
        "categories IN [`fabric`, `quilt`] AND project_types = `mod`"
    );
}

#[test]
fn project_reads_the_owner_and_missing_decoration_keeps_the_page() {
    let ctx = Context::default()
        .answer("/v2/project/P", 200, r##"{"id":"P","slug":"cool","project_type":"mod","title":"Cool","body":"# Details"}"##)
        .answer("/v2/project/P/members", 200, r#"[{"role":"Member","user":{"username":"helper"}},{"role":"Owner","user":{"username":"owner"}}]"#);
    let project = Modrinth.project(&ctx, "P").unwrap();
    assert_eq!(project.author.as_deref(), Some("owner"));
    assert_eq!(project.body, "# Details");
    assert_eq!(project.page_url, "https://modrinth.com/mod/cool");
    let ctx = ctx.answer("/v2/project/P/members", 200, "broken json");
    assert_eq!(Modrinth.project(&ctx, "P").unwrap().author, None);
}

#[test]
fn opaque_ids_are_a_single_segment_and_special_segments_are_refused() {
    let ctx = Context::default().answer(
        "/v2/project/P%2FQ%3Fx%23y",
        200,
        r#"{"id":"P","project_type":"mod"}"#,
    );
    assert!(Modrinth.project(&ctx, "P/Q?x#y").is_ok());
    for id in ["", ".", ".."] {
        let before = ctx.requests().len();
        assert!(matches!(
            Modrinth.project(&ctx, id),
            Err(PluginError::InvalidInput(_))
        ));
        assert_eq!(ctx.requests().len(), before);
    }
}

#[test]
fn versions_files_and_dependencies_follow_the_documented_endpoints() {
    let ctx = Context::default()
        .answer(
            "/v2/project/P/version?include_changelog=false",
            200,
            &format!("[{VERSION}]"),
        )
        .answer(
            "/v2/project/P/version?include_changelog=true",
            200,
            &format!("[{VERSION}]"),
        )
        .answer("/v2/version/v", 200, VERSION);
    let versions = Modrinth.versions(&ctx, "P", false).unwrap();
    assert_eq!(versions[0].channel, ReleaseChannel::Beta);
    assert_eq!(
        Modrinth.versions(&ctx, "P", true).unwrap()[0].changelog,
        "Changes"
    );
    let reference = VersionRef {
        project_id: "P".into(),
        version_id: "v".into(),
    };
    assert_eq!(
        Modrinth.version_files(&ctx, &reference).unwrap()[0].filename,
        "a.jar"
    );
    assert_eq!(
        Modrinth.dependencies(&ctx, &reference).unwrap()[0].kind,
        DependencyKind::Required
    );
    let wrong = VersionRef {
        project_id: "other".into(),
        ..reference
    };
    assert!(Modrinth.version_files(&ctx, &wrong).is_err());
    let reference = VersionRef {
        project_id: "P".into(),
        version_id: "other".into(),
    };
    let ctx = ctx.answer("/v2/version/other", 200, VERSION);
    assert!(Modrinth.dependencies(&ctx, &reference).is_err());
}

#[test]
fn file_recognition_maps_deduplicated_hashes_back_to_all_host_keys() {
    let ctx = Context::default().answer(
        "/v2/version_files",
        200,
        &format!(r#"{{"aa":{VERSION},"unrequested":{VERSION}}}"#),
    );
    let files = [file("one", "AA"), file("two", "aa"), file("unknown", "bb")];
    let result = Modrinth.identify(&ctx, &files).unwrap();
    assert_eq!(
        result.keys().collect::<Vec<_>>(),
        [&"one".to_owned(), &"two".to_owned()]
    );
    assert_eq!(result["two"].id, "v");
    let request = &ctx.requests()[0];
    assert_eq!(request.method, FetchMethod::Post);
    assert_eq!(
        request.headers,
        [("content-type".into(), "application/json".into())]
    );
    let body: serde_json::Value = serde_json::from_slice(request.body.as_ref().unwrap()).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"hashes":["aa","bb"],"algorithm":"sha1"})
    );
}

#[test]
fn updates_carry_compatibility_and_vanilla_has_no_loader_constraint() {
    let ctx = Context::default().answer(
        "/v2/version_files/update",
        200,
        &format!(r#"{{"aa":{VERSION}}}"#),
    );
    let mut compatibility = Compatibility {
        game_version: "1.21.1".into(),
        loader: Some("fabric".into()),
    };
    assert_eq!(
        Modrinth
            .latest_for(&ctx, &[file("file", "aa")], &compatibility)
            .unwrap()["file"]
            .number,
        "2"
    );
    compatibility.loader = None;
    Modrinth
        .latest_for(&ctx, &[file("file", "aa")], &compatibility)
        .unwrap();
    let requests = ctx.requests();
    for (index, loaders) in [serde_json::json!(["fabric"]), serde_json::json!([])]
        .into_iter()
        .enumerate()
    {
        let body: serde_json::Value =
            serde_json::from_slice(requests[index].body.as_ref().unwrap()).unwrap();
        assert_eq!(body["loaders"], loaders);
        assert_eq!(body["game_versions"], serde_json::json!(["1.21.1"]));
    }
}

#[test]
fn empty_batches_do_not_touch_the_context_and_unsupported_fingerprints_are_errors() {
    let ctx = Context::default();
    assert!(Modrinth.identify(&ctx, &[]).unwrap().is_empty());
    assert!(
        Modrinth
            .latest_for(
                &ctx,
                &[],
                &Compatibility {
                    game_version: "1".into(),
                    loader: None
                }
            )
            .unwrap()
            .is_empty()
    );
    assert!(Modrinth.project_summaries(&ctx, &[]).unwrap().is_empty());
    assert!(matches!(
        Modrinth.identify(
            &ctx,
            &[FileIdentity {
                key: "cf-file".into(),
                fingerprints: vec![Fingerprint::Murmur2(42)]
            }]
        ),
        Err(PluginError::InvalidInput(_))
    ));
    assert!(ctx.requests().is_empty());
}

#[test]
fn summaries_keep_labels_when_team_lookup_is_unavailable() {
    let ids = vec!["P".to_owned()];
    let ctx = Context::default()
        .answer(
            &format!("/v2/projects?ids={}", encoded_list(&ids)),
            200,
            r#"[{"id":"P","slug":"cool","title":"Cool","project_type":"mod","team":"t"}]"#,
        )
        .answer(
            &format!("/v2/teams?ids={}", encoded_list(&["t".to_owned()])),
            200,
            r#"[[{"role":"Owner","team_id":"t","user":{"username":"owner"}}]]"#,
        );
    assert_eq!(
        Modrinth.project_summaries(&ctx, &ids).unwrap()[0]
            .author
            .as_deref(),
        Some("owner")
    );
    let ctx = ctx.answer(
        &format!("/v2/teams?ids={}", encoded_list(&["t".to_owned()])),
        503,
        "offline",
    );
    let summaries = Modrinth.project_summaries(&ctx, &ids).unwrap();
    assert_eq!(summaries[0].title, "Cool");
    assert_eq!(summaries[0].author, None);
}

#[test]
fn filter_choices_decode_kinds_and_survive_a_missing_loader_list() {
    let ctx = Context::default()
        .answer(
            "/v2/tag/category",
            200,
            r#"[{"name":"optimization","project_type":"mod","header":"categories"}]"#,
        )
        .answer(
            "/v2/tag/game_version",
            200,
            r#"[{"version":"1.21.1","version_type":"release"}]"#,
        )
        .answer(
            "/v2/tag/loader",
            200,
            r#"[{"name":"fabric","supported_project_types":["mod","modpack","future"]}]"#,
        );
    let choices = Modrinth.filter_choices(&ctx).unwrap();
    assert_eq!(choices.categories[0].id, "optimization");
    assert_eq!(choices.game_versions[0].version, "1.21.1");
    assert_eq!(
        choices.loaders[0].kinds,
        [ProjectKind::Mod, ProjectKind::Modpack]
    );
    let ctx = ctx.answer("/v2/tag/loader", 500, "offline");
    assert!(Modrinth.filter_choices(&ctx).unwrap().loaders.is_empty());
}

#[test]
fn http_and_decode_failures_are_errors_instead_of_empty_success() {
    for (status, body) in [
        (404, "missing"),
        (429, "busy"),
        (500, "offline"),
        (200, "broken json"),
    ] {
        let ctx = Context::default().answer("/v2/version_files", status, body);
        // An HTTP error status can pass (the host retries it later); an answer
        // that cannot be read is the plugin's problem.
        let result = Modrinth.identify(&ctx, &[file("f", "aa")]);
        if status == 200 {
            assert!(matches!(result, Err(PluginError::Unavailable(_))));
        } else {
            assert!(matches!(result, Err(PluginError::Transient(_))));
        }
    }
}
