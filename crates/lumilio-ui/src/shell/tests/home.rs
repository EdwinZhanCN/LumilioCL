use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};
use lumilio_core::{LaunchPhase, LaunchSignal};

use super::super::{ActivitySummary, LauncherShell, ShellIntent};
use super::{continuing, fill, open_game, painted_at, settle};
use crate::home::{HomeIntent, HomePresentation, Subject, WorldHint};

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
    shell.read_with(cx, |shell, cx| {
        assert!(
            shell.pending_toasts(cx).is_empty(),
            "Home explains it in place"
        )
    });

    shell.update(cx, |shell, cx| {
        shell.set_home(HomePresentation::FirstUse, cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("home-overlay").is_none());
}

/// A launch that fails or a game that crashes while another page is open is
/// told there too, once, with the failed step's cause behind 技术详情.
#[gpui::test]
fn a_failure_away_from_home_floats_up_as_a_toast(cx: &mut TestAppContext) {
    use lumilio_core::LaunchFailure;

    use crate::route::Route;
    use crate::toast::ToastKind;

    cx.update(gpui_component::init);
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(continuing(), cx)
            .with_intent_handler(Rc::new(|_, _, _| {}))
    });
    let toasts = |cx: &mut gpui::VisualTestContext| {
        shell.read_with(cx, |shell, cx| shell.pending_toasts(cx).to_vec())
    };
    shell.update(cx, |shell, cx| {
        shell.set_home(continuing().begin_launch(), cx);
        shell.show(Route::Library);
        shell.apply_launch_signal(LaunchSignal::Phase(LaunchPhase::Assets), cx);
        shell.apply_launch_signal(
            LaunchSignal::Failed(LaunchFailure::step("checksum mismatch")),
            cx,
        );
        // Stray signals after the failure say nothing more.
        shell.apply_launch_signal(LaunchSignal::Cancelled, cx);
    });
    let shown = toasts(cx);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].kind, ToastKind::Error);
    assert!(
        shown[0]
            .text
            .contains(&continuing().subject().unwrap().title)
    );
    assert_eq!(shown[0].technical.as_deref(), Some("checksum mismatch"));

    shell.update(cx, |shell, cx| {
        shell.set_home(continuing().begin_launch(), cx);
        shell.apply_launch_signal(LaunchSignal::Running, cx);
        shell.apply_launch_signal(LaunchSignal::Exited { code: Some(3) }, cx);
    });
    assert_eq!(toasts(cx).len(), 2, "a crash is told too");

    // A game that quit while starting brings what it printed.
    shell.update(cx, |shell, cx| {
        shell.set_home(continuing().begin_launch(), cx);
        shell.apply_launch_signal(
            LaunchSignal::Failed(LaunchFailure::ExitedEarly {
                code: Some(1),
                output: vec!["Error: could not find main class".into()],
            }),
            cx,
        );
    });
    let shown = toasts(cx);
    assert_eq!(shown.len(), 3);
    assert_eq!(
        shown[2].technical.as_deref(),
        Some("Error: could not find main class")
    );
}

