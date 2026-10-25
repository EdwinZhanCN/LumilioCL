//! The Library's actions with a real launcher behind them, on its own
//! runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::Loader;

use super::super::{Library, LibraryEvent};
use crate::live::CollectionRow;
use crate::testing::{Launcher, launcher, toasts, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// The Library in a window, with what it told the application so far.
fn library<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<Library>,
    Rc<RefCell<Vec<LibraryEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let library = cx.new(|_| Library::new(Some(worker)));
    let events: Rc<RefCell<Vec<LibraryEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&library, move |_, event: &LibraryEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (library, events, cx)
}

#[gpui::test]
fn a_star_is_saved_quietly_and_the_lists_are_read_again(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let service = launcher.worker.service().clone();
    let game = launcher
        .runtime
        .block_on(service.create_instance("A", Some("1.21.1"), Loader::Vanilla, None))
        .unwrap();
    let (library, events, cx) = library(&launcher, cx);
    library.update(cx, |library, cx| {
        library.toggle_favorite(game.id.clone(), cx)
    });
    until(cx, "the star to be saved", |_| !events.borrow().is_empty());

    assert_eq!(events.borrow().as_slice(), [LibraryEvent::ListsChanged]);
    let saved = launcher
        .runtime
        .block_on(service.instance(&game.id))
        .unwrap();
    assert!(saved.favorite);
    assert!(toasts(cx).is_empty(), "the star shows by itself");
}

#[gpui::test]
fn a_deleted_collection_is_told_and_its_games_stay(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(service.create_collection("生存"))
        .unwrap();
    let (library, events, cx) = library(&launcher, cx);
    library.update(cx, |library, cx| {
        library.delete_collection("生存".to_owned(), cx)
    });
    until(cx, "the collection to go", |_| !events.borrow().is_empty());

    let collections = launcher.runtime.block_on(service.library()).collections;
    assert!(collections.is_empty());
    assert_eq!(toasts(cx).len(), 1, "a toast says it is gone");
}

#[gpui::test]
fn the_page_order_is_handed_on_to_be_remembered(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (library, events, cx) = library(&launcher, cx);
    library.update(cx, |library, cx| library.remember_view(1, 2, cx));
    assert_eq!(
        events.borrow().as_slice(),
        [LibraryEvent::Remember { sort: 1, loader: 2 }]
    );
}

#[test]
fn the_picker_marks_the_collections_a_game_is_in() {
    let mut library = Library::new(None);
    library.set_collections(vec![
        CollectionRow {
            name: "生存".into(),
            members: vec!["c".into(), "a".into()],
        },
        CollectionRow {
            name: "空".into(),
            members: Vec::new(),
        },
    ]);
    assert_eq!(
        library.memberships_of("a"),
        [("生存".to_owned(), true), ("空".to_owned(), false)]
    );
    assert_eq!(
        library.memberships_of("b"),
        [("生存".to_owned(), false), ("空".to_owned(), false)]
    );
    assert_eq!(library.collection_names(), ["生存", "空"]);
}
