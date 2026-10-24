//! The Discover page on its own: what it shows, what its controls change and
//! what it tells the application. The shell's part (history, the current
//! game) is tested with the shell.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};
use lumilio_core::{
    CategoryTag, ContentSourceInfo, DiscoverFilters, GameVersionTag, InstalledProject,
    InstanceRecord, InstanceSettings, Loader, LoaderTag, Pick, ProjectKind,
};

use super::super::model::{
    CardTag, CardTags, CountKind, DateKind, DiscoverChange, DiscoverQuery, FilterModel, Lock,
    SearchRow, SearchStatus,
};
use super::super::view::DiscoverIntent;
use super::super::{Discover, DiscoverEvent};
use crate::live::{LibraryCard, library_card};

fn record(id: &str, loader: Loader) -> InstanceRecord {
    InstanceRecord {
        id: id.to_owned(),
        name: id.to_owned(),
        game_version: "1.21.1".to_owned(),
        loader,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings::default(),
        source_project: Some(format!("pack-{id}")),
    }
}

fn games() -> Vec<LibraryCard> {
    vec![
        library_card(&record("fab", Loader::Fabric), 1),
        library_card(&record("van", Loader::Vanilla), 1),
    ]
}

fn row(n: u32, kind: ProjectKind) -> SearchRow {
    SearchRow {
        source: lumilio_core::DEFAULT_SOURCE.into(),
        project_id: format!("P{n}"),
        kind,
        slug: format!("mod-{n}"),
        title: format!("Mod {n}"),
        author: "a".into(),
        summary: String::new(),
        translated_summary: None,
        environment: None,
        tags: CardTags::default(),
        count: "1".into(),
        count_kind: CountKind::Downloads,
        date: String::new(),
        date_kind: DateKind::Updated,
        page_url: String::new(),
        icon_url: None,
        seed: n,
        manual_download: false,
    }
}

/// The page in a window of its own, with nothing to ask.
fn page(cx: &mut TestAppContext) -> (Entity<Discover>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    cx.add_window_view(|_, _| Discover::new(None))
}

/// What the page tells the application, as it says it.
fn remembered(
    page: &Entity<Discover>,
    cx: &mut VisualTestContext,
) -> Rc<RefCell<Vec<lumilio_core::DiscoverPreferences>>> {
    let seen: Rc<RefCell<Vec<_>>> = Rc::default();
    let sink = seen.clone();
    cx.update(|_, cx| {
        cx.subscribe(page, move |_, event: &DiscoverEvent, _| {
            if let DiscoverEvent::Remember(prefs) = event {
                sink.borrow_mut().push(prefs.clone());
            }
        })
        .detach();
    });
    seen
}

