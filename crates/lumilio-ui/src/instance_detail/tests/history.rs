use super::super::TAB_HISTORY;
use super::super::editors::Editor;
use super::super::intent::InstanceIntent;
use super::super::panels::Arrived;
use super::{click, record, rooted, world};
use gpui::TestAppContext;
use lumilio_core::LauncherSettings;
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn a_snapshot_is_taken_from_a_dialog_with_a_note_and_a_range(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_HISTORY, cx);
        view.history_sub = 2;
        view.arrived(Arrived::Snapshots(Ok(Vec::new())), cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
    });
    cx.run_until_parked();
    click(cx, "snapshot-create");
    view.read_with(cx, |view, _| {
        assert_eq!(view.editor, Some(Editor::Snapshot))
    });

    // A note and the second world.
    let note = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().snapshot_note.clone()
    });
    cx.update(|window, cx| {
        note.update(cx, |note, cx| {
            note.set_value("  before the farm  ", window, cx)
        })
    });
    view.update(cx, |view, cx| {
        view.snapshot_scope = 2;
        cx.notify();
    });
    click(cx, "instance-snapshot-go");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::CreateSnapshotAs {
            note: "before the farm".into(),
            world: Some("beta".into()),
        })
    );
}