/// The whole Home page scrolls, world included.
#[gpui::test]
fn home_scrolls_under_the_wheel(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

    cx.update(gpui_component::init);
    let (_shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(HomePresentation::FirstUse, cx)
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
fn home_lists_what_needs_attention_with_one_remedy(cx: &mut TestAppContext) {
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
                    id: None,
                    title: "生存".into(),
                    metadata: "1.21.1 · Fabric".into(),
                    world: WorldHint::Underground,
                },
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
    assert_eq!(open_game(&shell, cx).as_deref(), Some("sky"));
    assert!(
        seen.borrow().is_empty(),
        "the shell opens the game's page to resolve it"
    );
}

fn game(id: &str) -> lumilio_core::InstanceRecord {
    lumilio_core::InstanceRecord {
        id: id.to_owned(),
        name: id.to_uppercase(),
        game_version: "1.21.1".to_owned(),
        loader: lumilio_core::Loader::Fabric,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: Some(1),
        play_seconds: 0,
        installed: true,
        settings: lumilio_core::InstanceSettings::default(),
        source_project: None,
    }
}

/// A world, then a server that has answered.
fn places_of(instance: &str) -> crate::live::HomePlaces {
    use lumilio_core::{GameMode, Motd, PackPolicy, ServerEntry, ServerStatus, WorldInfo};

    use crate::live::{HomePlaces, Place, PlaceKind};
    use crate::places::ServerState;
    let world = WorldInfo {
        folder: "New World".into(),
        name: "新的世界".into(),
        last_played_ms: Some(1),
        game_version: Some("1.21.1".into()),
        hardcore: false,
        game_mode: Some(GameMode::Survival),
        cheats: false,
        icon: None,
        damaged: false,
        lock_touched_ms: None,
    };
    let server = ServerEntry {
        name: "朋友的服务器".into(),
        address: "play.example".into(),
        packs: PackPolicy::Ask,
        icon: None,
    };
    let answer = ServerStatus {
        motd: Motd::from_legacy("§aWelcome"),
        online: Some(3),
        max: Some(20),
        version: Some("Paper 1.21".into()),
        latency_ms: Some(42),
        favicon: None,
    };
    HomePlaces {
        instance: instance.to_owned(),
        places: vec![
            Place {
                kind: PlaceKind::World(world),
                picture: None,
                seed: 7,
            },
            Place {
                kind: PlaceKind::Server(server, ServerState::Online(answer)),
                picture: None,
                seed: 8,
            },
        ],
        play_seconds: 7200,
        worlds: 1,
        servers: 1,
    }
}

/// Under the world: the continued game's worlds go straight in, its record
/// sits on a display, and no other game is listed: Home is about the current
/// game only (plan global-context, D8).
#[gpui::test]
fn home_enters_a_world_and_lists_no_other_game(cx: &mut TestAppContext) {
    use crate::live::{LiveIntent, PlaceTarget, library_card};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let home = HomePresentation::Continue {
        subject: Subject {
            id: Some("survival".into()),
            title: "生存".into(),
            metadata: "1.21.1 · Fabric".into(),
            world: WorldHint::Overworld,
        },
    };
    shell.update(cx, |shell, cx| {
        shell.set_home(home.clone(), cx);
        shell.update_live(
            |model| {
                model.library = vec![
                    library_card(&game("survival"), 10),
                    library_card(&game("sky"), 10),
                ];
                model.home_places = Some(places_of("survival"));
            },
            cx,
        );
    });
    settle(cx);

    assert!(
        cx.debug_bounds("home-record").is_some(),
        "the record is drawn"
    );
    // A server says here what it says on the Worlds tab.
    for shown in ["server-players-1", "server-signal-1", "server-motd-1"] {
        assert!(cx.debug_bounds(shown).is_some(), "{shown} is drawn");
    }
    let enter = cx
        .debug_bounds("home-place-enter-0")
        .expect("a world to enter");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    let played = |shell: &gpui::Entity<LauncherShell>, cx: &mut gpui::VisualTestContext| {
        shell.read_with(cx, |shell, _| shell.current_choices.played.clone())
    };
    assert_eq!(
        played(&shell, cx),
        [(
            "survival".to_owned(),
            Some(PlaceTarget::World("New World".into()))
        )]
    );
    assert!(
        cx.debug_bounds("live-card-more-0").is_none(),
        "another game's Library faceplate is on Home"
    );

    // While a game is on its way nothing else starts from here.
    shell.update(cx, |shell, cx| {
        shell.set_home(home.clone().begin_launch(), cx)
    });
    settle(cx);
    let enter = cx.debug_bounds("home-place-enter-0").expect("still listed");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(played(&shell, cx).len(), 1, "a disabled key plays nothing");

    // Another game's worlds never sit under this one.
    shell.update(cx, |shell, cx| {
        shell.set_home(home.clone(), cx);
        shell.update_live(|model| model.home_places = Some(places_of("sky")), cx);
    });
    settle(cx);
    assert!(cx.debug_bounds("home-place-enter-0").is_none());
    assert!(cx.debug_bounds("home-record").is_none());

    // A new game with no worlds yet still shows its record and says where
    // its worlds will appear.
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                model.home_places = Some(crate::live::HomePlaces {
                    places: Vec::new(),
                    play_seconds: 0,
                    worlds: 0,
                    servers: 0,
                    ..places_of("survival")
                })
            },
            cx,
        );
    });
    settle(cx);
    assert!(cx.debug_bounds("home-places-empty").is_some());
    assert!(cx.debug_bounds("home-record").is_some());
    assert!(cx.debug_bounds("home-place-enter-0").is_none());
}

