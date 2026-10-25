//! A game's page with a real launcher behind it, on its own runtime, opened
//! and closed through the shell as the window does.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};
use lumilio_core::Loader;

use super::GameEvent;
use crate::LauncherShell;
use crate::instance_detail::InstanceIntent;
use crate::testing::{Launcher, launcher, toasts, until};

/// The shell in a window, with what the game feature told the application.
fn shell<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<LauncherShell>,
    Rc<RefCell<Vec<GameEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let worker = launcher.worker.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| LauncherShell::new(cx).with_worker(worker, cx));
    let events: Rc<RefCell<Vec<GameEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        let game = shell.read(cx).game().clone();
        cx.subscribe(&game, move |_, event: &GameEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (shell, events, cx)
}

fn create(launcher: &Launcher, name: &str) -> String {
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(service.create_instance(name, Some("1.21.1"), Loader::Vanilla, None))
        .unwrap()
        .id
}

#[gpui::test]
fn an_opened_page_reads_its_game_and_tells_the_application(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let id = create(&launcher, "生存");
    let (shell, events, cx) = shell(&launcher, cx);
    let view = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open_game(id.clone(), None, window, cx)
        })
    });
    until(cx, "the page to read the game", |cx| {
        view.read_with(cx, |view, _| view.title() == "生存")
    });
    assert!(
        matches!(events.borrow().first(), Some(GameEvent::Opened(opened)) if *opened == id),
        "{:?}",
        events.borrow()
    );
}

#[gpui::test]
fn a_game_deleted_from_its_page_leaves_the_window_and_the_lists(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let id = create(&launcher, "生存");
    let (shell, events, cx) = shell(&launcher, cx);
    let view = cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.open_game(id.clone(), None, window, cx)
        })
    });
    cx.update(|window, cx| {
        let work = shell.read(cx).game().read(cx).work().expect("a worker");
        work.answer(&id, view.downgrade(), InstanceIntent::Delete, window, cx);
    });
    until(cx, "the deletion to be told", |cx| !toasts(cx).is_empty());

    let gone = |event: &GameEvent| matches!(event, GameEvent::Deleted(deleted) if *deleted == id);
    assert!(events.borrow().iter().any(gone), "{:?}", events.borrow());
    assert!(
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, GameEvent::ListsChanged)),
        "the lists are read again"
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.live_instance().is_none()),
        "the window leaves the deleted game's page"
    );
}
