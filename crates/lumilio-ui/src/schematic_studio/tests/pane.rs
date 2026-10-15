//! The studio in the plugin pane (T20, D22): a summary instead of a viewport,
//! and the two ways into the window.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{A, B, click, fixture, info_of};
use crate::schematic_studio::{Command, Connection, Event, Placement, StudioView};

struct Pane {
    studio: Entity<StudioView>,
    /// What the pane asked the window to open on, in order.
    opened: Rc<RefCell<Vec<Option<String>>>>,
    events: async_channel::Sender<Event>,
    commands: async_channel::Receiver<Command>,
}

fn pane(cx: &mut TestAppContext) -> (Pane, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let opened: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let seen = opened.clone();
    let slot: Rc<RefCell<Option<Entity<StudioView>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let studio = cx.new(|cx| {
            StudioView::new(
                "lumilio.litematica".into(),
                "schematics".into(),
                Placement::Pane,
                Rc::new(|_, _, _| {}),
                Rc::new(|_, _, _, _| panic!("the pane never loads a model")),
                Rc::new(|_, _, _| {}),
                cx,
            )
            .on_open_window(Rc::new(move |file, _, _| seen.borrow_mut().push(file)))
        });
        *keep.borrow_mut() = Some(studio.clone());
        gpui_component::Root::new(studio, window, cx)
    });
    let studio = slot.borrow().clone().expect("studio");
    let (send, commands) = async_channel::unbounded();
    let (events, receive) = async_channel::unbounded();
    studio.update(cx, |studio, cx| {
        studio.connect(Connection { send, receive }, cx)
    });
    cx.run_until_parked();
    (
        Pane {
            studio,
            opened,
            events,
            commands,
        },
        cx,
    )
}

impl Pane {
    fn answer(&self, event: Event, cx: &mut VisualTestContext) {
        self.events.try_send(event).unwrap();
        cx.run_until_parked();
    }

    /// Answers every read from `files` until the studio asks for no more.
    fn serve(&self, files: &[(&str, Vec<u8>)], cx: &mut VisualTestContext) {
        loop {
            let reads: Vec<String> = std::iter::from_fn(|| self.commands.try_recv().ok())
                .filter_map(|command| match command {
                    Command::Read { file } => Some(file),
                    _ => None,
                })
                .collect();
            if reads.is_empty() {
                return;
            }
            for file in reads {
                let result = files
                    .iter()
                    .find(|(path, _)| *path == file)
                    .map(|(_, bytes)| (bytes.clone(), info_of(bytes)))
                    .ok_or_else(|| "missing".to_owned());
                self.answer(Event::Read { file, result }, cx);
            }
        }
    }
}

#[gpui::test]
fn the_catalogue_is_a_card_per_file_without_a_viewport(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    pane.serve(&files, cx);
    let a = cx
        .debug_bounds("studio-file-schematics/farms/a.litematic")
        .expect("a card for the file in a folder");
    let b = cx
        .debug_bounds("studio-file-schematics/b.litematic")
        .expect("a card for the file at the root");
    assert!(b.top() < a.top(), "the root's section comes first");
    pane.studio.read_with(cx, |studio, _| {
        assert_eq!(
            studio.summaries.get(A).map(|info| info.block_count),
            Some(4),
            "every card knows its size and blocks"
        );
    });

    click(cx, "studio-file-schematics/b.litematic");
    pane.serve(&files, cx);
    pane.studio.read_with(cx, |studio, _| {
        assert!(studio.viewer.is_none(), "no 3D viewport in the pane (D22)");
        assert_eq!(studio.selected(), Some(B));
    });
    assert!(
        cx.debug_bounds("studio-viewport").is_none()
            && cx.debug_bounds("studio-inspector").is_none()
            && cx.debug_bounds("studio-more").is_none()
            && cx.debug_bounds("studio-tree").is_none()
    );

    // The search keeps the matching cards only.
    let search = pane
        .studio
        .read_with(cx, |studio, _| studio.search.clone().unwrap());
    cx.update(|window, cx| search.update(cx, |input, cx| input.set_value("farms", window, cx)));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("studio-file-schematics/farms/a.litematic")
            .is_some()
    );
    assert!(
        cx.debug_bounds("studio-file-schematics/b.litematic")
            .is_none()
    );
}

/// T28: once a file's blocks are known its thumbnail is asked for, one at a
/// time, keyed by the content fingerprint; a file without one is not asked
/// again, and new blocks ask again.
#[gpui::test]
fn thumbnails_are_asked_one_at_a_time_by_content(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    let mut asked = Vec::new();
    loop {
        let commands: Vec<Command> = std::iter::from_fn(|| pane.commands.try_recv().ok()).collect();
        if commands.is_empty() {
            break;
        }
        for command in commands {
            match command {
                Command::Read { file } => {
                    let bytes = files
                        .iter()
                        .find(|(path, _)| *path == file)
                        .unwrap()
                        .1
                        .clone();
                    let info = info_of(&bytes);
                    pane.answer(
                        Event::Read {
                            file,
                            result: Ok((bytes, info)),
                        },
                        cx,
                    );
                }
                Command::Thumbnail { file, key } => asked.push((file, key)),
                _ => {}
            }
        }
    }
    assert_eq!(asked.len(), 1, "one at a time: {asked:?}");
    let (first, key) = asked[0].clone();
    let print = pane
        .studio
        .read_with(cx, |studio, _| studio.tree.fingerprint(&first).unwrap());
    assert_eq!(key, print.to_string());

    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        3,
        image::Rgba([9, 9, 9, 255]),
    ))
    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
    .unwrap();
    pane.answer(
        Event::Thumbnail {
            file: first.clone(),
            result: Ok(png),
        },
        cx,
    );
    let next: Vec<(String, String)> = std::iter::from_fn(|| pane.commands.try_recv().ok())
        .filter_map(|command| match command {
            Command::Thumbnail { file, key } => Some((file, key)),
            _ => None,
        })
        .collect();
    assert_eq!(next.len(), 1, "the next file's turn");
    assert_ne!(next[0].0, first);
    pane.answer(
        Event::Thumbnail {
            file: next[0].0.clone(),
            result: Err("no block assets".into()),
        },
        cx,
    );
    pane.studio.read_with(cx, |studio, _| {
        assert!(studio.thumbnails.contains_key(&first));
        assert!(studio.thumbnails.len() == 1 && studio.thumb_failed.contains(&next[0].0));
    });
    assert!(
        std::iter::from_fn(|| pane.commands.try_recv().ok())
            .all(|command| !matches!(command, Command::Thumbnail { .. })),
        "a file without a thumbnail is not asked again"
    );
}