/// Without a wallpaper Home draws no world: its title and Continue sit near
/// the top of the window instead of at the foot of the world (plan
/// translucent-themes, D2).
#[gpui::test]
fn a_bare_home_puts_its_title_at_the_top(cx: &mut TestAppContext) {
    let overlay_top = |cx: &mut TestAppContext, bare: bool| {
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.set_global(crate::theme::Wallpaper {
                bare,
                ..Default::default()
            });
        });
        let (_shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_home(continuing(), cx)
                .with_intent_handler(Rc::new(|_, _, _| {}))
        });
        cx.simulate_resize(gpui::size(gpui::px(1080.), gpui::px(800.)));
        settle(cx);
        assert!(
            cx.debug_bounds("home-continue").is_some(),
            "Continue is drawn"
        );
        f32::from(cx.debug_bounds("home-overlay").expect("the title").origin.y)
    };
    let on_world = overlay_top(cx, false);
    let bare = overlay_top(cx, true);
    assert!(
        bare < 100.,
        "a bare Home starts its title near the top ({bare})"
    );
    assert!(
        on_world > bare + 100.,
        "over the world the title sits at its foot ({on_world} vs {bare})"
    );
}

/// The veil under the navigation is drawn on a solid page only: on a
/// translucent one it would stack into a dark band over the glass.
#[gpui::test]
fn the_navigation_veil_is_left_out_on_a_translucent_page(cx: &mut TestAppContext) {
    let veiled = |cx: &mut TestAppContext, glass: bool| {
        cx.update(|cx| {
            gpui_component::init(cx);
            let local = glass.then(|| crate::theme::Entry {
                spec: crate::theme::parse_file(
                    r##"{"themes":[{"name":"Glass","appearance":"light","colors":{"page":"#e4e4e1bf"}}]}"##,
                )
                .unwrap()
                .remove(0)
                .unwrap(),
                origin: crate::theme::Origin::Local("glass.json".into()),
            });
            cx.set_global(crate::theme::Catalog::with_local(
                local.into_iter().collect(),
                Vec::new(),
            ));
            cx.set_global(crate::theme::Choice(lumilio_core::LookPreferences {
                light_theme: glass.then(|| "Glass".into()),
                ..Default::default()
            }));
            gpui_component::Theme::change(gpui_component::ThemeMode::Light, None, cx);
            crate::theme::tune(cx);
        });
        let (_shell, cx) = cx.add_window_view(|_, cx| {
            LauncherShell::new(cx)
                .with_home(continuing(), cx)
                .with_intent_handler(Rc::new(|_, _, _| {}))
        });
        settle(cx);
        cx.debug_bounds("nav-veil").is_some()
    };
    assert!(veiled(cx, false), "a solid page has the veil");
    // Where the system cannot blur, the translucent page is made solid (D3).
    assert_eq!(
        veiled(cx, true),
        !crate::theme::can_blur(),
        "a translucent page has no veil"
    );
}
