//! The Content tab with a real launcher behind it, over a content source
//! that answers only when the test lets it: the files show before any
//! source has answered, and a switch shows at once (#126, #127).

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use gpui::TestAppContext;
use lumilio_core::ProjectKind;
use lumilio_plugin_api::content::{
    Capabilities, Compatibility, Dependency, FileIdentity, FilterChoices, FilterSupport,
    FingerprintKind, KindSupport, Project, ProjectKind as SourceKind, ProjectSummary, SearchPage,
    SearchQuery, Sort, Version, VersionFile, VersionRef,
};
use lumilio_plugin_api::{
    API_VERSION, ContentSource, HostContext, Manifest, Plugin, PluginError, Words,
};

use super::{create, shell};
use crate::instance_detail::{InstanceIntent, Section, TAB_CONTENT};
use crate::testing::{launcher, until};

/// The identifications asked so far, numbered in the order they came, and
/// from which number on they may answer.
#[derive(Default)]
struct Calls {
    asked: usize,
    answer_from: Option<usize>,
    answered: usize,
}

#[derive(Default)]
struct Held {
    calls: Mutex<Calls>,
    changed: Condvar,
}

impl Held {
    fn asked(&self) -> usize {
        self.calls.lock().expect("calls").asked
    }

    fn answered(&self) -> usize {
        self.calls.lock().expect("calls").answered
    }

    /// Lets the identifications numbered `first` and later answer.
    fn answer_from(&self, first: usize) {
        self.calls.lock().expect("calls").answer_from = Some(first);
        self.changed.notify_all();
    }
}

/// A content source that knows no file and identifies only when let.
struct Source {
    held: Arc<Held>,
}

impl Plugin for Source {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.held".into(),
            name: Words::new("held", "held"),
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

impl ContentSource for Source {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            kinds: vec![KindSupport {
                kind: SourceKind::Mod,
                filters: FilterSupport::default(),
                sorts: vec![Sort::Relevance],
                browse_url: None,
            }],
            fingerprints: vec![FingerprintKind::Sha1],
            ..Default::default()
        }
    }

    fn search(&self, _: &dyn HostContext, _: &SearchQuery) -> Result<SearchPage, PluginError> {
        Ok(SearchPage {
            hits: Vec::new(),
            total_hits: 0,
        })
    }

    fn project(&self, _: &dyn HostContext, id: &str) -> Result<Project, PluginError> {
        Err(PluginError::Unavailable(format!("no project {id}")))
    }

    fn versions(&self, _: &dyn HostContext, _: &str, _: bool) -> Result<Vec<Version>, PluginError> {
        Ok(Vec::new())
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
        let mut calls = self.held.calls.lock().expect("calls");
        let call = calls.asked;
        calls.asked += 1;
        // Well inside the host's budget, so the source is never stopped.
        let (mut calls, _) = self
            .held
            .changed
            .wait_timeout_while(calls, Duration::from_secs(4), |calls| {
                calls.answer_from.is_none_or(|first| call < first)
            })
            .expect("calls");
        calls.answered += 1;
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

#[gpui::test]
fn files_show_before_the_sources_answer_and_a_switch_shows_at_once(cx: &mut TestAppContext) {
    let held = Arc::new(Held::default());
    let launcher = launcher(vec![
        Arc::new(Source { held: held.clone() }) as Arc<dyn Plugin>
    ]);
    let id = create(&launcher, "生存");
    let mods = launcher.worker.service().game_dir(&id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a mod").unwrap();
    let (shell, _, cx) = shell(&launcher, cx);
    let view = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open_game(id.clone(), None, window, cx)
        })
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_CONTENT, window, cx)));
    let shows = |cx: &mut gpui::VisualTestContext, file: &str, on: bool| {
        view.read_with(cx, |view, _| {
            view.identifying(ProjectKind::Mod)
                && view.listed_files(ProjectKind::Mod) == [(file.to_owned(), on)]
        })
    };
    until(cx, "the file, while the source is still asked", |cx| {
        shows(cx, "a.jar", true)
    });
    until(cx, "the source to be asked", |_| held.asked() > 0);
    let first = held.asked();

    cx.update(|window, cx| {
        let work = shell.read(cx).game().read(cx).work().expect("a worker");
        let intent = InstanceIntent::SetContent {
            kind: ProjectKind::Mod,
            files: vec!["a.jar".into()],
            enabled: false,
        };
        work.answer(&id, view.downgrade(), intent, window, cx);
    });
    until(cx, "the switched file, before any source answered", |cx| {
        shows(cx, "a.jar.disabled", false)
    });

    // The read after the switch answers first; the one from before it,
    // which saw the file still on, answers last and must not be shown.
    until(cx, "the source to be asked again", |_| held.asked() > first);
    held.answer_from(first);
    until(cx, "the newer answer", |cx| {
        view.read_with(cx, |view, _| !view.identifying(ProjectKind::Mod))
    });
    held.answer_from(0);
    until(cx, "the older answer", |_| held.answered() == held.asked());
    std::thread::sleep(Duration::from_millis(300));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.listed_files(ProjectKind::Mod),
            [("a.jar.disabled".to_owned(), false)]
        );
    });
    drop(launcher.runtime);
}

