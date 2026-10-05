use super::*;
use lumilio_plugin_api::ContentSource;
use lumilio_plugin_api::content::*;

struct Source {
    id: &'static str,
    panic: bool,
    caller: std::thread::ThreadId,
}

impl Plugin for Source {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: self.id.into(),
            name: self.id.into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: Vec::new(),
            settings: Vec::new(),
        }
    }
    fn content_source(&self) -> Option<&dyn ContentSource> {
        assert_ne!(self.caller, std::thread::current().id());
        Some(self)
    }
}

fn single_filter() -> PickSupport {
    PickSupport {
        max_included: 1,
        exclude: false,
        any: false,
        all: false,
    }
}

fn capabilities() -> Capabilities {
    Capabilities {
        kinds: vec![KindSupport {
            kind: ProjectKind::Mod,
            filters: FilterSupport {
                max_game_versions: 1,
                categories: Some(single_filter()),
                loaders: Some(single_filter()),
                ..Default::default()
            },
            sorts: vec![Sort::Downloads, Sort::Newest, Sort::Updated],
        }],
        fingerprints: vec![FingerprintKind::Murmur2],
    }
}

fn query() -> SearchQuery {
    SearchQuery {
        text: "sodium".into(),
        kind: ProjectKind::Mod,
        sort: Sort::Downloads,
        page: 2,
        page_size: 20,
        game_versions: vec!["1.21.1".into()],
        loaders: vec![Pick {
            name: "fabric".into(),
            stance: Stance::Include,
            any: false,
        }],
        categories: vec![Pick {
            name: "123".into(),
            stance: Stance::Include,
            any: false,
        }],
        client: false,
        server: false,
        open_source: None,
        hidden_projects: Vec::new(),
        excluded_disclosures: Vec::new(),
        excluded_types: Vec::new(),
    }
}

fn version(reference: &VersionRef) -> Version {
    Version {
        id: reference.version_id.clone(),
        project_id: reference.project_id.clone(),
        name: "1.0".into(),
        number: "1.0".into(),
        channel: ReleaseChannel::Release,
        game_versions: vec!["1.21.1".into()],
        loaders: vec!["fabric".into()],
        published: String::new(),
        files: vec![VersionFile {
            url: "https://cdn.test/a.jar".into(),
            filename: "a.jar".into(),
            primary: true,
            size: 4,
            sha1: Some("abc".into()),
        }],
        dependencies: vec![Dependency {
            project_id: Some("42".into()),
            version_id: None,
            kind: DependencyKind::Required,
        }],
        downloads: 0,
        changelog: String::new(),
    }
}

impl ContentSource for Source {
    fn capabilities(&self) -> Capabilities {
        assert_ne!(self.caller, std::thread::current().id());
        capabilities()
    }
    fn search(&self, _: &dyn HostContext, query: &SearchQuery) -> Result<SearchPage, PluginError> {
        assert_ne!(self.caller, std::thread::current().id());
        if self.panic {
            panic!("source crashed");
        }
        if query.text == "slow" {
            std::thread::sleep(Duration::from_millis(50));
        }
        query.validate(&capabilities())?;
        Ok(SearchPage {
            hits: Vec::new(),
            total_hits: 42,
        })
    }
    fn project(&self, _: &dyn HostContext, _: &str) -> Result<Project, PluginError> {
        Err(PluginError::Unavailable("fixture has no project".into()))
    }
    fn versions(
        &self,
        _: &dyn HostContext,
        project: &str,
        _: bool,
    ) -> Result<Vec<Version>, PluginError> {
        Ok(vec![version(&VersionRef {
            project_id: project.into(),
            version_id: "5678".into(),
        })])
    }
    fn version_files(
        &self,
        _: &dyn HostContext,
        reference: &VersionRef,
    ) -> Result<Vec<VersionFile>, PluginError> {
        Ok(version(reference).files)
    }
    fn dependencies(
        &self,
        _: &dyn HostContext,
        reference: &VersionRef,
    ) -> Result<Vec<Dependency>, PluginError> {
        Ok(version(reference).dependencies)
    }
    fn filter_choices(&self, _: &dyn HostContext) -> Result<FilterChoices, PluginError> {
        Ok(FilterChoices::default())
    }
    fn identify(
        &self,
        _: &dyn HostContext,
        files: &[FileIdentity],
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        Ok(files
            .iter()
            .filter(|file| file.fingerprints.contains(&Fingerprint::Murmur2(123)))
            .map(|file| {
                (
                    file.key.clone(),
                    version(&VersionRef {
                        project_id: "1234".into(),
                        version_id: "5678".into(),
                    }),
                )
            })
            .collect())
    }
    fn latest_for(
        &self,
        ctx: &dyn HostContext,
        files: &[FileIdentity],
        _: &Compatibility,
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        self.identify(ctx, files)
    }
    fn project_summaries(
        &self,
        _: &dyn HostContext,
        _: &[String],
    ) -> Result<Vec<ProjectSummary>, PluginError> {
        Ok(Vec::new())
    }
}

fn source(id: &'static str, panic: bool) -> Arc<dyn Plugin> {
    Arc::new(Source {
        id,
        panic,
        caller: std::thread::current().id(),
    })
}

