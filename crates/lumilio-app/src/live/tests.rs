use super::jobs::outcome;
use lumilio_ui::toast::Toast;

#[gpui_kit::test]
fn home_snapshot_and_rapid_selection_keep_the_last_game(cx: &mut gpui_kit::TestAppContext) {
    // The shipped backend wakes GPUI from its real tokio worker threads.
    cx.executor().allow_parking();
    use super::{Reload, State, Wiring, jobs::reload, library::choose_current};
    use crate::backend::Backend;
    use gpui_kit::AppContext as _;
    use lumilio_ui::{LauncherShell, home::HomePresentation};
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    let (a, b) = futures::executor::block_on(backend.spawn(async move {
        let a = service
            .create_instance("A", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        let b = service
            .create_instance("B", Some("1.0"), lumilio_core::Loader::Vanilla, None)
            .await
            .unwrap();
        service.set_current_instance(&b.id).await.unwrap();
        (a.id, b.id)
    }))
    .unwrap();
    cx.update(gpui_kit::init);
    let shell = cx.new(|cx| {
        LauncherShell::new(cx)
            .with_home(HomePresentation::Loading, cx)
            .with_live(Rc::new(|_, _, _| {}))
    });
    let wiring = Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    };
    cx.update(|cx| reload(&wiring, Reload::All, cx));
    let wait = |cx: &mut gpui_kit::TestAppContext, wanted: &str| {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            cx.run_until_parked();
            let ready = shell.read_with(cx, |shell, _| {
                matches!(shell.home(),
                HomePresentation::Continue { subject, .. } if subject.id.as_deref() == Some(wanted))
            });
            if ready {
                break;
            }
            assert!(Instant::now() < until, "Home did not show {wanted}");
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    wait(cx, &b);
    cx.update(|cx| {
        reload(&wiring, Reload::All, cx);
        choose_current(&wiring, b.clone(), cx);
        choose_current(&wiring, a.clone(), cx);
    });
    wait(cx, &a);
    let service = wiring.backend.service.clone();
    assert_eq!(
        futures::executor::block_on(
            wiring
                .backend
                .spawn(async move { service.current_instance().await })
        )
        .unwrap()
        .as_deref(),
        Some(a.as_str())
    );
    cx.run_until_parked();
    assert_eq!(
        wiring.state.borrow().continue_id.as_deref(),
        Some(a.as_str())
    );
    let latest = shell.read_with(cx, |shell, _| shell.live().unwrap().install_target.clone());
    assert_eq!(latest.as_deref(), Some(a.as_str()));
}

#[test]
fn a_failed_operation_says_it_plainly_and_keeps_the_cause_behind_details() {
    let failed = outcome(
        Err::<String, _>("os error 28: no space left"),
        |file| format!("已装好 {file}"),
        "没有装上 Sodium".to_owned(),
    );
    assert_eq!(
        failed,
        Toast::error("没有装上 Sodium").technical("os error 28: no space left")
    );
    assert!(
        !failed.text.contains("os error"),
        "no raw error as headline"
    );
    let done = outcome(
        Ok::<_, String>("sodium.jar"),
        |file| format!("已装好 {file}"),
        String::new(),
    );
    assert_eq!(done, Toast::success("已装好 sodium.jar"));
}

/// The CurseForge source through the shipped wiring, by clicks on the real
/// page: switch the source, search, install a mod into a game, install a
/// pack, and add a file the pack was waiting for. Talks to the real APIs and
/// needs a build with `CF_API_KEY`, so it is ignored by default. Run with
/// `cargo nextest run -p lumilio-app curseforge_through_the_page --run-ignored only --no-capture`.
#[gpui_kit::test]
#[ignore = "talks to the real CurseForge and Modrinth APIs"]
fn curseforge_through_the_page(cx: &mut gpui_kit::TestAppContext) {
    use super::dispatch::on_live_intent;
    use super::instance_write::write_instance;
    use super::{
        Reload, State, Wiring, discover::first_search, jobs::reload, library::choose_current,
    };
    use crate::backend::Backend;
    use gpui_kit::{AppContext as _, Modifiers, VisualTestContext};
    use lumilio_core::{Loader, ProjectKind};
    use lumilio_ui::LauncherShell;
    use lumilio_ui::instance_detail::{InstanceDetailView, InstanceIntent};
    use lumilio_ui::live::{DiscoverQuery, LiveIntent, SearchStatus};
    use lumilio_ui::route::Route;
    use sha1::{Digest, Sha1};
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    if !lumilio_plugin_curseforge::CurseForge::default().has_key() {
        println!("this build has no CurseForge key; nothing to check");
        return;
    }
    const CF: &str = lumilio_plugin_curseforge::ID;
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    let service = backend.service.clone();
    let game = futures::executor::block_on(backend.spawn(async move {
        let game = service
            .create_instance("Fabric", Some("1.20.1"), Loader::Fabric, Some("0.16.5"))
            .await
            .unwrap();
        service.set_current_instance(&game.id).await.unwrap();
        game
    }))
    .unwrap();
    cx.update(gpui_kit::init);
    let cell: Rc<RefCell<Option<Wiring>>> = Rc::default();
    let handler = {
        let cell = cell.clone();
        Rc::new(
            move |intent: LiveIntent, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                let wiring = cell.borrow().clone();
                if let Some(wiring) = wiring {
                    on_live_intent(&wiring, intent, window, cx);
                }
            },
        )
    };
    let (shell, cx) = cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(handler));
    let wiring = Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    };
    *cell.borrow_mut() = Some(wiring.clone());
    let service = wiring.backend.service.clone();
    let until = |cx: &mut VisualTestContext,
                 what: &str,
                 secs: u64,
                 done: &dyn Fn(&mut VisualTestContext) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let click = |cx: &mut VisualTestContext, selector: &'static str| {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is not on the page"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    let search_for = |cx: &mut VisualTestContext, words: &str| {
        let field = cx
            .debug_bounds("live-discover-search")
            .expect("the search box");
        cx.simulate_click(field.center(), Modifiers::none());
        cx.simulate_input(words);
        cx.simulate_keystrokes("enter");
    };
    let row_of = |cx: &mut VisualTestContext, project: &str| {
        shell.read_with(cx, |shell, _| {
            let model = shell.live().unwrap();
            (model.search != SearchStatus::Searching)
                .then(|| {
                    model
                        .results
                        .iter()
                        .position(|row| row.project_id == project)
                })
                .flatten()
        })
    };

    // The game to install into, and Discover's first search.
    cx.update(|_, cx| {
        reload(&wiring, Reload::All, cx);
        choose_current(&wiring, game.id.clone(), cx);
    });
    let id = game.id.clone();
    until(cx, "the install target", 10, &|cx| {
        shell.read_with(cx, |shell, _| {
            shell.live().unwrap().install_target.as_deref() == Some(id.as_str())
        })
    });
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| model.query = DiscoverQuery::new(ProjectKind::Mod),
            cx,
        );
    });
    cx.update(|_, cx| first_search(&wiring, cx));
    until(cx, "both sources and Modrinth's results", 30, &|cx| {
        shell.read_with(cx, |shell, _| {
            let model = shell.live().unwrap();
            model.sources.len() == 2 && matches!(model.search, SearchStatus::Done { .. })
        })
    });

    // Switch to CurseForge by its key.
    click(cx, "live-discover-source-key-1");
    until(cx, "CurseForge's results", 30, &|cx| {
        shell.read_with(cx, |shell, _| {
            let model = shell.live().unwrap();
            model.query.source == CF
                && matches!(model.search, SearchStatus::Done { .. })
                && !model.results.is_empty()
                && model.results.iter().all(|row| row.source == CF)
        })
    });

    // Search for JEI and install it into the Fabric game.
    search_for(cx, "jei");
    until(cx, "JEI in the results", 30, &|cx| {
        row_of(cx, "238222").is_some()
    });
    let at = row_of(cx, "238222").unwrap();
    click(cx, Box::leak(format!("live-install-{at}").into_boxed_str()));
    let mods = service.game_dir(&game.id).join("mods");
    until(cx, "JEI in the game's mods", 120, &|_| {
        std::fs::read_dir(&mods).is_ok_and(|entries| {
            entries
                .flatten()
                .any(|entry| entry.file_name().to_string_lossy().starts_with("jei-"))
        })
    });
    println!("installed JEI from CurseForge into {}", game.id);

    // The pack tab: install a small pack with a file only the website hands out.
    click(cx, "live-discover-tabs-tab-0");
    until(cx, "CurseForge's packs", 30, &|cx| {
        shell.read_with(cx, |shell, _| {
            let model = shell.live().unwrap();
            model.query.kind == ProjectKind::Modpack
                && matches!(model.search, SearchStatus::Done { .. })
        })
    });
    search_for(cx, "keo optimized");
    until(cx, "the pack in the results", 30, &|cx| {
        row_of(cx, "1459394").is_some()
    });
    let at = row_of(cx, "1459394").unwrap();
    click(cx, Box::leak(format!("live-install-{at}").into_boxed_str()));
    let pack = || {
        let service = service.clone();
        futures::executor::block_on(wiring.backend.spawn(async move {
            service
                .library()
                .await
                .instances
                .into_iter()
                .find(|record| {
                    record.source_project.as_deref() == Some("lumilio.curseforge/1459394")
                })
        }))
        .unwrap()
    };
    until(cx, "the pack in the library", 300, &|_| pack().is_some());
    let record = pack().unwrap();
    let pending = {
        let (service, id) = (service.clone(), record.id.clone());
        futures::executor::block_on(
            wiring
                .backend
                .spawn(async move { service.pending_downloads(&id).await }),
        )
        .unwrap()
        .unwrap()
    };
    println!(
        "installed {}; waiting for {:?}",
        record.name,
        pending
            .iter()
            .map(|file| &file.file_name)
            .collect::<Vec<_>>()
    );
    assert!(
        !pending.is_empty(),
        "the pack has a file only the website hands out"
    );

    // The person downloads it and adds it. The real file cannot be fetched
    // here (that is the point), so the list is pointed at a stand-in.
    let stand_in = dir.path().join("downloaded.zip");
    std::fs::write(&stand_in, b"the file from the website").unwrap();
    let digest: String = Sha1::digest(b"the file from the website")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let list = service
        .layout()
        .profile(&record.id)
        .join("pending-downloads.json");
    let mut waiting: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&list).unwrap()).unwrap();
    waiting[0]["sha1"] = serde_json::Value::String(digest);
    std::fs::write(&list, serde_json::to_vec(&waiting).unwrap()).unwrap();
    let target = service
        .game_dir(&record.id)
        .join(waiting[0]["path"].as_str().unwrap());
    let view = cx.update(|_, cx| {
        cx.new(|_| InstanceDetailView::new(record.id.clone(), Rc::new(|_, _, _| {})))
    });
    cx.update(|window, cx| {
        write_instance(
            &wiring,
            record.id.clone(),
            view.downgrade(),
            InstanceIntent::PendingFilesChosen(vec![stand_in.clone()]),
            window,
            cx,
        )
    });
    until(cx, "the added file in place", 30, &|_| target.is_file());
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"the file from the website"
    );
    println!("added the downloaded file at {}", target.display());
}

