//! Discover with a real launcher behind it, on its own runtime, over a
//! content source that answers from memory: answers that arrive late belong
//! to a search or a source the page has left, and must not be shown.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use lumilio_core::{Loader, ProjectKind};
use lumilio_plugin_api::content::{
    Capabilities, Compatibility, Dependency, FileIdentity, FilterChoices, FilterSupport,
    KindSupport, Project, ProjectKind as SourceKind, ProjectSummary, ReleaseChannel, SearchHit,
    SearchPage, SearchQuery, Sort, Version, VersionFile, VersionRef,
};
use lumilio_plugin_api::{
    API_VERSION, ContentSource, HostContext, Manifest, Plugin, PluginError, Words,
};

use super::super::model::{DiscoverQuery, SearchStatus};
use super::super::{Discover, DiscoverEvent};
use crate::live::library_card;
use crate::testing::{self, Launcher, until};

/// A content source that answers from memory. A search for "slow" answers
/// late; one for "broken" fails.
struct Memory {
    id: &'static str,
}

impl Plugin for Memory {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: self.id.into(),
            name: Words::new(self.id, self.id),
            description: Words::default(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: Vec::new(),
            settings: Vec::new(),
        }
    }

    fn content_source(&self) -> Option<&dyn ContentSource> {
        Some(self)
    }
}

fn kind(kind: SourceKind) -> KindSupport {
    KindSupport {
        kind,
        filters: FilterSupport::default(),
        sorts: vec![Sort::Relevance, Sort::Downloads],
        browse_url: None,
    }
}

fn hit(title: &str) -> SearchHit {
    SearchHit {
        project_id: title.to_owned(),
        slug: title.to_owned(),
        title: title.to_owned(),
        description: String::new(),
        author: "a".into(),
        kind: SourceKind::Mod,
        categories: Vec::new(),
        all_categories: Vec::new(),
        loaders: Vec::new(),
        downloads: 1,
        follows: None,
        published: String::new(),
        updated: String::new(),
        environment: None,
        icon_url: None,
        page_url: String::new(),
        manual_download: false,
    }
}

impl ContentSource for Memory {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            kinds: vec![kind(SourceKind::Mod), kind(SourceKind::ResourcePack)],
            ..Default::default()
        }
    }

    fn search(&self, _: &dyn HostContext, query: &SearchQuery) -> Result<SearchPage, PluginError> {
        match query.text.as_str() {
            "slow" => std::thread::sleep(Duration::from_millis(400)),
            "broken" => return Err(PluginError::Unavailable("the source is down".into())),
            _ => {}
        }
        Ok(SearchPage {
            hits: vec![hit(&format!("{} from {}", query.text, self.id))],
            total_hits: 1,
        })
    }

    fn project(&self, _: &dyn HostContext, id: &str) -> Result<Project, PluginError> {
        Err(PluginError::Unavailable(format!("no project {id}")))
    }

    fn versions(
        &self,
        _: &dyn HostContext,
        project: &str,
        _: bool,
    ) -> Result<Vec<Version>, PluginError> {
        Ok(vec![Version {
            id: "v1".into(),
            project_id: project.into(),
            name: "1".into(),
            number: "1".into(),
            channel: ReleaseChannel::Release,
            game_versions: vec!["1.21.1".into()],
            loaders: Vec::new(),
            published: String::new(),
            files: vec![VersionFile {
                // Only on the source's site: the launcher may not fetch it.
                url: String::new(),
                filename: "pack.zip".into(),
                primary: true,
                size: 4,
                sha1: None,
                download_page: Some("https://source.test/pack".into()),
            }],
            dependencies: Vec::new(),
            downloads: 0,
            changelog: String::new(),
        }])
    }

    fn version_files(
        &self,
        _: &dyn HostContext,
        _: &VersionRef,
    ) -> Result<Vec<VersionFile>, PluginError> {
        Ok(Vec::new())
    }

    fn dependencies(
        &self,
        _: &dyn HostContext,
        _: &VersionRef,
    ) -> Result<Vec<Dependency>, PluginError> {
        Ok(Vec::new())
    }

    fn filter_choices(&self, _: &dyn HostContext) -> Result<FilterChoices, PluginError> {
        Ok(FilterChoices::default())
    }

    fn identify(
        &self,
        _: &dyn HostContext,
        _: &[FileIdentity],
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        Ok(BTreeMap::new())
    }

    fn latest_for(
        &self,
        _: &dyn HostContext,
        _: &[FileIdentity],
        _: &Compatibility,
    ) -> Result<BTreeMap<String, Version>, PluginError> {
        Ok(BTreeMap::new())
    }

    fn project_summaries(
        &self,
        _: &dyn HostContext,
        _: &[String],
    ) -> Result<Vec<ProjectSummary>, PluginError> {
        Ok(Vec::new())
    }
}

/// A launcher with two in-memory sources.
fn launcher() -> Launcher {
    testing::launcher(vec![
        Arc::new(Memory { id: "test.one" }) as Arc<dyn Plugin>,
        Arc::new(Memory { id: "test.two" }),
    ])
}

fn page<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (Entity<Discover>, &'a mut VisualTestContext) {
    // The runtime's threads wake GPUI.
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let worker = launcher.worker.clone();
    cx.add_window_view(move |_, _| Discover::new(Some(worker)))
}

