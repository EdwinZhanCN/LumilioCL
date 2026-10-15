mod history;
mod ops;
mod pane;
mod sources;
mod tree;
mod viewport;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Entity, TestAppContext, VisualTestContext, WeakEntity};
use lumilio_schematic::{BlockState, Region, UniversalSchematic};

use super::{
    Command, Connection, Event, Inspection, InspectorTab, Placement, StudioAction, StudioView,
};
use crate::model_view::Viewer;

const A: &str = "schematics/farms/a.litematic";
const B: &str = "schematics/b.litematic";
const C: &str = "schematics/c.litematic";

/// A litematic with `blocks` stone blocks in a row and one oak stair.
pub(super) fn fixture(name: &str, blocks: i32) -> Vec<u8> {
    let mut schematic = UniversalSchematic::new(name.to_owned());
    schematic.metadata.author = Some("Builder".to_owned());
    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (blocks + 1, 1, 1));
    let stone = BlockState::from_block_string("minecraft:stone").unwrap();
    for x in 0..blocks {
        region.set_block(x, 0, 0, &stone);
    }
    region.set_block(
        blocks,
        0,
        0,
        &BlockState::from_block_string("minecraft:oak_stairs[facing=north]").unwrap(),
    );
    schematic.add_region(region);
    lumilio_schematic::to_litematic_bytes(&schematic).unwrap()
}

/// What the application reports a file looked like: unique per content.
pub(super) fn info_of(bytes: &[u8]) -> lumilio_plugin_api::FileInfo {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hash);
    lumilio_plugin_api::FileInfo {
        len: bytes.len() as u64,
        modified_ms: 0,
        sha256: format!("{:016x}", hash.finish()),
    }
}

pub(super) struct Rig {
    pub studio: Entity<StudioView>,
    pub commands: async_channel::Receiver<Command>,
    pub events: async_channel::Sender<Event>,
    pub actions: Rc<RefCell<Vec<StudioAction>>>,
    pub models: Rc<RefCell<Vec<(String, u64)>>>,
}

/// A studio in its own window, connected to channels the test answers.
pub(super) fn rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let actions: Rc<RefCell<Vec<StudioAction>>> = Rc::default();
    let models: Rc<RefCell<Vec<(String, u64)>>> = Rc::default();
    let (seen_actions, seen_models) = (actions.clone(), models.clone());
    let slot: Rc<RefCell<Option<Entity<StudioView>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let studio = cx.new(|cx| {
            StudioView::new(
                "lumilio.litematica".into(),
                "schematics".into(),
                Placement::Window,
                Rc::new(|_, _, _| {}),
                Rc::new(move |file, request, _, _| seen_models.borrow_mut().push((file, request))),
                Rc::new(move |action, _, _| seen_actions.borrow_mut().push(action)),
                cx,
            )
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
        Rig {
            studio,
            commands,
            events,
            actions,
            models,
        },
        cx,
    )
}

impl Rig {
    pub fn sent(&self) -> Vec<Command> {
        std::iter::from_fn(|| self.commands.try_recv().ok()).collect()
    }

    pub fn answer(&self, event: Event, cx: &mut VisualTestContext) {
        self.events.try_send(event).unwrap();
        cx.run_until_parked();
    }