/// What a source said is kept: reading the tab again does not ask it, the
/// refresh key does (#127).
#[gpui::test]
fn a_kept_answer_stands_until_the_refresh_key_asks_again(cx: &mut TestAppContext) {
    let held = Arc::new(Held::default());
    held.answer_from(0);
    let launcher = launcher(vec![
        Arc::new(Source { held: held.clone() }) as Arc<dyn Plugin>
    ]);
    let id = create(&launcher, "生存");
    let mods = launcher.worker.service().game_dir(&id).join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    std::fs::write(mods.join("a.jar"), b"a mod").unwrap();
    let (shell, _, cx) = shell(&launcher, cx);
    let view = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open_game(id.clone(), None, window, cx)
        })
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_CONTENT, window, cx)));
    let settled = |cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, _| {
            !view.identifying(ProjectKind::Mod) && view.listed_files(ProjectKind::Mod).len() == 1
        })
    };
    until(cx, "the first answer", settled);
    assert_eq!(held.asked(), 1);

    let mods = Section::Content(ProjectKind::Mod);
    cx.update(|window, cx| view.update(cx, |view, cx| view.request(mods, window, cx)));
    until(cx, "the list read again", settled);
    assert_eq!(held.asked(), 1, "the kept answer stands");

    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.identify_again(ProjectKind::Mod, window, cx)
        })
    });
    until(cx, "the source to be asked again", |_| held.asked() == 2);
    until(cx, "the list identified again", settled);
    drop(launcher.runtime);
}

/// A file that is gone says so, not that a service was out of reach (#126).
#[gpui::test]
fn a_switch_of_a_file_that_is_gone_says_which_file(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let id = create(&launcher, "生存");
    let (shell, _, cx) = shell(&launcher, cx);
    let view = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open_game(id.clone(), None, window, cx)
        })
    });
    cx.update(|window, cx| {
        let work = shell.read(cx).game().read(cx).work().expect("a worker");
        let intent = InstanceIntent::SetContent {
            kind: ProjectKind::Mod,
            files: vec!["gone.jar".into()],
            enabled: false,
        };
        work.answer(&id, view.downgrade(), intent, window, cx);
    });
    until(cx, "the failure to be told", |cx| {
        cx.update(|_, cx| !crate::toast::posted(cx).is_empty())
    });
    let technical = cx.update(|_, cx| crate::toast::posted(cx)[0].technical.clone());
    assert_eq!(
        technical.as_deref(),
        Some("gone.jar: \"gone.jar\" is not installed")
    );
    drop(launcher.runtime);
}
