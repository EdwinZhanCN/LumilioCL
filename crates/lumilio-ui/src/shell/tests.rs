use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext, point};
use lumilio_core::{LaunchPhase, LaunchSignal};

use super::{ActivitySummary, LauncherShell, Route, ShellIntent};
use crate::home::{HomeIntent, HomePresentation, RecentEntry, Subject, WorldHint};

fn continuing() -> HomePresentation {
    HomePresentation::Continue {
        subject: Subject {
            title: "生存".into(),
            metadata: "1.21.1 · Fabric".into(),
            world: WorldHint::Underground,
        },
        recent: vec![RecentEntry {
            id: None,
            title: "空岛".into(),
            metadata: "昨天".into(),
        }],
    }
}

/// Lets real-time entrance animations finish (gpui animations read the
/// wall clock), then draws a fresh frame.
fn settle(cx: &mut gpui::VisualTestContext) {
    std::thread::sleep(crate::theme::motion::SCENE + std::time::Duration::from_millis(120));
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Every quad painted exactly over an element's bounds. GPUI paints a
/// filled, bordered element as a fill quad plus border-only quads.
fn painted_at(cx: &mut gpui::VisualTestContext, selector: &'static str) -> Vec<gpui::Quad> {
    let Some(bounds) = cx.debug_bounds(selector) else {
        return Vec::new();
    };
    cx.update(|window, _| {
        let scale = window.scale_factor();
        let near = |a: f32, b: gpui::Pixels| (a / scale - f32::from(b)).abs() < 1.5;
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| {
                near(quad.bounds.origin.x.0, bounds.origin.x)
                    && near(quad.bounds.origin.y.0, bounds.origin.y)
                    && near(quad.bounds.size.width.0, bounds.size.width)
                    && near(quad.bounds.size.height.0, bounds.size.height)
            })
            .collect()
    })
}

fn fill(quads: &[gpui::Quad]) -> gpui::Hsla {
    quads
        .iter()
        .filter_map(|quad| quad.background.as_solid())
        .find(|color| color.a > 0.)
        .expect("the element paints a fill")
}

#[gpui::test]
fn keys_on_the_world_keep_their_colours_and_shift_on_hover(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(continuing(), cx)
            .with_intent_handler(Rc::new(|_, _, _| {}))
    });
    settle(cx);

    let resume_bg = fill(&painted_at(cx, "home-continue"));
    assert!(
        [
            crate::theme::Body::of(false).orange,
            crate::theme::Body::of(true).orange
        ]
        .contains(&resume_bg),
        "the primary on art is the orange key at rest, got {resume_bg:?}"
    );

    shell.update(cx, |shell, cx| {
        shell.set_home(continuing().begin_launch(), cx);
        shell.apply_launch_signal(LaunchSignal::Running, cx);
    });
    settle(cx);

    let rest = painted_at(cx, "home-stop-game");
    let rest_bg = fill(&rest);
    assert!(
        [
            crate::theme::Body::of(false).key_black,
            crate::theme::Body::of(true).key_black
        ]
        .contains(&rest_bg),
        "the secondary on art is the black key, got {rest_bg:?}"
    );

    let bounds = cx.debug_bounds("home-stop-game").unwrap();
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
    cx.run_until_parked();
    let hovered = fill(&painted_at(cx, "home-stop-game"));
    assert_ne!(hovered, rest_bg, "hovering shifts the key's face");
}