/// Two games installs could go into and the filter lists, browsing mods.
fn with_games(page: &Entity<Discover>, target: Option<&str>, cx: &mut VisualTestContext) {
    page.update(cx, |page, cx| {
        page.set_inputs(games(), target.map(str::to_owned), false, cx);
        page.edit_state(
            |state| {
                state.filters = FilterModel::from_core(&DiscoverFilters {
                    categories: vec![CategoryTag {
                        id: "magic".into(),
                        name: "magic".into(),
                        header: "categories".into(),
                        kind: ProjectKind::Mod,
                    }],
                    game_versions: vec![GameVersionTag {
                        version: "1.21.1".into(),
                        release: true,
                        snapshot: false,
                        published: String::new(),
                    }],
                    loaders: vec![LoaderTag {
                        name: "fabric".into(),
                        project_types: vec!["mod".into(), "modpack".into()],
                    }],
                    ..Default::default()
                });
                state.query = DiscoverQuery::new(ProjectKind::Mod);
                state.search = SearchStatus::Done { total: 0 };
            },
            cx,
        );
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_result_the_target_already_has_shows_installed_or_offers_the_update(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    page.update(cx, |page, cx| {
        page.set_inputs(games(), Some("fab".into()), false, cx);
        page.edit_state(
            |state| {
                state.query = DiscoverQuery::new(ProjectKind::Mod);
                state.results = (0..3).map(|n| row(n, ProjectKind::Mod)).collect();
                state.search = SearchStatus::Done { total: 3 };
                state.installed.insert(
                    "P0".into(),
                    InstalledProject {
                        file_name: "mod0.jar".into(),
                        version_id: "v1".into(),
                        update: None,
                    },
                );
                state.installed.insert(
                    "P1".into(),
                    InstalledProject {
                        file_name: "mod1.jar".into(),
                        version_id: "v1".into(),
                        update: Some("v2".into()),
                    },
                );
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("live-installed-0").is_some(), "has it");
    let update = cx.debug_bounds("live-update-1").expect("newer version");
    cx.simulate_click(update.center(), Modifiers::none());
    page.read_with(cx, |page, _| {
        assert_eq!(
            page.acted.last(),
            Some(&DiscoverIntent::UpdateInstalled {
                kind: ProjectKind::Mod,
                source: lumilio_core::DEFAULT_SOURCE.into(),
                project: "mod-1".into(),
                title: "Mod 1".into(),
                file_name: "mod1.jar".into(),
                version_id: "v2".into(),
            })
        );
    });
    assert!(cx.debug_bounds("live-installed-2").is_none());
    assert!(cx.debug_bounds("live-update-2").is_none());
}

#[gpui::test]
fn browsing_for_a_game_locks_its_filters_and_trims_the_tabs(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    with_games(&page, Some("fab"), cx);
    let before = page.read_with(cx, |page, _| page.search_seq);
    page.update_in(cx, |page, window, cx| {
        page.browse_for("fab".into(), ProjectKind::Mod, window, cx);
    });
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        let state = page.state();
        assert_eq!(state.browsing_for.as_deref(), Some("fab"));
        assert_eq!(state.query.kind, ProjectKind::Mod);
        assert_eq!(
            state.discover_kinds().len(),
            3,
            "no modpack tab inside a game"
        );
        assert_eq!(state.provided().loader.as_deref(), Some("fabric"));
        assert!(page.search_seq > before, "the locked search went out");
    });
    assert!(
        cx.debug_bounds("live-unlock-version").is_some(),
        "the version is locked"
    );
    assert!(
        cx.debug_bounds("live-unlock-loader").is_some(),
        "the loader is locked"
    );

    // Releasing a lock turns it into the person's own filter.
    page.update_in(cx, |page, window, cx| {
        page.change_query(DiscoverChange::Unlock(Lock::Loader), window, cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("live-unlock-loader").is_none());
    assert!(cx.debug_bounds("live-sync-loader").is_some());

    // Plain browsing again (choosing Discover in the navigation).
    page.update(cx, |page, cx| page.leave_browse_context(cx));
    page.read_with(cx, |page, _| assert_eq!(page.state().browsing_for, None));
}

/// Decision D3: another game installs go into ends browsing for a game; the
/// game browsed for becoming the target keeps it.
#[gpui::test]
fn another_target_ends_browsing_for_a_game(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    with_games(&page, Some("van"), cx);
    page.update_in(cx, |page, window, cx| {
        page.browse_for("fab".into(), ProjectKind::Mod, window, cx);
        page.set_inputs(games(), Some("fab".into()), false, cx);
        assert_eq!(page.state().browsing_for.as_deref(), Some("fab"));
        page.set_inputs(games(), Some("van".into()), false, cx);
        assert_eq!(
            page.state().browsing_for,
            None,
            "its version no longer applies"
        );
        assert_eq!(page.state().target.as_deref(), Some("van"));
    });
}

#[gpui::test]
fn a_vanilla_game_gets_no_mod_tab_and_a_filter_row_asks_for_its_option(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    with_games(&page, None, cx);
    page.update_in(cx, |page, window, cx| {
        page.browse_for("van".into(), ProjectKind::ResourcePack, window, cx)
    });
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        assert_eq!(
            page.state().discover_kinds(),
            [ProjectKind::ResourcePack, ProjectKind::Shader]
        );
    });

    // Plain browsing of mods: clicking a loader asks for it.
    page.update(cx, |page, cx| {
        page.leave_browse_context(cx);
        page.edit_state(
            |state| state.query = DiscoverQuery::new(ProjectKind::Mod),
            cx,
        );
    });
    cx.run_until_parked();
    // The loader section starts closed in plain browsing; open it.
    let head = cx
        .debug_bounds("live-section-1")
        .expect("the loader section head");
    cx.simulate_click(head.center(), Modifiers::none());
    cx.run_until_parked();
    let row = cx
        .debug_bounds("live-pick-loader-fabric")
        .expect("a loader row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        assert_eq!(page.state().query.loaders, vec![Pick::include("fabric")]);
        assert_eq!(page.state().search, SearchStatus::Searching);
    });
}

#[gpui::test]
fn hide_installed_is_for_packs_and_the_pack_choice_is_remembered(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    let seen = remembered(&page, cx);
    with_games(&page, None, cx);
    assert!(
        cx.debug_bounds("live-hide-installed-cap").is_none(),
        "not offered for plain mod browsing"
    );
    page.update(cx, |page, cx| {
        page.edit_state(
            |state| state.query = DiscoverQuery::new(ProjectKind::Modpack),
            cx,
        )
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-hide-installed-label").is_some(),
        "the switch says what it does"
    );
    let cap = cx
        .debug_bounds("live-hide-installed-cap")
        .expect("offered on the pack tab");
    cx.simulate_click(cap.center(), Modifiers::none());
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        assert!(page.state().discover_prefs.hide_installed_modpacks);
        assert!(page.state().hiding_installed());
    });
    assert!(
        seen.borrow()
            .iter()
            .any(|prefs| prefs.hide_installed_modpacks),
        "the choice goes to the application to keep"
    );
}

#[gpui::test]
fn choosing_another_source_searches_there_and_resets_its_filters(cx: &mut TestAppContext) {
    let source = |id: &str, name: &str| ContentSourceInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        catalog: None,
    };
    let (page, cx) = page(cx);
    let seen = remembered(&page, cx);
    page.update(cx, |page, cx| {
        page.edit_state(
            |state| {
                state.query = DiscoverQuery::new(ProjectKind::Mod);
                state.sources = vec![source(lumilio_core::DEFAULT_SOURCE, "Modrinth")];
                state.filters.loaded = true;
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-discover-sources").is_none(),
        "one source needs no choice"
    );

    page.update(cx, |page, cx| {
        page.edit_state(
            |state| {
                state.sources = vec![
                    source(lumilio_core::DEFAULT_SOURCE, "Modrinth"),
                    source("lumilio.curseforge", "CurseForge"),
                ];
            },
            cx,
        );
    });
    cx.run_until_parked();
    let before = page.read_with(cx, |page, _| page.search_seq);
    let key = cx
        .debug_bounds("live-discover-source-key-1")
        .expect("both sources are offered");
    cx.simulate_click(key.center(), Modifiers::none());
    cx.run_until_parked();
    page.read_with(cx, |page, _| {
        let state = page.state();
        assert_eq!(state.query.source, "lumilio.curseforge");
        assert_eq!(state.query.kind, ProjectKind::Mod);
        assert!(!state.filters.loaded, "its own filters are to be loaded");
        assert!(page.search_seq > before, "searched there");
    });
    assert_eq!(
        seen.borrow()
            .last()
            .and_then(|prefs| prefs.source.as_deref()),
        Some("lumilio.curseforge")
    );
}

#[gpui::test]
fn turning_translation_off_puts_the_original_summaries_back(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    let mut translated = row(1, ProjectKind::Mod);
    translated.summary = "A rendering engine.".to_owned();
    translated.translated_summary = Some("渲染引擎。".to_owned());
    page.update(cx, |page, cx| {
        page.set_inputs(Vec::new(), None, true, cx);
        page.edit_state(|state| state.results = vec![translated], cx);
    });
    let shown = |page: &Entity<Discover>, cx: &mut VisualTestContext| {
        page.read_with(cx, |page, _| {
            let row = &page.state().results[0];
            (row.summary.clone(), row.translated_summary.clone())
        })
    };
    assert_eq!(
        shown(&page, cx),
        ("A rendering engine.".into(), Some("渲染引擎。".into()))
    );
    page.update(cx, |page, cx| page.set_inputs(Vec::new(), None, false, cx));
    assert_eq!(shown(&page, cx), ("A rendering engine.".into(), None));
}

#[gpui::test]
fn result_rows_stay_short_and_a_narrow_window_folds_the_filters(cx: &mut TestAppContext) {
    use gpui::{px, size};

    let (page, cx) = page(cx);
    // A long name, a summary that would take several lines and a full set of
    // tags: everything that used to make a row tall.
    let mut long = row(1, ProjectKind::Modpack);
    long.title = "All the Mods 10 - ATM10 - To the Sky and Beyond".to_owned();
    long.author = "ATMTeam".to_owned();
    long.summary = "A kitchen-sink pack with hundreds of mods, quests that guide you through \
                    every one of them and a long road to the star at the end."
        .to_owned();
    long.environment = Some(lumilio_core::Environment::ClientAndServer);
    long.tags = CardTags {
        shown: ["adventure", "kitchen-sink", "magic", "technology"]
            .into_iter()
            .map(|name| CardTag {
                name: name.into(),
                loader: false,
            })
            .collect(),
        overflow: vec!["quests".into(), "multiplayer".into()],
    };
    long.count = "1905.7 万".to_owned();
    long.date = "10 个月前".to_owned();
    page.update(cx, |page, cx| {
        page.edit_state(
            |state| {
                state.results = vec![long];
                state.search = SearchStatus::Done { total: 1 };
            },
            cx,
        );
    });
    let short = |cx: &mut VisualTestContext, window: &str| {
        let line = cx.debug_bounds("live-result-0").expect("the result line");
        assert!(
            line.size.height <= px(100.),
            "a {window} row stays one short line: {line:?}"
        );
        cx.debug_bounds("live-result-name-0")
            .expect("the name column")
            .size
            .width
    };

    // The default window keeps the filter column beside the list.
    cx.simulate_resize(size(px(1080.), px(720.)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("live-discover-sidebar").is_some());
    assert!(cx.debug_bounds("live-discover-filters").is_none());
    let name = short(cx, "default");
    assert!(name >= px(380.), "the name keeps its room: {name:?}");

    // The smallest window folds the column into a key, so the name has room.
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-discover-sidebar").is_none(),
        "no column beside the list"
    );
    let name = short(cx, "smallest");
    assert!(name >= px(280.), "the name keeps its room: {name:?}");

    // The key opens the same filters in a panel.
    let key = cx
        .debug_bounds("live-discover-filters")
        .expect("the filters key");
    assert!(cx.debug_bounds("live-filter-panel").is_none());
    cx.simulate_click(key.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-filter-panel").is_some(),
        "the key opens the panel"
    );
    assert!(
        cx.debug_bounds("live-section-0").is_some(),
        "the panel holds the filter sections"
    );
}

/// Not a check: prints how long the Discover list takes to lay out and
/// paint, so performance work has a number to move. Run with
/// `cargo nextest run -p lumilio-ui frame_cost --ignored --no-capture`
/// (add `--release` for an optimized figure).
#[gpui::test]
#[ignore = "a measurement, not a check"]
fn frame_cost_of_a_full_discover_page(cx: &mut TestAppContext) {
    let (page, cx) = page(cx);
    let rows: Vec<SearchRow> = (0..20)
        .map(|n| {
            let mut row = row(n, ProjectKind::Modpack);
            row.title = format!("Project number {n}");
            row.summary = "A long enough summary to wrap onto a second line when the window is \
                           narrow, describing the pack."
                .to_owned();
            row.environment = Some(lumilio_core::Environment::ClientAndServer);
            row.tags = CardTags {
                shown: ["adventure", "magic", "technology", "quests"]
                    .into_iter()
                    .map(|name| CardTag {
                        name: name.into(),
                        loader: false,
                    })
                    .collect(),
                overflow: Vec::new(),
            };
            row.icon_url = Some(format!("https://cdn.example/{n}.png"));
            row
        })
        .collect();
    page.update(cx, |page, cx| {
        page.edit_state(
            |state| {
                state.results = rows;
                state.search = SearchStatus::Done { total: 500 };
            },
            cx,
        );
    });
    cx.run_until_parked();
    let frames = 60;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        page.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let each = start.elapsed() / frames;
    eprintln!("discover frame: {each:?} each over {frames} frames");
}
