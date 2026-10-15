//! 历史 (T12, T40): the log and the backups folder in one panel. A line
//! says whether its copy is kept and how large; a copy no line names is
//! listed below and restores too; the panel shows every file's history or
//! the open file's.

use gpui::{TestAppContext, VisualTestContext};
use lumilio_core::{KeptBackup, SchematicLogEntry};

use super::{A, B, Rig, choose, click, fixture, rig};
use crate::schematic_studio::history::{loose, size_words};
use crate::schematic_studio::{Command, Event, InspectorTab};

fn line(index: usize, file: &str, backup: Option<&str>) -> SchematicLogEntry {
    SchematicLogEntry {
        index,
        time: 0,
        operation: "replace".into(),
        file: file.into(),
        backup: backup.map(str::to_owned),
        summary: String::new(),
        restores: None,
    }
}

fn copy(path: &str, file: &str, size: u64) -> KeptBackup {
    KeptBackup {
        path: path.into(),
        file: file.into(),
        size,
        time: 0,
    }
}

const A_COPY: &str = "backups/schematics__farms__a.litematic/1000";
const B_COPY: &str = "backups/schematics__b.litematic/2000";
const LOOSE: &str = "backups/schematics__b.litematic/3000";

/// The history tab over lines for `A` and `B`, `B`'s copy gone, and one
/// copy of `B` no line names.
fn history(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    rig.serve(&files, cx);
    rig.answer(Event::CanWrite(true), cx);
    rig.answer(
        Event::History(Ok(vec![line(2, B, Some(B_COPY)), line(1, A, Some(A_COPY))])),
        cx,
    );
    rig.answer(
        Event::Backups(Ok(vec![copy(LOOSE, B, 2048), copy(A_COPY, A, 1536)])),
        cx,
    );
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::History;
        cx.notify();
    });
    cx.run_until_parked();
    (rig, cx)
}

#[test]
fn sizes_and_loose_copies_are_worded_from_the_folder() {
    assert_eq!(size_words(512), "512 B");
    assert_eq!(size_words(1536), "1.5 KB");
    assert_eq!(size_words(3 * 1024 * 1024), "3.0 MB");
    let lines = [line(1, A, Some(A_COPY)), line(2, B, None)];
    let kept = [copy(A_COPY, A, 1), copy(LOOSE, B, 1)];
    let loose: Vec<&str> = loose(&lines, &kept)
        .into_iter()
        .map(|kept| kept.path.as_str())
        .collect();
    assert_eq!(loose, [LOOSE]);
}

#[gpui::test]
fn each_line_says_whether_its_copy_is_kept(cx: &mut TestAppContext) {
    let (rig, cx) = history(cx);
    assert!(cx.debug_bounds("studio-history-summary").is_some());
    assert!(
        cx.debug_bounds("studio-history-filter").is_none(),
        "no file open: every file's history"
    );
    assert!(cx.debug_bounds("studio-history-copy-1").is_some());
    assert!(cx.debug_bounds("studio-history-copy-2").is_some());
    // B's copy is gone: nothing to put back.
    click(cx, "studio-restore-2");
    assert!(!cx.update(gpui_component::WindowExt::has_active_dialog));
    click(cx, "studio-restore-1");
    assert!(cx.update(gpui_component::WindowExt::has_active_dialog));
    assert!(rig.sent().is_empty(), "nothing before the person confirms");
}

#[gpui::test]
fn a_copy_no_line_names_is_listed_and_restores(cx: &mut TestAppContext) {
    let (rig, cx) = history(cx);
    assert!(cx.debug_bounds("studio-history-loose").is_some());
    click(cx, "studio-restore-loose-0");
    assert!(cx.update(gpui_component::WindowExt::has_active_dialog));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(
        rig.sent(),
        [Command::RestoreBackup {
            backup: LOOSE.into()
        }]
    );
}

#[gpui::test]
fn every_file_s_history_shows_unless_this_one_is_chosen(cx: &mut TestAppContext) {
    let (rig, cx) = history(cx);
    choose(cx, A);
    rig.serve(&[(A, fixture("A", 3))], cx);
    assert!(cx.debug_bounds("studio-history-filter").is_some());
    assert!(
        cx.debug_bounds("studio-restore-2").is_some(),
        "B's line too"
    );
    rig.studio.update(cx, |studio, cx| {
        studio.history_this_only = true;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-restore-1").is_some());
    assert!(cx.debug_bounds("studio-restore-2").is_none(), "B's line");
    assert!(
        cx.debug_bounds("studio-history-loose").is_none(),
        "B's loose copy"
    );
}
