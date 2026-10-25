//! Starting a game with a real launcher behind it, on its own runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::{LaunchSignal, Loader};

use super::{Launch, LaunchEvent, Readiness};
use crate::live::PlaceTarget;
use crate::testing::{Launcher, launcher, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// The launch feature in a window, with what it told so far.
fn launch<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<Launch>,
    Rc<RefCell<Vec<LaunchEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let launch = cx.new(|_| Launch::new(Some(worker)));
    let events: Rc<RefCell<Vec<LaunchEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&launch, move |_, event: &LaunchEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (launch, events, cx)
}

/// A current game of a version that does not exist: launching it fails
/// while preparing.
fn current_game(launcher: &Launcher, account: bool) -> String {
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(async move {
            let game = service
                .create_instance("A", Some("lumilio-test-none-a"), Loader::Vanilla, None)
                .await?;
            if account {
                service.add_account("Steve", None).await?;
            }
            service.set_current_instance(&game.id).await?;
            Ok::<_, lumilio_core::ServiceError>(game.id)
        })
        .unwrap()
}

fn readiness(launch: &Entity<Launch>, cx: &mut VisualTestContext) -> Readiness {
    let answer: Rc<RefCell<Option<Readiness>>> = Rc::default();
    let sink = answer.clone();
    cx.update(|_, cx| {
        let task = launch.update(cx, |launch, cx| launch.readiness(cx));
        cx.spawn(async move |_| *sink.borrow_mut() = Some(task.await))
            .detach();
    });
    until(cx, "the readiness check", |_| answer.borrow().is_some());
    answer.take().unwrap()
}

#[gpui::test]
fn a_game_asks_for_its_folder_then_an_account_before_it_starts(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let id = current_game(&launcher, false);
    let (launch, _, cx) = launch(&launcher, cx);
    assert_eq!(readiness(&launch, cx), Readiness::NoAccount);

    let folder = launcher.worker.service().game_dir(&id);
    std::fs::remove_dir_all(&folder).unwrap();
    assert!(
        matches!(readiness(&launch, cx), Readiness::MissingFolder { id: missing, .. } if missing == id),
        "the folder comes before the account"
    );

    std::fs::create_dir_all(&folder).unwrap();
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(async move { service.add_account("Steve", None).await })
        .unwrap();
    assert_eq!(readiness(&launch, cx), Readiness::Ready);
}

#[gpui::test]
fn a_launch_that_fails_says_so_ends_and_goes_nowhere_next_time(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    current_game(&launcher, true);
    let (launch, events, cx) = launch(&launcher, cx);
    cx.update(|window, cx| {
        let shown_in = window.window_handle();
        launch.update(cx, |launch, cx| {
            launch.aim(PlaceTarget::World("New World".into()));
            launch.start(shown_in, cx);
            assert!(launch.launching());
        });
    });
    until(cx, "the launch to end", |_| {
        events.borrow().last() == Some(&LaunchEvent::Ended)
    });

    let events = events.borrow();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, LaunchEvent::Signal(LaunchSignal::Failed(_)))),
        "{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, LaunchEvent::Output { running: false, .. })),
        "the page learns the game is no longer on its way"
    );
    launch.update(cx, |launch, _| {
        assert!(!launch.launching(), "nothing is left to stop");
        assert_eq!(launch.take_target(), None, "the world was used once");
    });
}