/// Choosing a source asks the application for the source's filters from
/// inside the shell's own update; reading the shell there at once panicked.
#[gpui_kit::test]
fn choosing_a_source_by_its_key_does_not_read_the_shell_mid_update(
    cx: &mut gpui_kit::TestAppContext,
) {
    use super::dispatch::on_live_intent;
    use super::{State, Wiring};
    use crate::backend::Backend;
    use gpui_kit::Modifiers;
    use lumilio_core::{ContentSourceInfo, ProjectKind};
    use lumilio_ui::LauncherShell;
    use lumilio_ui::live::{DiscoverQuery, LiveIntent};
    use lumilio_ui::route::Route;
    use std::{cell::RefCell, rc::Rc};

    // The shipped backend wakes GPUI from its real tokio worker threads.
    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    cx.update(gpui_kit::init);
    let cell: Rc<RefCell<Option<Wiring>>> = Rc::default();
    let handler = {
        let cell = cell.clone();
        Rc::new(
            move |intent: LiveIntent, window: &mut gpui_kit::Window, cx: &mut gpui_kit::App| {
                let wiring = cell.borrow().clone();
                if let Some(wiring) = wiring {
                    on_live_intent(&wiring, intent, window, cx);
                }
            },
        )
    };
    let (shell, cx) = cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(handler));
    *cell.borrow_mut() = Some(Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    });
    let source = |id: &str, name: &str| ContentSourceInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        catalog: None,
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Discover);
        shell.update_live(
            |model| {
                model.query = DiscoverQuery::new(ProjectKind::Mod);
                model.sources = vec![
                    source(lumilio_core::DEFAULT_SOURCE, "Modrinth"),
                    source(lumilio_plugin_curseforge::ID, "CurseForge"),
                ];
            },
            cx,
        );
    });
    cx.run_until_parked();
    let key = cx
        .debug_bounds("live-discover-source-key-1")
        .expect("the CurseForge key");
    cx.simulate_click(key.center(), Modifiers::none());
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.live().unwrap().query.source,
            lumilio_plugin_curseforge::ID
        );
    });
}