#[gpui::test]
fn pressing_continue_plays_the_launch_moment_through_every_state(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let intents = Rc::new(RefCell::new(Vec::new()));
    let seen = intents.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(continuing(), cx)
            .with_intent_handler(Rc::new(move |intent, _, _| {
                seen.borrow_mut().push(intent);
            }))
    });
    cx.run_until_parked();

    let button = cx
        .debug_bounds("home-continue")
        .expect("Continue is drawn on the hero");
    let column = cx
        .debug_bounds("home-overlay-column")
        .expect("the content column is drawn");
    assert!(
        button.size.width < column.size.width / 3.,
        "Continue keeps its own width instead of stretching ({button:?} in {column:?})"
    );
    cx.simulate_click(button.center(), Modifiers::none());
    assert_eq!(
        *intents.borrow(),
        vec![ShellIntent::Home(HomeIntent::Continue)]
    );
    let home =
        |cx: &mut gpui::VisualTestContext| shell.read_with(cx, |shell, _| shell.home().clone());
    assert!(matches!(home(cx), HomePresentation::Launching { .. }));

    let signals = [
        LaunchSignal::Phase(LaunchPhase::Libraries),
        LaunchSignal::Progress {
            done: 40,
            total: 86,
        },
        LaunchSignal::Phase(LaunchPhase::Assets),
        LaunchSignal::Progress {
            done: 900,
            total: 3412,
        },
        LaunchSignal::Running,
    ];
    for signal in signals {
        shell.update(cx, |shell, cx| shell.apply_launch_signal(signal, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("home-overlay").is_some());
    }
    assert!(matches!(home(cx), HomePresentation::Playing { .. }));

    shell.update(cx, |shell, cx| {
        shell.apply_launch_signal(LaunchSignal::Exited { code: Some(3) }, cx)
    });
    cx.run_until_parked();
    assert!(matches!(home(cx), HomePresentation::Recovery { .. }));
    assert!(cx.debug_bounds("home-overlay").is_some());

    shell.update(cx, |shell, cx| {
        shell.set_home(HomePresentation::FirstUse, cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("home-overlay").is_none());
}

/// The whole Home page scrolls, world included.
#[gpui::test]
fn home_scrolls_under_the_wheel(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

    cx.update(gpui_component::init);
    let (_shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(continuing(), cx)
            .with_intent_handler(Rc::new(|_, _, _| {}))
    });
    cx.simulate_resize(gpui::size(px(1080.), px(500.)));
    cx.run_until_parked();

    let scroll = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-160.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
    };

    let before = cx.debug_bounds("home-body").expect("Home body is drawn");
    // Wheel over the world, not only over the page below it.
    scroll(cx, point(px(300.), px(120.)));
    let after = cx.debug_bounds("home-body").expect("Home body is drawn");
    assert!(
        after.origin.y < before.origin.y - px(10.),
        "Home scrolls when the wheel is over the world ({before:?} → {after:?})"
    );
}

#[test]
fn activity_summary_formats_a_bounded_badge() {
    assert_eq!(ActivitySummary::new(0).badge_label(), None);
    assert_eq!(ActivitySummary::new(4).badge_label(), Some("4".into()));
    assert_eq!(ActivitySummary::new(100).badge_label(), Some("99+".into()));
}

#[gpui::test]
fn live_library_cards_manage_and_explicit_play_is_independent(cx: &mut TestAppContext) {
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
    let record = InstanceRecord {
        id: "survival".to_owned(),
        name: "生存".to_owned(),
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
        shell.show(Route::Library);
        shell.update_live(
            |model| model.set_library(vec![library_card(&record, 10)], None),
            cx,
        );
    });
    cx.run_until_parked();

    let star = cx
        .debug_bounds("live-favorite-0")
        .expect("the star is drawn");
    // Play, star and ⋯ are one row of keys at one height, in that order.
    let play = cx.debug_bounds("live-play-0").expect("play key");
    let more = cx.debug_bounds("live-card-more-0").expect("more key");
    assert_eq!(play.size.height, star.size.height, "play and star");
    assert_eq!(play.size.height, more.size.height, "play and more");
    assert!(play.right() <= star.left() && star.right() <= more.left());
    cx.simulate_click(star.center(), Modifiers::none());
    assert_eq!(
        *seen.borrow(),
        vec![LiveIntent::ToggleFavorite("survival".to_owned())],
        "starring must not also start the game"
    );

    let card = cx.debug_bounds("live-card-0").expect("the card is drawn");
    let body = gpui::point(card.center().x, card.bottom() - gpui::px(12.));
    cx.simulate_click(body, Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::OpenInstance("survival".to_owned()))
    );
    let before = seen.borrow().len();
    let play = cx
        .debug_bounds("live-play-0")
        .expect("explicit play button");
    cx.simulate_click(play.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().len(),
        before + 1,
        "play must not also open details"
    );
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::Play("survival".to_owned()))
    );
}

