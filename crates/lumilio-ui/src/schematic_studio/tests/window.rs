//! The studio window (T20, T30; D22, D27): the head over the viewport and
//! the materials column, on the file the pane handed over.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Entity, TestAppContext, px, size};

use super::{A, B, fixture, info_of};
use crate::schematic_studio::{Command, Connection, Event, StudioWindow};

#[gpui::test]
fn the_window_draws_the_workbench_and_opens_on_the_handed_file(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let models: Rc<RefCell<Vec<String>>> = Rc::default();
    let seen = models.clone();
    let (send, commands) = async_channel::unbounded();
    let (events, receive) = async_channel::unbounded();
    let slot: Rc<RefCell<Option<Entity<StudioWindow>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let content = cx.new(|cx| {
            StudioWindow::new(
                "lumilio.litematica".into(),
                "schematics".into(),
                Some(B.into()),
                Rc::new(move |file, _, _, _| seen.borrow_mut().push(file)),
                Rc::new(|_, _, _| {}),
                Connection { send, receive },
                cx,
            )
        });
        *keep.borrow_mut() = Some(content.clone());
        gpui_component::Root::new(content, window, cx)
    });
    cx.simulate_resize(size(px(720.), px(480.)));
    cx.run_until_parked();
    let sent: Vec<Command> = std::iter::from_fn(|| commands.try_recv().ok()).collect();
    assert_eq!(
        sent,
        [
            Command::List,
            Command::History,
            Command::Backups,
            Command::CanWrite,
            Command::Names,
            Command::Schemes
        ],
        "the window connects at once"
    );
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    events
        .try_send(Event::Listed(Ok(vec![A.into(), B.into()])))
        .unwrap();
    cx.run_until_parked();
    loop {
        let reads: Vec<String> = std::iter::from_fn(|| commands.try_recv().ok())
            .filter_map(|command| match command {
                Command::Read { file } => Some(file),
                _ => None,
            })
            .collect();
        if reads.is_empty() {
            break;
        }
        for file in reads {
            let bytes = files
                .iter()
                .find(|(path, _)| *path == file)
                .unwrap()
                .1
                .clone();
            let info = info_of(&bytes);
            events
                .try_send(Event::Read {
                    file,
                    result: Ok((bytes, info)),
                })
                .unwrap();
            cx.run_until_parked();
        }
    }
    let content = slot.borrow().clone().expect("window content");
    let studio = content.read_with(cx, |content, _| content.studio().clone());
    studio.read_with(cx, |studio, _| {
        assert_eq!(studio.selected(), Some(B), "opened on the handed file");
        assert!(studio.viewer.is_some(), "with its viewport");
    });
    assert_eq!(
        *models.borrow(),
        [B.to_owned()],
        "the viewport asked for B's assets"
    );

    let columns = cx.debug_bounds("studio-columns").expect("two columns");
    assert!(
        cx.debug_bounds("studio-tree").is_none(),
        "no tree column (D27)"
    );
    let viewport = cx.debug_bounds("studio-viewport").unwrap();
    let inspector = cx.debug_bounds("studio-inspector").unwrap();
    assert!(viewport.right() <= inspector.left());
    assert!(
        inspector.right() <= columns.right() && columns.right() <= px(720.),
        "no horizontal overflow at the minimum: {inspector:?} in {columns:?}"
    );
    assert!(
        viewport.size.width >= px(380.),
        "the picture takes most of the window: {viewport:?}"
    );
    let head = cx.debug_bounds("studio-head").unwrap();
    for part in ["studio-switcher", "studio-replace", "studio-more"] {
        let bounds = cx
            .debug_bounds(part)
            .unwrap_or_else(|| panic!("{part} is in the head"));
        assert!(
            bounds.right() <= head.right() && bounds.top() >= head.top(),
            "{part}: {bounds:?} in {head:?}"
        );
    }
    let tiles = cx.debug_bounds("studio-tiles").expect("the block tiles");
    assert!(
        tiles.size.width >= px(4. * 60. + 3. * 4.),
        "four tiles a row at the minimum: {tiles:?}"
    );
    for index in 0..3 {
        let tab = cx
            .debug_bounds(Box::leak(
                format!("studio-inspector-tab-{index}").into_boxed_str(),
            ))
            .expect("every inspector tab is drawn");
        assert!(
            tab.left() >= inspector.left() && tab.right() <= inspector.right(),
            "tab {index} stays inside the inspector: {tab:?}"
        );
    }
    assert!(
        cx.debug_bounds("studio-open-window").is_none(),
        "a studio in its window has no key to open one"
    );
    assert!(
        cx.debug_bounds("studio-more").is_some(),
        "operations live here"
    );
}

/// Block icons sit in a slot set off from the panel; at night it is
/// lighter than the panel, so black concrete or tinted glass shows against
/// it as it does on the game's inventory slots.
#[test]
fn block_slots_stand_out_from_the_panel_and_lighten_at_night() {
    use crate::schematic_studio::materials::slot_fill;
    use crate::theme::Body;
    let night = Body::of(true);
    assert!(slot_fill(night).l > night.panel.l + 0.05);
    let day = Body::of(false);
    assert!((slot_fill(day).l - day.panel.l).abs() > 0.05);
}
