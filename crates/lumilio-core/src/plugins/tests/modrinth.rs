use crate::plugins::{PluginHost, PluginStatus};
use crate::transfer::{
    HttpRequest, OfficialSource, SourceChain, SourceProvider, Transport, TransportFuture,
    TransportResponse,
};
use lumilio_plugin_api::PluginState;
use lumilio_plugin_api::content::{ProjectKind, SearchQuery, Sort};
use lumilio_plugin_modrinth::{ID, Modrinth};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Api {
    status: u16,
    requests: Arc<Mutex<Vec<HttpRequest>>>,
}

impl Transport for Api {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        panic!("plugin requests must use the single-hop transport")
    }

    fn send_no_redirect<'a>(&'a self, request: HttpRequest) -> TransportFuture<'a> {
        self.requests.lock().unwrap().push(request);
        Box::pin(async move {
            Ok(TransportResponse::from_bytes(self.status, br#"{"total_hits":1,"hits":[{"project_id":"A","slug":"sodium","name":"Sodium","project_types":["mod"]}]}"#.to_vec()))
        })
    }
}

fn setup(status: u16) -> (PluginHost, Api) {
    let api = Api {
        status,
        requests: Arc::default(),
    };
    let host = PluginHost::new(vec![Arc::new(Modrinth)], Default::default()).with_network(
        api.clone(),
        SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap(),
    );
    (host, api)
}

fn query() -> SearchQuery {
    SearchQuery {
        text: "sodium".into(),
        kind: ProjectKind::Mod,
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

#[tokio::test]
async fn modrinth_contributes_via_the_host_and_stops_requests_when_disabled() {
    let (host, api) = setup(200);
    let sources = host.content_sources().await;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].plugin, ID);
    assert_eq!(sources[0].capabilities.kinds.len(), 4);
    assert_eq!(
        host.search_content(ID, query()).await.unwrap().hits[0].title,
        "Sodium"
    );
    let requests = api.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0]
            .url
            .starts_with("https://api.modrinth.com/v3/search?")
    );
    host.set_state(
        ID.into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    assert!(host.content_sources().await.is_empty());
    assert!(host.search_content(ID, query()).await.is_none());
    assert!(
        host.call_content(ID, |source, ctx| source.project(ctx, "P"))
            .await
            .is_none()
    );
    assert_eq!(api.requests.lock().unwrap().len(), requests.len());
    host.set_state(
        ID.into(),
        PluginState {
            enabled: Some(true),
            ..Default::default()
        },
    );
    assert_eq!(
        host.search_content(ID, query()).await.unwrap().hits[0].slug,
        "sodium"
    );
}

#[tokio::test]
async fn an_api_error_fails_that_call_only_and_modrinth_keeps_running() {
    let (host, api) = setup(429);
    assert!(host.search_content(ID, query()).await.is_none());
    let info = host.list().await;
    assert_eq!(info[0].status, PluginStatus::Enabled);
    assert!(
        host.last_transient_error(ID)
            .await
            .is_some_and(|why| why.contains("HTTP 429"))
    );
    // Asking again reaches the API again: a retry can succeed.
    assert_eq!(host.content_sources().await.len(), 1);
    assert!(host.search_content(ID, query()).await.is_none());
    assert_eq!(api.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn missing_network_permission_leaves_no_search_result_and_no_io() {
    struct Ungranted;
    impl lumilio_plugin_api::Plugin for Ungranted {
        fn manifest(&self) -> lumilio_plugin_api::Manifest {
            let mut manifest = lumilio_plugin_api::Plugin::manifest(&Modrinth);
            manifest.permissions.clear();
            manifest
        }
        fn content_source(&self) -> Option<&dyn lumilio_plugin_api::ContentSource> {
            Some(&Modrinth)
        }
    }
    let (_, api) = setup(200);
    let host = PluginHost::new(vec![Arc::new(Ungranted)], Default::default()).with_network(
        api.clone(),
        SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap(),
    );
    assert!(host.search_content(ID, query()).await.is_none());
    assert!(api.requests.lock().unwrap().is_empty());
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
}

#[tokio::test]
#[ignore = "reads the real Modrinth API; no downloads or launcher data"]
async fn modrinth_content_contract_against_the_real_api() {
    use lumilio_plugin_api::content::{Compatibility, FileIdentity, Fingerprint, VersionRef};
    let host = PluginHost::new(vec![Arc::new(Modrinth)], Default::default()).with_network(
        crate::transfer::DefaultTransport::new().unwrap(),
        SourceChain::new([Arc::new(OfficialSource) as Arc<dyn SourceProvider>]).unwrap(),
    );
    let page = host
        .search_content(ID, query())
        .await
        .expect("search contribution");
    assert!(page.hits.iter().any(|hit| hit.slug == "sodium"));
    let project = host
        .call_content(ID, |source, ctx| source.project(ctx, "sodium"))
        .await
        .expect("project contribution");
    assert_eq!(project.slug, "sodium");
    assert!(!project.body.is_empty());
    let versions = host
        .call_content(ID, |source, ctx| source.versions(ctx, "sodium", true))
        .await
        .expect("versions contribution");
    let version = versions
        .iter()
        .find(|version| {
            version.files.iter().any(|file| file.sha1.is_some())
                && !version.game_versions.is_empty()
        })
        .expect("a hashed installable version")
        .clone();
    let reference = VersionRef {
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
    };
    let files_reference = reference.clone();
    let files = host
        .call_content(ID, move |source, ctx| {
            source.version_files(ctx, &files_reference)
        })
        .await
        .expect("file contribution");
    assert!(files.iter().any(|file| file.filename.ends_with(".jar")));
    assert!(
        host.call_content(ID, move |source, ctx| source.dependencies(ctx, &reference))
            .await
            .is_some()
    );
    let identities = vec![FileIdentity {
        key: "test-file".into(),
        fingerprints: vec![Fingerprint::Sha1(
            files.iter().find_map(|file| file.sha1.clone()).unwrap(),
        )],
    }];
    let recognition = identities.clone();
    let known = host
        .call_content(ID, move |source, ctx| source.identify(ctx, &recognition))
        .await
        .expect("recognition contribution");
    assert_eq!(known["test-file"].id, version.id);
    let compatibility = Compatibility {
        game_version: version.game_versions[0].clone(),
        loader: version.loaders.first().cloned(),
    };
    let latest = host
        .call_content(ID, move |source, ctx| {
            source.latest_for(ctx, &identities, &compatibility)
        })
        .await
        .expect("update contribution");
    assert!(latest.contains_key("test-file"));
    let projects = host
        .call_content(ID, move |source, ctx| {
            source.project_summaries(ctx, &[project.id])
        })
        .await
        .expect("summary contribution");
    assert_eq!(projects[0].slug, "sodium");
    let choices = host
        .call_content(ID, |source, ctx| source.filter_choices(ctx))
        .await
        .expect("filter choices contribution");
    assert!(
        choices
            .categories
            .iter()
            .any(|category| category.name == "optimization")
    );
    assert!(!choices.game_versions.is_empty());
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
}