#[gpui::test]
fn the_collections_tab_shows_each_collection_and_the_card_menu_does_not_open_the_game(
    cx: &mut TestAppContext,
) {
    use crate::live::{CollectionRow, LiveIntent, library_card};
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
        shell.show(Route::Library);
        shell.show_tab(Route::Library, crate::pages::live::COLLECTIONS_TAB);
        shell.update_live(
            |model| {
                model.set_library(
                    vec![
                        library_card(&record("a"), 10),
                        library_card(&record("b"), 10),
                    ],
                    None,
                );
                model.collections = vec![CollectionRow {
                    name: "生存".into(),
                    members: vec!["b".into()],
                }];
            },
            cx,
        );
    });
    cx.run_until_parked();

    // Only the collection's own game is drawn, and its button asks for that game.
    assert!(cx.debug_bounds("live-card-0").is_some());
    assert!(cx.debug_bounds("live-card-1").is_none());
    let more = cx
        .debug_bounds("live-card-more-0")
        .expect("the card's more menu");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        seen.borrow().is_empty(),
        "opening the card's menu must not also open the game: {:?}",
        seen.borrow()
    );

    let new = cx
        .debug_bounds("live-new-collection")
        .expect("new collection");
    cx.simulate_click(new.center(), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&LiveIntent::NewCollection));
}

#[gpui::test]
fn a_failed_task_offers_retry_and_open_and_a_game_that_is_gone_offers_no_open(
    cx: &mut TestAppContext,
) {
    use crate::live::{ActivityRow, ActivityState, LiveIntent, library_card};
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader, RetryAction, TaskCategory};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let record = InstanceRecord {
        id: "here".to_owned(),
        name: "在".to_owned(),
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
    let failed = |instance: &str| ActivityRow {
        task: None,
        amount: None,
        unit: lumilio_core::ProgressUnit::Items,
        rate: None,
        instance: Some(instance.to_owned()),
        retry: Some(RetryAction::RepairInstance {
            instance: instance.to_owned(),
        }),
        category: TaskCategory::Repair,
        title: "修复".into(),
        detail: String::new(),
        fraction: None,
        state: ActivityState::Failed("no route".into()),
        cancel: None,
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Activity);
        shell.update_live(
            |model| {
                model.set_library(vec![library_card(&record, 10)], None);
                model.set_activity(vec![failed("here"), failed("gone")], 1_000);
            },
            cx,
        );
    });
    cx.run_until_parked();

    let retry = cx.debug_bounds("live-retry-0").expect("retry");
    cx.simulate_click(retry.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::RetryTask(RetryAction::RepairInstance {
            instance: "here".into()
        }))
    );
    let open = cx.debug_bounds("live-open-0").expect("open");
    cx.simulate_click(open.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::OpenInstance("here".into()))
    );
    assert!(cx.debug_bounds("live-retry-1").is_some());
    assert!(
        cx.debug_bounds("live-open-1").is_none(),
        "a deleted game cannot be opened"
    );
}

#[gpui::test]
fn home_lists_what_needs_attention_with_one_remedy_and_recent_games_open(cx: &mut TestAppContext) {
    use crate::home::AttentionRow;
    use crate::instance_detail::ProblemAction;
    use crate::live::LiveIntent;

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    shell.update(cx, |shell, cx| {
        shell.set_home(
            HomePresentation::Continue {
                subject: Subject {
                    title: "生存".into(),
                    metadata: "1.21.1 · Fabric".into(),
                    world: WorldHint::Underground,
                },
                recent: vec![RecentEntry {
                    id: Some("sky".into()),
                    title: "空岛".into(),
                    metadata: "昨天".into(),
                }],
            },
            cx,
        );
        shell.update_live(
            |model| {
                model.attention = vec![AttentionRow {
                    instance: "sky".into(),
                    name: "空岛".into(),
                    title: "有游戏文件缺失或损坏".into(),
                    detail: "共 2 个".into(),
                    action: Some((ProblemAction::Repair, "修复")),
                    more: 1,
                }];
            },
            cx,
        );
    });
    settle(cx);

    let act = cx.debug_bounds("home-attention-act-0").expect("remedy");
    cx.simulate_click(act.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::Resolve("sky".into(), ProblemAction::Repair))
    );
    let card = cx.debug_bounds("home-recent-0").expect("recent card");
    cx.simulate_click(card.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::OpenInstance("sky".into()))
    );
}

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
fn changing_the_library_order_or_loader_is_reported_so_it_can_be_remembered(
    cx: &mut TestAppContext,
) {
    use crate::kit::ViewIntent;
    use crate::live::LiveIntent;
    use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let choose = |intent: ViewIntent, cx: &mut gpui::VisualTestContext| {
        cx.update(|window, app| {
            shell.update(app, |shell, cx| {
                shell.apply_view_intent(intent, cx);
                shell.remember_library_view(intent, window, cx);
            })
        });
    };
    choose(ViewIntent::Choose(LIBRARY_SORT, 1), cx);
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::RememberLibraryView { sort: 1, loader: 0 })
    );
    choose(ViewIntent::Choose(LIBRARY_LOADER, 2), cx);
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::RememberLibraryView { sort: 1, loader: 2 })
    );
    // Other choices are not the library's business.
    let before = seen.borrow().len();
    choose(ViewIntent::Choose(7, 1), cx);
    assert_eq!(seen.borrow().len(), before);
}

