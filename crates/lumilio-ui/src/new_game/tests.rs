use super::*;

fn version(version: &str, stable: bool) -> LoaderVersion {
    LoaderVersion {
        version: version.to_owned(),
        stable,
    }
}

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn catalog() -> Vec<CatalogEntry> {
    let doc = r#"{"latest":{"release":"26.3"},"versions":[
        {"id":"26.4-snapshot-1","type":"snapshot","url":"https://x/a","releaseTime":"2026-09-20T00:00:00+00:00"},
        {"id":"26.3","type":"release","url":"https://x/b","releaseTime":"2026-09-01T00:00:00+00:00"},
        {"id":"26.2","type":"release","url":"https://x/c","releaseTime":"2026-06-01T00:00:00+00:00"}]}"#;
    lumilio_core::VersionCatalog::decode_json(doc)
        .unwrap()
        .entries()
        .to_vec()
}

#[gpui::test]
fn the_dialog_asks_for_what_it_shows_and_creates_once(cx: &mut gpui::TestAppContext) {
    use gpui::Modifiers;
    use std::cell::RefCell;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<NewGameIntent>>> = Rc::default();
    let sink = seen.clone();
    let form = cx.update(|window, cx| {
        cx.new(|cx| {
            NewGameForm::new(
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| NewGameForm::open(form.clone(), window, cx));
    cx.run_until_parked();
    assert_eq!(seen.borrow().as_slice(), [NewGameIntent::LoadGameVersions]);

    form.update(cx, |form, cx| form.game_versions_arrived(Ok(catalog()), cx));
    cx.run_until_parked();
    // The newest release is chosen, and the render asks for its Fabric versions once.
    assert_eq!(
        seen.borrow().last(),
        Some(&NewGameIntent::LoadLoaderVersions {
            loader: Loader::Fabric,
            game_version: "26.3".into()
        })
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        seen.borrow().len(),
        2,
        "no second request for the same pair"
    );
    assert!(
        cx.debug_bounds("new-game-create").is_some(),
        "the dialog is on screen"
    );

    form.update(cx, |form, cx| {
        form.loader_versions_arrived(
            Loader::Fabric,
            "26.3",
            Ok(vec![version("0.19.6-beta", false), version("0.19.5", true)]),
            cx,
        )
    });
    // A late answer for another pair changes nothing.
    form.update(cx, |form, cx| {
        form.loader_versions_arrived(Loader::Quilt, "26.3", Ok(vec![version("9", true)]), cx)
    });
    cx.run_until_parked();
    let create = cx.debug_bounds("new-game-create").unwrap();
    cx.simulate_click(create.center(), Modifiers::none());
    cx.simulate_click(create.center(), Modifiers::none());
    let creates: Vec<_> = seen
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            NewGameIntent::Create(request) => Some(request.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        creates,
        [NewGameRequest {
            name: "Fabric 26.3".into(),
            loader: Loader::Fabric,
            game_version: "26.3".into(),
            loader_version: Some("0.19.5".into()),
            install: true,
        }],
        "stable Fabric, the default name, once"
    );

    // A failure stays in the dialog, with the form usable again.
    form.update(cx, |form, cx| {
        form.created(Err(("没有创建成功".into(), "boom".into())), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("new-game-error").is_some());
    assert!(cx.debug_bounds("dialog-layer").is_some(), "still open");

    // Success closes it.
    form.update(cx, |form, cx| form.created(Ok(()), cx));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("dialog-layer").is_none(), "closed");
}

/// The caller's handler runs while the person presses the button, and a
/// change that is sent as a write finishes the form from inside it. That
/// used to update the form while its own click was still updating it.
#[gpui::test]
fn a_handler_may_finish_the_form_it_was_called_from(cx: &mut gpui::TestAppContext) {
    use gpui::Modifiers;
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let form = cx.update(|window, cx| {
        cx.new(|cx| {
            let weak: gpui::WeakEntity<NewGameForm> = cx.weak_entity();
            NewGameForm::for_change(
                Rc::new(move |intent, _: &mut Window, cx: &mut App| {
                    if matches!(intent, NewGameIntent::Create(_)) {
                        let _ = weak.update(cx, |form, cx| form.created(Ok(()), cx));
                    }
                }),
                RuntimeChange {
                    game_version: "26.2".into(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.19.5".into()),
                    snapshot: Rc::new(|_, _| {}),
                },
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| NewGameForm::open(form.clone(), window, cx));
    cx.run_until_parked();
    form.update(cx, |form, cx| form.game_versions_arrived(Ok(catalog()), cx));
    cx.run_until_parked();
    form.update(cx, |form, cx| {
        form.game
            .update(cx, |picker, cx| picker.select("26.3".into(), cx))
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    form.update(cx, |form, cx| {
        form.loader_versions_arrived(
            Loader::Fabric,
            "26.3",
            Ok(vec![version("0.19.5", true)]),
            cx,
        )
    });
    cx.run_until_parked();

    let create = cx.debug_bounds("new-game-create").unwrap();
    cx.simulate_click(create.center(), Modifiers::none());
    cx.run_until_parked();
    form.read_with(cx, |form, _| {
        assert!(!form.busy, "the handler's own finish reached the form");
    });
}

#[gpui::test]
fn changing_a_game_starts_on_what_it_is_on_and_only_a_different_choice_can_be_sent(
    cx: &mut gpui::TestAppContext,
) {
    use gpui::Modifiers;
    use std::cell::{Cell, RefCell};
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (_, cx) = cx.add_window_view(|window, cx| {
        let host = cx.new(|_| Host);
        gpui_component::Root::new(host, window, cx)
    });
    let seen: Rc<RefCell<Vec<NewGameIntent>>> = Rc::default();
    let sink = seen.clone();
    let snapshots: Rc<Cell<u32>> = Rc::default();
    let counted = snapshots.clone();
    let form = cx.update(|window, cx| {
        cx.new(|cx| {
            NewGameForm::for_change(
                Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
                RuntimeChange {
                    game_version: "26.2".into(),
                    loader: Loader::Fabric,
                    loader_version: Some("0.19.5".into()),
                    snapshot: Rc::new(move |_, _| counted.set(counted.get() + 1)),
                },
                window,
                cx,
            )
        })
    });
    cx.update(|window, cx| NewGameForm::open(form.clone(), window, cx));
    cx.run_until_parked();
    form.update(cx, |form, cx| form.game_versions_arrived(Ok(catalog()), cx));
    cx.run_until_parked();
    // It asks for the Fabric versions of the game's own version, not the newest.
    assert_eq!(
        seen.borrow().last(),
        Some(&NewGameIntent::LoadLoaderVersions {
            loader: Loader::Fabric,
            game_version: "26.2".into()
        })
    );
    cx.update(|window, cx| window.draw(cx).clear(cx));
    form.update(cx, |form, cx| {
        form.loader_versions_arrived(
            Loader::Fabric,
            "26.2",
            Ok(vec![version("0.19.6-beta", false), version("0.19.5", true)]),
            cx,
        )
    });
    cx.run_until_parked();

    // The name and download choices belong to creating; the warning and
    // the snapshot link belong to changing.
    assert!(cx.debug_bounds("new-game-change-warning").is_some());
    let before = seen.borrow().len();
    let create = cx.debug_bounds("new-game-create").unwrap();
    cx.simulate_click(create.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().len(),
        before,
        "the same combination cannot be sent"
    );
    form.read_with(cx, |form, cx| assert!(form.request(cx).is_none()));

    // The snapshot link runs the caller's snapshot and changes nothing here.
    let snapshot = cx.debug_bounds("new-game-snapshot").unwrap();
    cx.simulate_click(snapshot.center(), Modifiers::none());
    assert_eq!(snapshots.get(), 1);

    // Another game version: its stable Fabric version is chosen and can be sent.
    form.update(cx, |form, cx| {
        form.game
            .update(cx, |picker, cx| picker.select("26.3".into(), cx))
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    form.update(cx, |form, cx| {
        form.loader_versions_arrived(
            Loader::Fabric,
            "26.3",
            Ok(vec![version("0.19.6-beta", false), version("0.19.5", true)]),
            cx,
        )
    });
    cx.run_until_parked();
    let create = cx.debug_bounds("new-game-create").unwrap();
    cx.simulate_click(create.center(), Modifiers::none());
    let sent: Vec<_> = seen
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            NewGameIntent::Create(request) => Some(request.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        sent,
        [NewGameRequest {
            name: String::new(),
            loader: Loader::Fabric,
            game_version: "26.3".into(),
            loader_version: Some("0.19.5".into()),
            install: false,
        }]
    );
}

#[test]
fn stable_latest_and_other_resolve_to_a_listed_version() {
    let versions = [
        version("0.19.6-beta", false),
        version("0.19.5", true),
        version("0.19.4", true),
    ];
    assert_eq!(
        resolve(LoaderPick::Stable, &versions, None).as_deref(),
        Some("0.19.5")
    );
    assert_eq!(
        resolve(LoaderPick::Latest, &versions, None).as_deref(),
        Some("0.19.6-beta")
    );
    assert_eq!(
        resolve(LoaderPick::Other, &versions, Some("0.19.4")).as_deref(),
        Some("0.19.4")
    );
    assert_eq!(resolve(LoaderPick::Other, &versions, Some("9.9")), None);
    assert_eq!(resolve(LoaderPick::Stable, &[], None), None);
}

#[test]
fn the_default_name_follows_loader_and_version() {
    assert_eq!(default_name(Loader::Fabric, Some("26.3")), "Fabric 26.3");
    assert_eq!(default_name(Loader::Vanilla, Some("26.3")), "原版 26.3");
    assert_eq!(default_name(Loader::NeoForge, None), "新的NeoForge游戏");
}

#[test]
fn loader_choices_say_which_are_stable() {
    let choices = loader_choices(&[version("1.0", true), version("1.1-beta", false)]);
    assert_eq!(choices[0].tag, Some("稳定"));
    assert_eq!(choices[1].tag, Some("测试"));
    assert!(choices.iter().all(|choice| choice.primary));
}
