use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn discover_leaves_naming_the_target_game_to_the_corner_chip(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, library_card};
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let record = |id: &str| InstanceRecord {
        id: id.to_owned(),
        name: id.to_owned(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Fabric,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings::default(),
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.set_library(
                    vec![
                        library_card(&record("main"), 1),
                        library_card(&record("side"), 1),
                    ],
                    Some("main".into()),
                );
                model.query = crate::live::DiscoverQuery::new(lumilio_core::ProjectKind::Mod);
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-discover-target").is_none(),
        "the corner already names the game; Discover adds no banner"
    );

    shell.update(cx, |shell, cx| shell.set_install_target("side".into(), cx));
    cx.run_until_parked();
    shell.update(cx, |shell, _| {
        assert_eq!(
            shell.live().unwrap().install_target.as_deref(),
            Some("side")
        );
    });
}

#[gpui::test]
fn a_result_the_target_already_has_shows_installed_or_offers_the_update(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, SearchRow, SearchStatus, library_card};
    use lumilio_core::{InstalledProject, InstanceRecord, InstanceSettings, Loader};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let record = InstanceRecord {
        id: "main".to_owned(),
        name: "main".to_owned(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Fabric,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings::default(),
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.set_library(vec![library_card(&record, 1)], Some("main".into()));
                model.query = crate::live::DiscoverQuery::new(lumilio_core::ProjectKind::Mod);
                model.results = (0..3)
                    .map(|n| SearchRow {
                        project_id: format!("P{n}"),
                        kind: lumilio_core::ProjectKind::Mod,
                        slug: format!("mod-{n}"),
                        title: format!("Mod {n}"),
                        author: "a".into(),
                        summary: String::new(),
                        environment: None,
                        categories: Vec::new(),
                        loaders: Vec::new(),
                        downloads: "1".into(),
                        follows: "1".into(),
                        updated: String::new(),
                        icon_url: None,
                        seed: n,
                    })
                    .collect();
                model.search = SearchStatus::Done { total: 3 };
                model.installed.insert(
                    "P0".into(),
                    InstalledProject {
                        file_name: "mod0.jar".into(),
                        version_id: "v1".into(),
                        update: None,
                    },
                );
                model.installed.insert(
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
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::UpdateInstalled {
            kind: lumilio_core::ProjectKind::Mod,
            project: "mod-1".into(),
            title: "Mod 1".into(),
            file_name: "mod1.jar".into(),
            version_id: "v2".into(),
        })
    );
    assert!(cx.debug_bounds("live-installed-2").is_none());
    assert!(cx.debug_bounds("live-update-2").is_none());
}

#[gpui::test]
fn a_project_opens_in_place_and_back_returns_to_the_same_list(cx: &mut TestAppContext) {
    use crate::live::{DiscoverChange, LiveIntent, SearchStatus};
    use crate::project_detail::DetailState;
    use lumilio_core::ProjectKind;

    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.query = model
                    .query
                    .clone()
                    .apply(DiscoverChange::ToggleCategory("magic".to_owned()))
                    .apply(DiscoverChange::Page(3));
                model.search = SearchStatus::Done { total: 500 };
            },
            cx,
        );
    });
    cx.run_until_parked();

    shell.update(cx, |shell, cx| {
        shell.open_detail(ProjectKind::Mod, "sodium", cx);
        shell.set_detail_state("sodium", DetailState::Failed("offline".to_owned()), cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-detail-body").is_some(),
        "the detail is showing"
    );
    assert_eq!(
        shell.read_with(cx, |shell, cx| shell.location_title(cx).to_string()),
        "Mod 详情"
    );

    // A stale answer for another project changes nothing.
    shell.update(cx, |shell, cx| {
        shell.set_detail_state("other", DetailState::Loading, cx);
        assert_eq!(shell.detail_project(), Some((ProjectKind::Mod, "sodium")));
    });

    let back = cx.debug_bounds("navigation-back").unwrap();
    cx.simulate_click(back.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-detail-body").is_none(),
        "back leaves the detail"
    );
    shell.read_with(cx, |shell, _| {
        assert!(shell.detail_project().is_none());
        let query = &shell.live().unwrap().query;
        assert_eq!(query.page, 3, "the page is where it was");
        assert_eq!(query.categories, ["magic"], "the filters are still on");
    });

    // Forward returns to the same detail view, without asking again.
    let forward = cx.debug_bounds("navigation-forward").unwrap();
    cx.simulate_click(forward.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(shell.detail_project(), Some((ProjectKind::Mod, "sodium")));
        assert!(!shell.can_go_forward());
    });
    shell.update(cx, |shell, cx| shell.close_detail(cx));

    // Choosing another landmark also closes an open detail.
    shell.update(cx, |shell, cx| {
        shell.open_detail(ProjectKind::Mod, "sodium", cx)
    });
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    shell.update_in(cx, |shell, window, cx| {
        shell.select_route(Route::Library, window, cx)
    });
    shell.read_with(cx, |shell, _| assert!(shell.detail_project().is_none()));
}

/// Not a check: prints how long the Discover list takes to lay out and
/// paint, so performance work has a number to move. Run with
/// `cargo test -p lumilio-ui frame_cost -- --ignored --nocapture`
/// (add `--release` for an optimized figure).
#[gpui::test]
#[ignore = "a measurement, not a check"]
fn frame_cost_of_a_full_discover_page(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, SearchRow, SearchStatus};
    use lumilio_core::ProjectKind;

    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
    });
    let rows: Vec<SearchRow> = (0..20)
        .map(|n| SearchRow {
            project_id: format!("id{n}"),
            kind: ProjectKind::Modpack,
            slug: format!("p{n}"),
            title: format!("Project number {n}"),
            author: "someone".to_owned(),
            summary: "A long enough summary to wrap onto a second line when the window is narrow, describing the pack.".to_owned(),
            environment: Some(lumilio_core::Environment::ClientAndServer),
            categories: vec!["adventure".into(), "magic".into(), "technology".into(), "quests".into()],
            loaders: vec!["fabric".into()],
            downloads: "12.5 万".to_owned(),
            follows: "4886".to_owned(),
            updated: "3 天前".to_owned(),
            icon_url: Some(format!("https://cdn.example/{n}.png")),
            seed: n,
        })
        .collect();
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.results = rows;
                model.search = SearchStatus::Done { total: 500 };
            },
            cx,
        );
    });
    cx.run_until_parked();
    let frames = 60;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        shell.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let each = start.elapsed() / frames;
    eprintln!("discover frame: {each:?} each over {frames} frames");
}