#[gpui::test]
fn the_library_sorts_and_filters_from_dropdowns_that_follow_what_is_remembered(
    cx: &mut TestAppContext,
) {
    use crate::kit::ViewIntent;
    use crate::live::{LiveIntent, library_card};
    use crate::pages::live::{LIBRARY_LOADER, LIBRARY_SORT, loader_code};
    use gpui_component::select::SelectEvent;
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader};

    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let record = |id: &str, loader: Loader| InstanceRecord {
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
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        shell.update_live(
            |model| {
                model.set_library(
                    vec![
                        library_card(&record("a", Loader::Fabric), 1),
                        library_card(&record("b", Loader::Vanilla), 1),
                    ],
                    None,
                );
            },
            cx,
        );
    });
    cx.run_until_parked();
    let selected = |cx: &mut gpui::VisualTestContext, loader: bool| {
        shell.read_with(cx, |shell, cx| {
            let controls = shell.live_controls.as_ref().unwrap().read(cx);
            let select = if loader {
                &controls.library_loader
            } else {
                &controls.library_sort
            };
            select.read(cx).selected_index(cx).map(|ix| ix.row)
        })
    };
    assert_eq!(selected(cx, true), Some(0), "every loader to begin with");

    // What the application restores shows up in the dropdowns.
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(
            ViewIntent::Choose(LIBRARY_LOADER, loader_code(Loader::Fabric)),
            cx,
        );
        shell.apply_view_intent(ViewIntent::Choose(LIBRARY_SORT, 2), cx);
    });
    cx.run_until_parked();
    assert_eq!(selected(cx, true), Some(2), "全部, 原版, Fabric");
    assert_eq!(selected(cx, false), Some(2));
    assert!(
        seen.borrow().is_empty(),
        "restoring is not a new choice to save"
    );

    // A choice made in a dropdown is applied and reported.
    let sort = shell.read_with(cx, |shell, cx| {
        shell
            .live_controls
            .as_ref()
            .unwrap()
            .read(cx)
            .library_sort
            .clone()
    });
    sort.update(cx, |_, cx| {
        cx.emit(SelectEvent::Confirm(Some("名称".to_owned())))
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::RememberLibraryView {
            sort: 1,
            loader: loader_code(Loader::Fabric) as u8
        })
    );
    assert_eq!(selected(cx, false), Some(1));
}