/// Filters that arrive after another source was chosen belong to the old
/// one: they must not take the page back there.
#[gpui_kit::test]
fn filters_for_a_source_left_behind_do_not_take_the_page_back(cx: &mut gpui_kit::TestAppContext) {
    use super::discover::run_load_filters;
    use super::{State, Wiring};
    use crate::backend::Backend;
    use lumilio_core::ProjectKind;
    use lumilio_ui::LauncherShell;
    use lumilio_ui::live::DiscoverQuery;
    use std::time::{Duration, Instant};
    use std::{cell::RefCell, rc::Rc};

    cx.executor().allow_parking();
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::open(dir.path().join("root")).unwrap();
    cx.update(gpui_kit::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    let wiring = Wiring {
        backend: Rc::new(backend),
        shell: shell.downgrade(),
        state: Rc::new(RefCell::new(State::default())),
    };
    let browse =
        |shell: &mut LauncherShell, source: &str, cx: &mut gpui_kit::Context<LauncherShell>| {
            shell.update_live(
                |model| {
                    model.query = DiscoverQuery::new(ProjectKind::Mod);
                    model.query.source = source.to_owned();
                },
                cx,
            );
        };
    // A source that is not on: once loaded, the page would move to the first
    // one that is. The person chooses another before that arrives.
    shell.update(cx, |shell, cx| browse(shell, "lumilio.gone", cx));
    cx.update(|_, cx| run_load_filters(&wiring, cx));
    shell.update(cx, |shell, cx| browse(shell, "lumilio.chosen", cx));
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.read_with(cx, |shell, _| shell.live().unwrap().sources.is_empty()) {
        assert!(Instant::now() < deadline, "the load never finished");
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(5));
    }
    cx.run_until_parked();
    shell.read_with(cx, |shell, _| {
        let model = shell.live().unwrap();
        assert_eq!(model.query.source, "lumilio.chosen");
        assert!(
            !model.filters.loaded,
            "the old source's filters were applied"
        );
    });
}