#[gpui::test]
fn the_key_and_a_double_click_open_the_window_on_the_chosen_file(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    click(cx, "studio-open-window");
    assert_eq!(*pane.opened.borrow(), [None], "nothing chosen yet");

    click(cx, "studio-file-schematics/b.litematic");
    click(cx, "studio-open-window");
    assert_eq!(pane.opened.borrow().last().unwrap().as_deref(), Some(B));

    let row = cx
        .debug_bounds("studio-file-schematics/farms/a.litematic")
        .expect("the row is drawn");
    cx.simulate_event(gpui::MouseDownEvent {
        button: gpui::MouseButton::Left,
        position: row.center(),
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(gpui::MouseUpEvent {
        button: gpui::MouseButton::Left,
        position: row.center(),
        modifiers: gpui::Modifiers::none(),
        click_count: 2,
    });
    cx.run_until_parked();
    assert_eq!(
        pane.opened.borrow().last().unwrap().as_deref(),
        Some(A),
        "a double click chooses the file and opens the window on it"
    );
    pane.studio
        .read_with(cx, |studio, _| assert_eq!(studio.selected(), Some(A)));
}

#[gpui::test]
fn a_file_handed_over_before_the_listing_is_chosen_when_it_arrives(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    pane.studio
        .update(cx, |studio, cx| studio.select_file(B.into(), cx));
    pane.studio
        .read_with(cx, |studio, _| assert_eq!(studio.selected(), None));
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    pane.studio
        .read_with(cx, |studio, _| assert_eq!(studio.selected(), Some(B)));
    // A file the folder does not have is left alone.
    pane.studio.update(cx, |studio, cx| {
        studio.select_file("schematics/x.litematic".into(), cx)
    });
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    pane.studio
        .read_with(cx, |studio, _| assert_eq!(studio.selected(), Some(B)));
}

/// Answers the file reads asked for and returns the thumbnails asked for.
fn thumbnails_asked(
    pane: &Pane,
    files: &[(&str, Vec<u8>)],
    cx: &mut VisualTestContext,
) -> Vec<String> {
    let mut asked = Vec::new();
    loop {
        let commands: Vec<Command> = std::iter::from_fn(|| pane.commands.try_recv().ok()).collect();
        if commands.is_empty() {
            return asked;
        }
        for command in commands {
            match command {
                Command::Read { file } => {
                    let bytes = files
                        .iter()
                        .find(|(path, _)| *path == file)
                        .unwrap()
                        .1
                        .clone();
                    let info = info_of(&bytes);
                    pane.answer(
                        Event::Read {
                            file,
                            result: Ok((bytes, info)),
                        },
                        cx,
                    );
                }
                Command::Thumbnail { file, .. } => asked.push(file),
                _ => {}
            }
        }
    }
}

fn a_png() -> Vec<u8> {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        3,
        image::Rgba([9, 9, 9, 255]),
    ))
    .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
    .unwrap();
    png
}

/// T21: thumbnails are kept per look of the sources switched on, so a kept
/// switch asks for every thumbnail shown again; the one being drawn as the
/// switch flips was drawn with the old sources, so its answer is dropped
/// and it is asked for again too.
#[gpui::test]
fn a_kept_switch_asks_for_the_thumbnails_again(cx: &mut TestAppContext) {
    let (pane, cx) = pane(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    pane.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    let first = thumbnails_asked(&pane, &files, cx);
    assert_eq!(first.len(), 1);
    pane.answer(
        Event::Thumbnail {
            file: first[0].clone(),
            result: Ok(a_png()),
        },
        cx,
    );
    let second = thumbnails_asked(&pane, &files, cx);
    assert_eq!(second.len(), 1, "the other file is being drawn");
    assert_ne!(second[0], first[0]);

    pane.answer(
        Event::SourceSwitched {
            path: "/game/mods/create.jar".into(),
            on: true,
            result: Ok(()),
        },
        cx,
    );
    pane.studio.read_with(cx, |studio, _| {
        assert!(studio.thumbnails.is_empty(), "drawn with the old sources");
    });
    assert_eq!(
        thumbnails_asked(&pane, &files, cx),
        Vec::<String>::new(),
        "one at a time"
    );
    pane.answer(
        Event::Thumbnail {
            file: second[0].clone(),
            result: Ok(a_png()),
        },
        cx,
    );
    pane.studio.read_with(cx, |studio, _| {
        assert!(studio.thumbnails.is_empty(), "the stale answer is dropped");
    });
    assert_eq!(thumbnails_asked(&pane, &files, cx), [first[0].clone()]);
    pane.answer(
        Event::Thumbnail {
            file: first[0].clone(),
            result: Ok(a_png()),
        },
        cx,
    );
    assert_eq!(
        thumbnails_asked(&pane, &files, cx),
        [second[0].clone()],
        "and the stale one again"
    );
}