    /// Answers every read the studio asked for from `files`, until it asks
    /// for nothing more; returns the other commands it sent meanwhile.
    pub fn serve(&self, files: &[(&str, Vec<u8>)], cx: &mut VisualTestContext) -> Vec<Command> {
        let mut others = Vec::new();
        loop {
            let mut reads = Vec::new();
            for command in self.sent() {
                match command {
                    Command::Read { file } => reads.push(file),
                    other => others.push(other),
                }
            }
            if reads.is_empty() {
                return others;
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

    pub fn viewer(&self, cx: &mut VisualTestContext) -> Option<WeakEntity<Viewer>> {
        self.studio.read_with(cx, |studio, _| {
            studio.viewer.as_ref().map(Entity::downgrade)
        })
    }
}

pub(super) fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = cx
        .debug_bounds(Box::leak(selector.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{selector} is drawn"));
    cx.simulate_click(bounds.center(), gpui::Modifiers::none());
    cx.run_until_parked();
}

/// Opens `file` from the head's switcher, as a person does in the window.
pub(super) fn choose(cx: &mut VisualTestContext, file: &str) {
    click(cx, "studio-switcher");
    click(cx, &format!("studio-file-{file}"));
}

#[gpui::test]
fn connecting_lists_the_folder_and_asks_for_history_write_access_and_names(
    cx: &mut TestAppContext,
) {
    let (rig, _cx) = rig(cx);
    assert_eq!(
        rig.sent(),
        [
            Command::List,
            Command::History,
            Command::Backups,
            Command::CanWrite,
            Command::Names,
            Command::Schemes
        ]
    );
}

#[gpui::test]
fn choosing_a_file_fills_the_materials_and_switching_releases_the_old_viewport(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = rig(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    rig.sent();
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    rig.serve(&files, cx);
    assert!(
        cx.debug_bounds("studio-tree").is_none(),
        "the window has no tree column: its switcher lists the files (D27)"
    );

    choose(cx, B);
    rig.serve(&files, cx);
    let first = rig.viewer(cx).expect("a viewport for the chosen file");
    assert_eq!(
        rig.models.borrow().last().map(|(file, _)| file.as_str()),
        Some(B)
    );
    rig.studio.read_with(cx, |studio, _| {
        assert!(matches!(studio.inspection, Inspection::Ready(_)));
        assert_eq!(studio.tab, InspectorTab::Materials, "materials first");
    });
    assert!(
        cx.debug_bounds("studio-tile-minecraft:stone").is_some(),
        "a tile per block kind"
    );
    assert!(
        cx.debug_bounds("studio-tile-minecraft:oak_stairs")
            .is_some()
    );
    assert!(cx.debug_bounds("studio-materials-summary").is_some());
    assert!(
        cx.debug_bounds("studio-detail").is_none(),
        "nothing chosen yet"
    );

    choose(cx, A);
    rig.serve(&files, cx);
    assert!(
        first.upgrade().is_none(),
        "the old viewport, its worker and frames are gone"
    );
    let second = rig.viewer(cx).expect("a new viewport");
    assert_ne!(second.entity_id(), first.entity_id());
    assert_eq!(
        rig.models.borrow().last().map(|(file, _)| file.as_str()),
        Some(A)
    );
}

#[gpui::test]
fn late_assets_for_a_replaced_viewport_are_dropped(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    let files = [(A, fixture("A", 1)), (B, fixture("B", 1))];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    choose(cx, B);
    let old = rig.models.borrow().last().unwrap().1;
    choose(cx, A);
    rig.serve(&files, cx);
    let new = rig.models.borrow().last().unwrap().1;
    rig.studio.update(cx, |studio, cx| {
        assert!(!studio.model_arrived(old, Err("late".into()), cx));
        assert_eq!(studio.viewport_request(), Some(new));
    });
}

#[gpui::test]
fn files_with_the_same_blocks_share_a_marker(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    // Same blocks, different names (metadata): duplicates. C differs by a block.
    let files = [
        (A, fixture("A", 2)),
        (B, fixture("Other", 2)),
        (C, fixture("C", 3)),
    ];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into(), C.into()])), cx);
    rig.serve(&files, cx);
    rig.studio.update(cx, |studio, cx| {
        assert_eq!(
            studio.tree.duplicates(),
            vec![vec![B.to_owned(), A.to_owned()]]
        );
        studio.tab = InspectorTab::Info;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("studio-duplicate-row-schematics/b.litematic")
            .is_some()
    );
    assert!(
        cx.debug_bounds("studio-duplicate-row-schematics/c.litematic")
            .is_none()
    );
}

#[gpui::test]
fn the_menu_hands_csv_and_reveal_to_the_page(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.answer(Event::Listed(Ok(vec![B.into()])), cx);
    choose(cx, B);
    let menu = rig.studio.read_with(cx, |studio, _| studio.menu());
    cx.update(|window, cx| {
        for entry in &menu {
            if !entry.is_disabled() {
                entry.choose(window, cx);
            }
        }
    });
    assert_eq!(
        *rig.actions.borrow(),
        [
            StudioAction::ExportCsv(B.into()),
            StudioAction::Reveal(B.into())
        ],
        "only CSV and reveal are built; the rest say why they are not"
    );
    assert!(menu.last().unwrap().is_danger(), "删除… is last, in danger");
}

#[gpui::test]
fn restore_asks_first_and_is_blocked_while_the_game_runs(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.sent();
    let entry = lumilio_core::SchematicLogEntry {
        index: 1,
        time: 0,
        operation: "replace".into(),
        file: B.into(),
        backup: Some("backups/1.litematic".into()),
        summary: "stone → glass".into(),
        restores: None,
    };
    rig.answer(Event::History(Ok(vec![entry])), cx);
    rig.answer(Event::CanWrite(false), cx);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::History;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "studio-restore-1");
    assert!(
        !cx.update(gpui_component::WindowExt::has_active_dialog),
        "a running game keeps restore shut"
    );
    rig.answer(Event::CanWrite(true), cx);
    click(cx, "studio-restore-1");
    assert!(cx.update(gpui_component::WindowExt::has_active_dialog));
    assert!(
        rig.sent().is_empty(),
        "nothing is written before the person confirms"
    );
}

#[gpui::test]
fn an_empty_history_says_so_calmly(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.answer(Event::History(Ok(Vec::new())), cx);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::History;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-history-empty").is_some());
}

#[gpui::test]
fn a_game_start_shuts_writes_until_the_application_answers(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    rig.answer(Event::CanWrite(true), cx);
    rig.sent();
    rig.studio
        .update(cx, |studio, cx| studio.write_access_changed(cx));
    rig.studio
        .read_with(cx, |studio, _| assert!(!studio.can_write));
    assert_eq!(rig.sent(), [Command::CanWrite]);
}

/// Hands an open replace dialog a block table of `ids`, as the
/// application's answer would, and draws it (T24).
fn give_catalog(
    dialog: &gpui::Entity<crate::schematic_studio::dialogs::ReplaceDialog>,
    ids: &[&str],
    cx: &mut VisualTestContext,
) {
    let mut catalog = lumilio_core::BlockCatalog::default();
    for id in ids {
        catalog.add_block(id);
    }
    dialog.update(cx, |dialog, cx| dialog.set_catalog(Ok(catalog), cx));
    cx.run_until_parked();
}
