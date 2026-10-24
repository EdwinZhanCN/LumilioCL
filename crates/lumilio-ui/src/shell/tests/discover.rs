//! The shell's part of Discover: the game installs go into, the details it
//! keeps in history and draws over everything. The page itself is tested in
//! `features::discover`.

use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};
use lumilio_core::{InstanceRecord, InstanceSettings, Loader, ProjectKind};

use super::super::{LauncherShell, Route};
use crate::features::discover::detail::DetailState;
use crate::features::discover::model::{DiscoverChange, DiscoverQuery, PickGroup, SearchStatus};
use crate::live::{LiveIntent, library_card};

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
        source_project: None,
    }
}

/// What core publishes when `id` is the current game.
fn current(id: &str) -> lumilio_core::Context {
    lumilio_core::Context {
        current_instance: Some(id.to_owned()),
        ..lumilio_core::Context::default()
    }
}

#[gpui::test]
fn discover_leaves_naming_the_target_game_to_the_corner_chip(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.set_library(vec![
                    library_card(&record("main", Loader::Fabric), 1),
                    library_card(&record("side", Loader::Fabric), 1),
                ]);
            },
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("live-discover-target").is_none(),
        "the corner already names the game; Discover adds no banner"
    );

    // The target is the game core says is current, once the library has it,
    // and Discover installs into the same one.
    shell.update(cx, |shell, cx| {
        assert_eq!(shell.install_target(), None);
        shell.follow_context(current("ghost"), cx);
        assert_eq!(shell.install_target(), None, "not a game in the library");
        shell.follow_context(current("side"), cx);
        assert_eq!(shell.install_target(), Some("side"));
        let discover = shell.discover().read(cx);
        assert_eq!(discover.state().target.as_deref(), Some("side"));
    });
}

/// Decision D3: another current game ends browsing for a game, whoever
/// chose it; the game browsed for becoming current keeps it.
#[gpui::test]
fn another_current_game_ends_browsing_for_a_game(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                model.set_library(vec![
                    library_card(&record("fab", Loader::Fabric), 1),
                    library_card(&record("van", Loader::Vanilla), 1),
                ]);
            },
            cx,
        );
    });
    let browsing = |shell: &LauncherShell, cx: &gpui::App| {
        shell.discover().read(cx).state().browsing_for.clone()
    };
    shell.update_in(cx, |shell, window, cx| {
        shell.follow_context(current("van"), cx);
        shell.browse_for("fab".into(), ProjectKind::Mod, window, cx);
        shell.follow_context(current("fab"), cx);
        assert_eq!(browsing(shell, cx).as_deref(), Some("fab"), "its own game");
        shell.follow_context(current("van"), cx);
        assert_eq!(browsing(shell, cx), None);
        assert_eq!(shell.install_target(), Some("van"));
    });
}

#[gpui::test]
fn a_project_opens_in_place_and_back_returns_to_the_same_list(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.discover().clone().update(cx, |page, cx| {
            page.edit_state(
                |state| {
                    state.query = state
                        .query
                        .clone()
                        .apply(DiscoverChange::Include(
                            PickGroup::Category,
                            "magic".to_owned(),
                        ))
                        .apply(DiscoverChange::Page(3));
                    state.search = SearchStatus::Done { total: 500 };
                },
                cx,
            )
        });
    });
    cx.run_until_parked();

    shell.update(cx, |shell, cx| {
        shell.open_project(ProjectKind::Mod, lumilio_core::DEFAULT_SOURCE, "sodium", cx);
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
    shell.read_with(cx, |shell, cx| {
        assert!(shell.detail_project().is_none());
        let query = &shell.discover().read(cx).state().query;
        assert_eq!(query.page, 3, "the page is where it was");
        assert_eq!(
            query.categories,
            [lumilio_core::Pick::include("magic")],
            "the filters are still on"
        );
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
        shell.open_project(ProjectKind::Mod, lumilio_core::DEFAULT_SOURCE, "sodium", cx)
    });
    cx.run_until_parked();
    shell.update_in(cx, |shell, window, cx| {
        shell.select_route(Route::Library, window, cx)
    });
    shell.read_with(cx, |shell, _| assert!(shell.detail_project().is_none()));
}

#[gpui::test]
fn a_gallery_image_shown_large_is_drawn_by_the_shell_over_everything(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(|_: LiveIntent, _, _| {}))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(|_| {}, cx);
        shell.open_project(ProjectKind::Mod, lumilio_core::DEFAULT_SOURCE, "cool", cx);
        shell.set_detail_state(
            "cool",
            DetailState::Ready(Box::new(crate::features::discover::detail::tests::detail())),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("detail-viewer").is_none());
    shell.update(cx, |shell, cx| shell.open_gallery_image(0, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("detail-viewer").is_some(),
        "the viewer is part of the shell's own layer, above the navigation"
    );
    let close = cx.debug_bounds("detail-viewer-close").expect("a close key");
    cx.simulate_click(close.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("detail-viewer").is_none());
}

/// The saved preferences turn translation on for Discover from startup,
/// before the Settings page has loaded anything.
#[gpui::test]
fn translation_follows_the_preferences_without_the_settings_page(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    shell.update(cx, |shell, cx| {
        assert!(shell.live().unwrap().settings.is_none());
        shell.set_translation(true, cx);
        assert!(shell.discover().read(cx).state().translation);
        // A later live refresh does not turn it back off.
        shell.update_live(|model| model.attention_checking = true, cx);
        assert!(shell.discover().read(cx).state().translation);
    });
}

/// What the page lists stays with it when the shell's live model changes.
#[gpui::test]
fn a_live_refresh_leaves_the_search_as_it_was(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    shell.update(cx, |shell, cx| {
        shell.discover().clone().update(cx, |page, cx| {
            page.edit_state(
                |state| state.query = DiscoverQuery::new(ProjectKind::Shader),
                cx,
            )
        });
        shell.update_live(|model| model.attention_checking = true, cx);
        assert_eq!(
            shell.discover().read(cx).state().query.kind,
            ProjectKind::Shader
        );
    });
}
