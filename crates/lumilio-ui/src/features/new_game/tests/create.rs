//! Making games with a real launcher behind it, on its own runtime.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::{CancellationToken, ExportSpec, Loader, PackFormat};

use super::super::form::{NewGameForm, NewGameRequest};
use super::super::{NewGame, NewGameEvent};
use crate::testing::{Launcher, launcher, toasts, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
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

#[gpui::test]
fn a_made_game_is_handed_to_the_application_after_the_lists_change(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
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
        toasts(cx).iter().any(|text| text.contains("Plain")),
        "a toast names the new game"
    );
}

#[gpui::test]
fn dropped_files_that_are_not_packs_start_nothing(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (feature, events, cx) = new_game(&launcher, cx);
    let window = cx.window_handle();
    feature.update(cx, |feature, cx| {
        feature.import_files(vec![PathBuf::from("notes.txt")], window, cx);
    });
    cx.run_until_parked();
    assert_eq!(toasts(cx).len(), 1, "said once that it cannot be used");
    assert!(events.borrow().is_empty(), "no task, no game");
}

#[gpui::test]
fn a_dropped_pack_is_shown_like_a_made_game(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let service = launcher.worker.service().clone();
    let source = launcher
        .runtime
        .block_on(service.create_instance("Source", Some("1.21.1"), Loader::Vanilla, None))
        .unwrap();
    let folder = service.layout().game(&source.id);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("options.txt"), "fov:0.0\n").unwrap();
    // Beside the game's folder, inside the launcher's temporary home.
    let pack = folder.parent().unwrap().join("shared.zip");
    launcher
        .runtime
        .block_on(service.export_modpack(
            &source.id,
            ExportSpec {
                format: PackFormat::Prism,
                name: "Shared".to_owned(),
                version: "1".to_owned(),
                summary: None,
                include: vec!["options.txt".to_owned()],
            },
            &pack,
            CancellationToken::new(),
        ))
        .unwrap();
    let (feature, events, cx) = new_game(&launcher, cx);
    let window = cx.window_handle();

    feature.update(cx, |feature, cx| {
        feature.import_files(vec![pack], window, cx)
    });
    until(cx, "the pack to be imported", |_| {
        events
            .borrow()
            .iter()
            .any(|event| matches!(event, NewGameEvent::Created { .. }))
    });

    let library = launcher.runtime.block_on(service.library());
    let imported = library
        .instances
        .iter()
        .find(|record| record.id != source.id)
        .expect("the pack became a second game");
    assert!(
        events.borrow().iter().any(|event| matches!(
            event,
            NewGameEvent::Created { id, window: shown_in }
                if *id == imported.id && *shown_in == window
        )),
        "the imported game is handed over to be shown"
    );
}