#[gpui::test]
fn live_instance_back_keeps_library_filters_and_late_data_is_isolated(cx: &mut TestAppContext) {
    use lumilio_core::{InstanceRecord, InstanceSettings, LauncherSettings, Loader};
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell
                .live_controls
                .as_ref()
                .unwrap()
                .read(cx)
                .library_filter
                .clone()
                .update(cx, |input, cx| input.set_value("生存", window, cx));
        })
    });
    let first = shell.update(cx, |shell, cx| {
        shell.open_live_instance("first".into(), |_| Rc::new(|_, _, _| {}), cx)
    });
    let second = shell.update(cx, |shell, cx| {
        shell.open_live_instance("second".into(), |_| Rc::new(|_, _, _| {}), cx)
    });
    first.update(cx, |view, cx| {
        view.loaded(
            Ok((
                InstanceRecord {
                    id: "first".into(),
                    name: "旧响应".into(),
                    game_version: "1.21.1".into(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                    favorite: false,
                    created_at: 1,
                    last_played: None,
                    play_seconds: 0,
                    installed: false,
                    settings: InstanceSettings::default(),
                },
                LauncherSettings::default(),
            )),
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(
        second.read_with(cx, |view, _| view.title().to_owned()),
        "游戏详情"
    );
    assert!(cx.debug_bounds("live-card-0").is_none());
    let title = |cx: &mut gpui::VisualTestContext| {
        shell.read_with(cx, |shell, cx| shell.location_title(cx).to_string())
    };
    assert_eq!(title(cx), "游戏详情");
    let back = cx.debug_bounds("navigation-back").expect("back button");
    // Back walks the instances in the order they were opened, then Library.
    cx.simulate_click(back.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(title(cx), "旧响应", "back to the first instance");
    cx.simulate_click(back.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(title(cx), "游戏库");
    assert!(shell.read_with(cx, |shell, _| shell.live_instance().is_none()));
    assert!(!shell.read_with(cx, |shell, _| shell.can_go_back()));
    shell.read_with(cx, |shell, cx| {
        assert_eq!(
            shell
                .live_controls
                .as_ref()
                .unwrap()
                .read(cx)
                .library_filter
                .read(cx)
                .value(),
            "生存"
        )
    });
}

#[gpui::test]
fn messages_float_as_toasts_and_never_take_room_in_the_page(cx: &mut TestAppContext) {
    use crate::toast::Toast;
    use gpui::{AppContext as _, Entity};
    use gpui_component::WindowExt as _;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
        *keep.borrow_mut() = Some(shell.clone());
        gpui_component::Root::new(shell, window, cx)
    });
    let shell = slot.borrow().clone().expect("shell");
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    let header = cx.debug_bounds("live-library-actions").map(|b| b.origin);
    shell.update(cx, |shell, cx| {
        shell.toast(Toast::error("没有装上 Sodium").technical("disk full"), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    shell.read_with(cx, |shell, _| assert!(shell.pending_toasts().is_empty()));
    assert_eq!(cx.update(|window, cx| window.notifications(cx).len()), 1);
    assert_eq!(
        cx.debug_bounds("live-library-actions").map(|b| b.origin),
        header,
        "the page did not move to make room"
    );
}

#[gpui::test]
fn a_deleted_instance_drops_out_of_history_and_the_chip_retargets(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, library_card};
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader};
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
    });
    let record = |id: &str| InstanceRecord {
        id: id.into(),
        name: id.into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        let cards = vec![library_card(&record("a"), 1), library_card(&record("b"), 1)];
        shell.update_live(|model| model.set_library(cards, Some("a".into())), cx);
    });
    cx.run_until_parked();
    for id in ["a", "b"] {
        shell.update(cx, |shell, cx| {
            shell.open_live_instance(id.into(), |_| Rc::new(|_, _, _| {}), cx);
        });
    }
    // Back to "a", so "b" is the way forward; then "a" is deleted.
    shell.update(cx, |shell, cx| assert!(shell.go_back(None, cx)));
    shell.update(cx, |shell, cx| shell.forget_instance("a", cx));
    cx.run_until_parked();
    shell.read_with(cx, |shell, cx| {
        assert!(shell.live_instance().is_none(), "the deleted one left");
        assert_eq!(shell.location_title(cx), "游戏库");
        assert!(!shell.can_go_back());
    });
    shell.update(cx, |shell, cx| assert!(shell.go_forward(None, cx)));
    shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.live_instance().unwrap().read(cx).id(), "b");
        assert!(!shell.can_go_forward(), "forward never reaches \"a\"");
    });

    // The trailing chip lists every instance and retargets on choice.
    cx.run_until_parked();
    let chip = cx
        .debug_bounds("navigation-instance")
        .expect("current instance");
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let second = cx
        .debug_bounds("navigation-instance-choice-1")
        .expect("the list opened");
    cx.simulate_click(second.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::InstallTarget("b".into()))
    );
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

fn account(name: &str, selected: bool) -> crate::live::AccountRow {
    crate::live::AccountRow {
        key: name.into(),
        name: name.into(),
        uuid: lumilio_core::ProfileId::offline(name).to_string(),
        selected,
        custom_id: false,
        microsoft: false,
        needs_sign_in: false,
    }
}

