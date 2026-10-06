use super::super::{ActivitySummary, LauncherShell, ShellIntent};
use super::{continuing, fill, painted_at, settle};
use crate::home::{HomeIntent, HomePresentation, RecentEntry, Subject, WorldHint};
use gpui::{Modifiers, TestAppContext};
use lumilio_core::{LaunchPhase, LaunchSignal};
use std::cell::RefCell;
use std::rc::Rc;

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
                    id: None,
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

fn places_of(instance: &str) -> crate::live::HomePlaces {
    use crate::live::{HomePlaces, Place, PlaceTarget};
    HomePlaces {
        instance: instance.to_owned(),
        places: vec![Place {
            target: PlaceTarget::World("New World".into()),
            name: "新的世界".into(),
            detail: "世界 · 上次游玩 昨天".into(),
        }],
        play_seconds: 7200,
        worlds: 1,
        servers: 0,
    }
}

/// Under the world: the continued game's worlds go straight in, its record
/// sits on a display, and the recent games are the Library's faceplates.
#[gpui::test]
fn home_enters_a_world_and_shows_recent_games_as_library_cards(cx: &mut TestAppContext) {
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
        recent: vec![RecentEntry {
            id: Some("sky".into()),
            title: "空岛".into(),
            metadata: "昨天".into(),
        }],
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
    let enter = cx
        .debug_bounds("home-place-enter-0")
        .expect("a world to enter");
    cx.simulate_click(enter.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::PlayPlace(
            "survival".into(),
            PlaceTarget::World("New World".into())
        ))
    );
    assert!(
        cx.debug_bounds("live-card-more-0").is_some(),
        "a recent game is the Library's faceplate"
    );
    let card = cx.debug_bounds("home-recent-0").expect("recent card");
    cx.simulate_click(card.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::OpenInstance("sky".into()))
    );

    // While a game is on its way nothing else starts from here.
    shell.update(cx, |shell, cx| {
        shell.set_home(home.clone().begin_launch(), cx)
    });
    settle(cx);
    let count = seen.borrow().len();
    let enter = cx.debug_bounds("home-place-enter-0").expect("still listed");
    cx.simulate_click(enter.center(), Modifiers::none());
    assert_eq!(seen.borrow().len(), count, "a disabled key sends nothing");

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