fn query(source: &str, text: &str) -> DiscoverQuery {
    let mut query = DiscoverQuery::new(ProjectKind::Mod);
    query.source = source.to_owned();
    query.text = text.to_owned();
    query
}

#[gpui::test]
fn a_slow_answer_does_not_replace_a_newer_search(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (page, cx) = page(&launcher, cx);
    page.update(cx, |page, cx| {
        let slow = query("test.one", "slow");
        page.edit_state(|state| state.query = slow.clone(), cx);
        page.search(&slow, cx);
        let fast = query("test.one", "fast");
        page.edit_state(|state| state.query = fast.clone(), cx);
        page.search(&fast, cx);
    });
    until(cx, "the newer answer", |cx| {
        page.read_with(cx, |page, _| {
            matches!(page.state().search, SearchStatus::Done { .. })
        })
    });
    // Long enough for the slow answer to arrive too.
    std::thread::sleep(Duration::from_millis(600));
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        let titles: Vec<&str> = page
            .state()
            .results
            .iter()
            .map(|row| row.title.as_str())
            .collect();
        assert_eq!(titles, ["fast from test.one"]);
    });
    drop(launcher.runtime);
}

#[gpui::test]
fn a_source_that_fails_says_so_and_shows_nothing(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (page, cx) = page(&launcher, cx);
    page.update(cx, |page, cx| {
        let broken = query("test.one", "broken");
        page.edit_state(|state| state.query = broken.clone(), cx);
        page.search(&broken, cx);
    });
    until(cx, "the failure", |cx| {
        page.read_with(cx, |page, _| {
            matches!(page.state().search, SearchStatus::Failed(_))
        })
    });
    page.read_with(cx, |page, _| assert!(page.state().results.is_empty()));
}

/// Filters that arrive after another source was chosen belong to the old one:
/// they must not take the page back there.
#[gpui::test]
fn filters_for_a_source_left_behind_do_not_take_the_page_back(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (page, cx) = page(&launcher, cx);
    // A source that is not on: once loaded, the page would move to the first
    // one that is. The person chooses another before that arrives.
    page.update(cx, |page, cx| {
        page.edit_state(|state| state.query = query("test.gone", ""), cx);
        page.load_filters(cx);
        page.edit_state(|state| state.query = query("test.two", ""), cx);
    });
    until(cx, "the sources", |cx| {
        page.read_with(cx, |page, _| !page.state().sources.is_empty())
    });
    page.read_with(cx, |page, _| {
        let state = page.state();
        assert_eq!(state.query.source, "test.two");
        assert!(
            !state.filters.loaded,
            "the old source's filters were applied"
        );
    });
}

/// Choosing a source from the page searches there; the answer that comes
/// back is that source's.
#[gpui::test]
fn the_first_visit_lists_the_sources_and_searches_the_remembered_one(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (page, cx) = page(&launcher, cx);
    page.update(cx, |page, cx| {
        page.apply_preferences(
            lumilio_core::DiscoverPreferences {
                source: Some("test.two".into()),
                ..Default::default()
            },
            cx,
        );
        page.edit_state(|state| state.query.kind = ProjectKind::Mod, cx);
        page.shown(cx);
    });
    until(cx, "the first page", |cx| {
        page.read_with(cx, |page, _| {
            page.state().sources.len() == 2
                && matches!(page.state().search, SearchStatus::Done { .. })
        })
    });
    page.read_with(cx, |page, _| {
        let state = page.state();
        assert_eq!(state.query.source, "test.two");
        assert!(state.filters.loaded);
        assert_eq!(state.results[0].title, " from test.two");
    });
}

/// An install runs as a task of its own: the row says so while it runs, the
/// result is told when it ends, and Activity is told to look again.
#[gpui::test]
fn an_install_says_how_it_ended_and_lets_its_row_go(cx: &mut TestAppContext) {
    let launcher = launcher();
    let service = launcher.worker.service().clone();
    let game = launcher
        .runtime
        .block_on(service.create_instance("Game", Some("1.21.1"), Loader::Vanilla, None))
        .unwrap();
    let (page, cx) = page(&launcher, cx);
    let tasks = Rc::new(RefCell::new(0_usize));
    let seen = tasks.clone();
    cx.update(|_, cx| {
        cx.subscribe(&page, move |_, event: &DiscoverEvent, _| {
            if matches!(event, DiscoverEvent::TasksChanged) {
                *seen.borrow_mut() += 1;
            }
        })
        .detach();
    });
    page.update_in(cx, |page, window, cx| {
        page.set_inputs(
            vec![library_card(&game, 1)],
            Some(game.id.clone()),
            false,
            cx,
        );
        page.install(
            ProjectKind::ResourcePack,
            "test.one".into(),
            "pack".into(),
            "Pack".into(),
            None,
            window,
            cx,
        );
        assert!(page.state().installing.contains("pack"), "its row says so");
    });
    until(cx, "the install to end", |cx| {
        page.read_with(cx, |page, _| page.state().installing.is_empty())
    });
    let told = cx.update(|_, cx| crate::toast::posted(cx).to_vec());
    assert!(
        told.iter()
            .any(|toast| toast.kind == crate::toast::ToastKind::Error),
        "a file only the source's site hands out fails the install, and says so: {told:?}"
    );
    assert!(*tasks.borrow() > 0, "Activity is told to look again");
}