#[test]
fn a_single_filter_source_can_express_its_limits_without_modrinth_syntax() {
    let base = query();
    assert!(base.validate(&capabilities()).is_ok());
    for bad in [
        SearchQuery {
            kind: ProjectKind::Shader,
            ..base.clone()
        },
        SearchQuery {
            sort: Sort::Follows,
            ..base.clone()
        },
        SearchQuery {
            game_versions: vec!["1.21.1".into(), "1.20.1".into()],
            ..base.clone()
        },
        SearchQuery {
            client: true,
            ..base.clone()
        },
        SearchQuery {
            open_source: Some(Stance::Exclude),
            ..base.clone()
        },
        SearchQuery {
            hidden_projects: vec!["1234".into()],
            ..base.clone()
        },
        SearchQuery {
            excluded_disclosures: vec!["epilepsy_triggers".into()],
            ..base.clone()
        },
        SearchQuery {
            excluded_types: vec!["datapack".into()],
            ..base.clone()
        },
        SearchQuery {
            categories: vec![Pick {
                name: "123".into(),
                stance: Stance::Exclude,
                any: false,
            }],
            ..base.clone()
        },
        SearchQuery {
            categories: vec![base.categories[0].clone(), base.categories[0].clone()],
            ..base.clone()
        },
    ] {
        assert!(bad.validate(&capabilities()).is_err(), "{bad:?}");
    }
    let bytes = serde_json::to_vec(&base).unwrap();
    assert_eq!(serde_json::from_slice::<SearchQuery>(&bytes).unwrap(), base);
}

#[test]
fn combined_filters_keep_any_all_and_exclude_distinct() {
    let mut caps = capabilities();
    caps.kinds[0].filters.categories = Some(PickSupport {
        max_included: 4,
        exclude: true,
        any: true,
        all: true,
    });
    let mut query = query();
    query.categories.push(Pick {
        name: "456".into(),
        stance: Stance::Include,
        any: true,
    });
    query.categories.push(Pick {
        name: "789".into(),
        stance: Stance::Exclude,
        any: false,
    });
    assert!(query.validate(&caps).is_ok());
    caps.kinds[0].filters.categories.as_mut().unwrap().all = false;
    assert!(query.validate(&caps).is_err());
    query.categories[0].any = true;
    assert!(query.validate(&caps).is_ok());
    caps.kinds[0].filters.categories.as_mut().unwrap().any = false;
    assert!(query.validate(&caps).is_err());
}

#[tokio::test]
async fn content_sources_and_search_disappear_when_disabled_and_return_when_enabled() {
    let host = PluginHost::new(
        vec![
            source("test.source", false),
            fake("test.other", API_VERSION, true),
        ],
        BTreeMap::new(),
    );
    let sources = host.content_sources().await;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].capabilities, capabilities());
    assert_eq!(
        host.search_content("test.source", query())
            .await
            .unwrap()
            .total_hits,
        42
    );
    host.set_state(
        "test.source".into(),
        PluginState {
            enabled: Some(false),
            ..Default::default()
        },
    );
    assert!(host.content_sources().await.is_empty());
    assert!(host.search_content("test.source", query()).await.is_none());
    host.set_state("test.source".into(), PluginState::default());
    assert_eq!(host.content_sources().await.len(), 1);
}

#[tokio::test]
async fn a_panicking_content_source_does_not_disable_other_sources() {
    let host = PluginHost::new(
        vec![source("test.bad", true), source("test.good", false)],
        BTreeMap::new(),
    );
    assert!(host.search_content("test.bad", query()).await.is_none());
    assert_eq!(
        host.content_sources()
            .await
            .iter()
            .map(|source| source.plugin.as_str())
            .collect::<Vec<_>>(),
        ["test.good"]
    );
    assert_eq!(
        host.search_content("test.good", query())
            .await
            .unwrap()
            .total_hits,
        42
    );
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn search_uses_its_own_budget_instead_of_the_normal_call_timeout() {
    let mut host = PluginHost::new(vec![source("test.source", false)], BTreeMap::new());
    host.list().await;
    host.timeout = Duration::from_millis(5);
    let mut query = query();
    query.text = "slow".into();
    assert_eq!(
        host.search_content("test.source", query)
            .await
            .unwrap()
            .total_hits,
        42
    );
    assert_eq!(host.list().await[0].status, PluginStatus::Enabled);
}

#[tokio::test]
async fn timed_out_content_calls_discard_late_results_and_preserve_other_sources() {
    let mut host = PluginHost::new(
        vec![source("test.bad", false), source("test.good", false)],
        BTreeMap::new(),
    );
    host.list().await;
    host.timeout = Duration::from_millis(20);
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let call = host.call_content("test.bad", move |_, _| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(42)
    });
    let (result, ()) = tokio::join!(call, async {
        started_rx.await.unwrap();
    });
    assert_eq!(result, None);
    release_tx.send(()).unwrap();
    assert_eq!(host.call_content("test.good", |_, _| Ok(7)).await, Some(7));
    assert!(matches!(
        host.list().await[0].status,
        PluginStatus::Failed { .. }
    ));
}

#[tokio::test]
async fn version_refs_and_fingerprints_cover_a_project_scoped_file_api() {
    let host = PluginHost::new(vec![source("test.source", false)], BTreeMap::new());
    let versions = host
        .call_content("test.source", |source, ctx| {
            source.versions(ctx, "1234", false)
        })
        .await
        .unwrap();
    let reference = VersionRef {
        project_id: versions[0].project_id.clone(),
        version_id: versions[0].id.clone(),
    };
    let files = host
        .call_content("test.source", move |source, ctx| {
            source.version_files(ctx, &reference)
        })
        .await
        .unwrap();
    assert_eq!(files[0].filename, "a.jar");
    let found = host
        .call_content("test.source", |source, ctx| {
            source.identify(
                ctx,
                &[
                    FileIdentity {
                        key: "local-file".into(),
                        fingerprints: vec![Fingerprint::Murmur2(123)],
                    },
                    FileIdentity {
                        key: "unknown".into(),
                        fingerprints: vec![Fingerprint::Sha1("other".into())],
                    },
                ],
            )
        })
        .await
        .unwrap();
    assert_eq!(found["local-file"].project_id, "1234");
    assert!(!found.contains_key("unknown"));
}
