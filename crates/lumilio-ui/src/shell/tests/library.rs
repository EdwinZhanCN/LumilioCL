use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

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