#[gpui::test]
fn the_account_chip_and_page_choose_add_and_manage_accounts(cx: &mut TestAppContext) {
    use crate::live::LiveIntent;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Accounts);
        shell.update_live(|model| model.accounts_loaded = true, cx);
    });
    cx.run_until_parked();

    // No account yet: the chip offers to add one, and so does the page.
    let chip = cx.debug_bounds("navigation-account").expect("account chip");
    cx.simulate_click(chip.center(), Modifiers::none());
    assert_eq!(seen.borrow().as_slice(), [LiveIntent::NewAccount]);
    // The page offers both kinds: offline on the left, Microsoft on the right.
    let actions = cx
        .debug_bounds("accounts-actions")
        .expect("add on the page");
    let at = |share: f32| {
        point(
            actions.origin.x + actions.size.width * share,
            actions.center().y,
        )
    };
    cx.simulate_click(at(0.2), Modifiers::none());
    assert_eq!(seen.borrow().len(), 2);
    assert_eq!(seen.borrow()[1], LiveIntent::NewAccount);
    cx.simulate_click(at(0.85), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&LiveIntent::MicrosoftSignIn));
    assert!(cx.debug_bounds("account-choose-0").is_none());

    // With accounts: the page lists them, a click on a row selects it.
    seen.borrow_mut().clear();
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| model.accounts = vec![account("Steve", true), account("Alex", false)],
            cx,
        );
    });
    cx.run_until_parked();
    let alex = cx.debug_bounds("account-choose-1").expect("Alex's row");
    cx.simulate_click(alex.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SelectAccount("Alex".into())]
    );

    // The chip lists them too and leads to the page from its foot.
    seen.borrow_mut().clear();
    let chip = cx.debug_bounds("navigation-account").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let second = cx
        .debug_bounds("navigation-account-choice-1")
        .expect("the list opened");
    cx.simulate_click(second.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SelectAccount("Alex".into())]
    );
    shell.update(cx, |shell, cx| {
        shell.show(Route::Library);
        cx.notify();
    });
    cx.run_until_parked();
    let chip = cx.debug_bounds("navigation-account").unwrap();
    cx.simulate_click(chip.center(), Modifiers::none());
    cx.run_until_parked();
    let manage = cx
        .debug_bounds("navigation-account-manage")
        .expect("manage entry");
    cx.simulate_click(manage.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| assert_eq!(shell.route(), Route::Accounts));
}

#[gpui::test]
fn the_settings_page_shows_each_tab_and_edits_open_a_dialog(cx: &mut TestAppContext) {
    use crate::kit::ViewIntent;
    use crate::live::{LiveIntent, SettingsView};
    use crate::pages::settings::TAB_GROUP;
    use gpui::{AppContext as _, Entity};
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| {
            LauncherShell::new(cx)
                .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
        });
        *keep.borrow_mut() = Some(shell.clone());
        gpui_component::Root::new(shell, window, cx)
    });
    let shell = slot.borrow().clone().expect("shell");
    shell.update(cx, |shell, _| shell.show(Route::Settings));
    // Before the settings arrive the page says so instead of showing blanks.
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-appearance").is_none());

    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                model.settings = Some(SettingsView {
                    max_memory_mb: Some(4096),
                    total_memory_mb: Some(16_384),
                    data_dir: "/data".into(),
                    ..SettingsView::default()
                })
            },
            cx,
        );
    });
    cx.run_until_parked();
    for selector in [
        "settings-appearance",
        "settings-after-launch",
        "settings-foreground",
        "settings-motion",
        "settings-language",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} on 通用");
    }

    let tabs = [
        (1, "settings-memory"),
        (2, "settings-java-roots"),
        (3, "settings-data-dir"),
        (4, "settings-version"),
    ];
    for (tab, selector) in tabs {
        shell.update(cx, |shell, cx| {
            shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, tab), cx)
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} on tab {tab}"
        );
    }

    // An edit button opens its dialog; cancelling sends nothing.
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 1), cx)
    });
    cx.run_until_parked();
    let edit = cx
        .debug_bounds("settings-memory-edit")
        .expect("edit button");
    cx.simulate_click(edit.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("settings-save").is_some(),
        "the dialog opened"
    );
    assert!(seen.borrow().is_empty());
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
