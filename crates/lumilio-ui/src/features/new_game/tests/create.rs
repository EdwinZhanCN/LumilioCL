//! Making games with a real launcher behind it, on its own runtime.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::{DefaultTransport, LauncherService, Loader};

use super::super::form::{NewGameForm, NewGameRequest};
use super::super::{NewGame, NewGameEvent};
use crate::runtime::Worker;

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// A launcher over an empty folder. The runtime and the folder live as long
/// as the test.
struct Launcher {
    runtime: tokio::runtime::Runtime,
    worker: Worker,
    _dir: tempfile::TempDir,
}

fn launcher() -> Launcher {
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let service = {
        let _guard = runtime.enter();
        LauncherService::open(
            dir.path().join("root"),
            DefaultTransport::new().unwrap(),
            Vec::new(),
        )
        .unwrap()
    };
    let worker = Worker::new(Arc::new(service), runtime.handle().clone());
    Launcher {
        runtime,
        worker,
        _dir: dir,
    }
}

/// The feature in a window, with what it told the application so far.
fn new_game<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<NewGame>,
    Rc<RefCell<Vec<NewGameEvent>>>,
    &'a mut VisualTestContext,
) {
    // The runtime's threads wake GPUI.
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let feature = cx.new(|_| NewGame::new(Some(worker)));
    let events: Rc<RefCell<Vec<NewGameEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&feature, move |_, event: &NewGameEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (feature, events, cx)
}

/// Lets the runtime and GPUI work until `done` holds, for at most ten seconds.
fn until(cx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn texts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        crate::toast::posted(cx)
            .iter()
            .map(|toast| toast.text.clone())
            .collect()
    })
}

#[gpui::test]
fn a_made_game_is_handed_to_the_application_after_the_lists_change(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (feature, events, cx) = new_game(&launcher, cx);
    let form =
        cx.update(|window, cx| cx.new(|cx| NewGameForm::new(Rc::new(|_, _, _| {}), window, cx)));
    let window = cx.window_handle();
    feature.update(cx, |feature, cx| {
        feature.create(
            form.downgrade(),
            window,
            NewGameRequest {
                name: "Plain".to_owned(),
                loader: Loader::Vanilla,
                game_version: "1.21.1".to_owned(),
                loader_version: None,
                install: false,
            },
            cx,
        );
    });
    until(cx, "the game to be made", |_| {
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, NewGameEvent::Created { .. }))
    });

    let service = launcher.worker.service().clone();
    let library = launcher.runtime.block_on(service.library());
    let [record] = library.instances.as_slice() else {
        panic!("one game in the library, not {:?}", library.instances);
    };
    assert_eq!(record.name, "Plain");
    let events = events.borrow();
    assert!(
        matches!(events.as_slice(), [
            NewGameEvent::TasksChanged,
            NewGameEvent::Created { id, window: made_in },
        ] if *id == record.id && *made_in == window),
        "the lists are read again before the game is shown"
    );
    assert!(
        texts(cx).iter().any(|text| text.contains("Plain")),
        "a toast names the new game"
    );
}

#[gpui::test]
fn dropped_files_that_are_not_packs_start_nothing(cx: &mut TestAppContext) {
    let launcher = launcher();
    let (feature, events, cx) = new_game(&launcher, cx);
    feature.update(cx, |feature, cx| {
        feature.import_files(vec![PathBuf::from("notes.txt")], cx);
    });
    cx.run_until_parked();
    assert_eq!(texts(cx).len(), 1, "said once that it cannot be used");
    assert!(events.borrow().is_empty(), "no task, no game");
}
