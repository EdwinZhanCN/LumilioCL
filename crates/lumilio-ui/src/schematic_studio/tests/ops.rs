//! The studio's operations (T13–T16): what each dialog shows before it
//! writes, what it sends, and that cancelling or a refused draft sends
//! nothing.

use gpui::{TestAppContext, VisualTestContext};
use gpui_component::WindowExt as _;
use lumilio_core::{BlockCatalog, SchematicLogEntry};
use lumilio_schematic::{BlockState, Region, UniversalSchematic};

use super::{A, B, C, Rig, click, fixture, rig};
use crate::schematic_studio::dialogs::{
    ConvertDialog, DialogForm as _, MetadataDialog, OpenDialog, Planned, Preview, ReplaceDialog,
};
use crate::schematic_studio::{Command, Event, InspectorTab, WriteFailure};

fn entry(index: usize, operation: &str, file: &str) -> SchematicLogEntry {
    SchematicLogEntry {
        index,
        time: 0,
        operation: operation.into(),
        file: file.into(),
        backup: Some(format!("backups/{index}")),
        summary: String::new(),
        restores: None,
    }
}

/// Lists `files` as readable, allows writes and opens `chosen` in the
/// inspector.
fn open(rig: &Rig, cx: &mut VisualTestContext, files: &[(&str, Vec<u8>)], chosen: &str) {
    rig.answer(
        Event::Listed(Ok(files
            .iter()
            .map(|(file, _)| (*file).to_owned())
            .collect())),
        cx,
    );
    rig.answer(Event::CanWrite(true), cx);
    rig.serve(files, cx);
    super::choose(cx, chosen);
    rig.serve(files, cx);
    rig.sent();
}

/// Brings 信息 forward: metadata, entities and duplicates live there.
fn show_info(rig: &Rig, cx: &mut VisualTestContext) {
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::Info;
        cx.notify();
    });
    cx.run_until_parked();
}

fn has_dialog(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| window.has_active_dialog(cx))
}

fn applied(sent: &[Command]) -> Vec<(String, Vec<u8>, String, String)> {
    sent.iter()
        .filter_map(|command| match command {
            Command::Apply {
                file,
                bytes,
                operation,
                ..
            } => Some((
                file.clone(),
                bytes.clone(),
                operation.kind.clone(),
                operation.summary.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn count_of(bytes: &[u8], id: &str) -> u64 {
    let schematic = lumilio_schematic::load_litematic(bytes).unwrap();
    lumilio_schematic::stats::stats(&schematic)
        .blocks
        .iter()
        .find(|block| block.id == id)
        .map_or(0, |block| block.count)
}

fn replace_dialog(rig: &Rig, cx: &mut VisualTestContext) -> gpui::Entity<ReplaceDialog> {
    rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Replace(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the replace dialog is open"),
    })
}

#[gpui::test]
fn replace_shows_the_count_first_and_writes_exactly_that_many(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 3))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    // The head's one orange key (D27).
    click(cx, "studio-replace");
    assert!(has_dialog(cx));
    let dialog = replace_dialog(&rig, cx);
    // The most common block is chosen first; nothing is counted without a target.
    assert_eq!(
        dialog.read_with(cx, |dialog, _| dialog.preview.clone()),
        Preview::Waiting
    );

    super::give_catalog(
        &dialog,
        &["minecraft:stone", "minecraft:oak_stairs", "minecraft:glass"],
        cx,
    );
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_target_for_test(0, "minecraft:glass", window, cx)
        })
    });
    cx.run_until_parked();
    let shown = match dialog.read_with(cx, |dialog, _| dialog.preview.clone()) {
        Preview::Count(count, _) => count,
        other => panic!("a count is shown before confirming: {other:?}"),
    };
    assert_eq!(shown, 3);
    assert!(cx.debug_bounds("studio-replace-preview").is_some());
    assert!(
        rig.sent().is_empty(),
        "nothing is written before confirming"
    );

    click(cx, "studio-dialog-commit");
    let sent = rig.sent();
    let writes = applied(&sent);
    assert_eq!(writes.len(), 1, "{sent:?}");
    let (file, bytes, kind, summary) = &writes[0];
    assert_eq!((file.as_str(), kind.as_str()), (A, "replace"));
    assert_eq!(
        count_of(bytes, "minecraft:glass"),
        shown,
        "applied equals previewed"
    );
    assert_eq!(count_of(bytes, "minecraft:stone"), 0);
    assert!(summary.contains("minecraft:glass"), "{summary}");
    assert!(has_dialog(cx), "the dialog waits for the write");

    rig.answer(
        Event::Applied {
            file: A.into(),
            result: Ok(entry(1, "replace", A)),
        },
        cx,
    );
    assert!(!has_dialog(cx), "success closes the dialog");
    let after = rig.serve(&files, cx);
    assert!(after.contains(&Command::History), "{after:?}");
}

#[gpui::test]
fn a_refused_write_stays_in_the_dialog(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 2))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_replace(window, cx));
    cx.run_until_parked();
    let dialog = replace_dialog(&rig, cx);
    super::give_catalog(
        &dialog,
        &["minecraft:stone", "minecraft:oak_stairs", "minecraft:glass"],
        cx,
    );
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_target_for_test(0, "minecraft:glass", window, cx)
        })
    });
    cx.run_until_parked();
    click(cx, "studio-dialog-commit");
    rig.sent();
    rig.answer(
        Event::Applied {
            file: A.into(),
            result: Err(WriteFailure::Conflict),
        },
        cx,
    );
    assert!(has_dialog(cx));
    assert!(cx.debug_bounds("studio-dialog-error").is_some());
}

/// A cherry log at the newest data version: older versions cannot hold it.
fn cherry() -> Vec<u8> {
    let mut schematic = UniversalSchematic::new("Cherry".to_owned());
    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (1, 1, 1));
    region.set_block(
        0,
        0,
        0,
        &BlockState::from_block_string("minecraft:cherry_log[axis=y]").unwrap(),
    );
    schematic.add_region(region);
    lumilio_schematic::to_litematic_bytes(&schematic).unwrap()
}

fn convert_dialog(rig: &Rig, cx: &mut VisualTestContext) -> gpui::Entity<ConvertDialog> {
    rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Convert(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the convert dialog is open"),
    })
}

fn versions(newest: i32) -> Vec<(String, u32)> {
    vec![
        ("26.3".to_owned(), u32::try_from(newest).unwrap()),
        ("1.13.2".to_owned(), 1631),
    ]
}

/// What a 1.13.2 bundle slice says about logs: no cherry yet.
fn catalogs_1_13_2() -> (BlockCatalog, Option<BlockCatalog>) {
    let mut target = BlockCatalog::default();
    for axis in ["x", "y", "z"] {
        target.add_variant("minecraft:oak_log", &format!("axis={axis}"));
    }
    (target, None)
}

#[gpui::test]
fn a_backward_conversion_lists_its_losses_and_cancel_writes_nothing(cx: &mut TestAppContext) {
    let bytes = cherry();
    let newest = lumilio_schematic::convert::data_version(
        &lumilio_schematic::load_litematic(&bytes).unwrap(),
    );
    let files = [(A, bytes)];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    // The file's opening asked for the versions already (T23).
    rig.answer(Event::Versions(Ok(versions(newest))), cx);
    let dialog = convert_dialog(&rig, cx);
    dialog.update(cx, |dialog, cx| dialog.choose("1.13.2".into(), cx));
    cx.run_until_parked();
    // A backward conversion is checked against the target's block states.
    assert_eq!(
        rig.sent().last(),
        Some(&Command::Catalogs {
            target: "1.13.2".into(),
            source_data_version: u32::try_from(newest).unwrap(),
        })
    );
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Ok(catalogs_1_13_2()),
        },
        cx,
    );
    let losses = dialog.read_with(cx, |dialog, _| match &dialog.planned {
        Planned::Ready { conversion, .. } => conversion
            .losses
            .iter()
            .map(|loss| (loss.kind, loss.severity, loss.path.clone()))
            .collect::<Vec<_>>(),
        _ => panic!("the conversion was tried"),
    });
    assert_eq!(
        losses,
        [(
            lumilio_schematic::convert::MISSING_BLOCK,
            lumilio_schematic::convert::Severity::Lost,
            "minecraft:cherry_log".to_owned()
        )]
    );
    assert!(cx.debug_bounds("studio-convert-lossy").is_some());
    assert!(
        cx.debug_bounds("studio-loss-kept-0").is_some(),
        "a block the target lacks is kept, written as it is (T23)"
    );
    assert!(cx.debug_bounds("studio-convert-unchecked").is_none());

    click(cx, "studio-dialog-cancel");
    assert!(!has_dialog(cx));
    assert!(applied(&rig.sent()).is_empty(), "cancel writes nothing");
}

#[gpui::test]
fn without_the_targets_block_states_the_dialog_says_it_could_not_check(cx: &mut TestAppContext) {
    let bytes = cherry();
    let newest = lumilio_schematic::convert::data_version(
        &lumilio_schematic::load_litematic(&bytes).unwrap(),
    );
    let files = [(A, bytes)];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(Event::Versions(Ok(versions(newest))), cx);
    let dialog = convert_dialog(&rig, cx);
    dialog.update(cx, |dialog, cx| dialog.choose("1.13.2".into(), cx));
    cx.run_until_parked();
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Err("block asset index unreachable".into()),
        },
        cx,
    );
    assert!(cx.debug_bounds("studio-convert-unchecked").is_some());
    click(cx, "studio-dialog-commit");
    assert_eq!(
        applied(&rig.sent()).len(),
        1,
        "the conversion is still offered"
    );
}

#[gpui::test]
fn a_forward_conversion_writes_on_confirm(cx: &mut TestAppContext) {
    let mut old = lumilio_schematic::load_litematic(&fixture("Old", 2)).unwrap();
    let newest = lumilio_schematic::convert::data_version(&old);
    lumilio_schematic::convert::convert(&mut old, 1631, None);
    let files = [(A, lumilio_schematic::to_litematic_bytes(&old).unwrap())];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(Event::Versions(Ok(versions(newest))), cx);
    let dialog = convert_dialog(&rig, cx);
    dialog.update(cx, |dialog, cx| dialog.choose("26.3".into(), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-convert-lossy").is_none());
    click(cx, "studio-dialog-commit");
    let writes = applied(&rig.sent());
    assert_eq!(writes.len(), 1);
    let written = lumilio_schematic::load_litematic(&writes[0].1).unwrap();
    assert_eq!(lumilio_schematic::convert::data_version(&written), newest);
    assert_eq!(writes[0].2, "convert");
}

#[gpui::test]
fn an_empty_name_stays_in_the_dialog_and_a_name_writes_only_metadata(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 3))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::Info;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "studio-meta-edit");
    assert!(has_dialog(cx));
    let dialog: gpui::Entity<MetadataDialog> =
        rig.studio.read_with(cx, |studio, _| match &studio.dialog {
            Some(OpenDialog::Metadata(dialog)) => dialog.upgrade().unwrap(),
            _ => panic!("the metadata dialog is open"),
        });
    let name = dialog.read_with(cx, |dialog, _| dialog.name_input());
    cx.update(|window, cx| name.update(cx, |input, cx| input.set_value("   ", window, cx)));
    click(cx, "studio-dialog-commit");
    assert!(has_dialog(cx), "an empty name keeps the dialog open");
    assert!(cx.debug_bounds("studio-dialog-error").is_some());
    assert!(applied(&rig.sent()).is_empty());

    cx.update(|window, cx| name.update(cx, |input, cx| input.set_value("Tower", window, cx)));
    click(cx, "studio-dialog-commit");
    let writes = applied(&rig.sent());
    assert_eq!(writes.len(), 1);
    let before = lumilio_schematic::load_litematic(&files[0].1).unwrap();
    let after = lumilio_schematic::load_litematic(&writes[0].1).unwrap();
    assert_eq!(lumilio_schematic::metadata::read(&after).name, "Tower");
    assert_eq!(
        lumilio_schematic::fingerprint::fingerprint(&after),
        lumilio_schematic::fingerprint::fingerprint(&before),
        "the blocks are untouched"
    );
}

#[gpui::test]
fn keeping_one_moves_the_others_to_the_backups_and_history_lists_it(cx: &mut TestAppContext) {
    let files = [
        (A, fixture("A", 2)),
        (B, fixture("Other", 2)),
        (C, fixture("C", 3)),
    ];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, C);
    show_info(&rig, cx);
    click(cx, "studio-keep-schematics/b.litematic");
    assert!(
        cx.update(|window, cx| window.has_active_dialog(cx)),
        "it asks first"
    );
    assert!(rig.sent().is_empty(), "nothing moves before confirming");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let sent = rig.sent();
    let moved: Vec<_> = sent
        .iter()
        .filter_map(|command| match command {
            Command::Remove { files, operation } => Some((
                files
                    .iter()
                    .map(|(file, _)| file.clone())
                    .collect::<Vec<_>>(),
                operation.kind.clone(),
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        moved,
        [(vec![A.to_owned()], "dedupe".to_owned())],
        "{sent:?}"
    );

    let logged = entry(1, "dedupe", A);
    rig.answer(Event::Removed(Ok(vec![logged.clone()])), cx);
    let after = rig.sent();
    assert!(after.contains(&Command::History) && after.contains(&Command::List));
    rig.answer(Event::History(Ok(vec![logged])), cx);
    rig.answer(Event::Listed(Ok(vec![B.into(), C.into()])), cx);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::History;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("studio-restore-1").is_some(),
        "history lists it, restorable"
    );
    rig.studio
        .read_with(cx, |studio, _| assert!(studio.tree.duplicates().is_empty()));
}

#[gpui::test]
fn delete_asks_and_moves_the_file_to_the_backups(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 2))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    let menu = rig.studio.read_with(cx, |studio, _| studio.menu());
    let delete = menu.last().unwrap();
    assert!(delete.is_danger() && !delete.is_disabled());
    cx.update(|window, cx| delete.choose(window, cx));
    cx.run_until_parked();
    assert!(rig.sent().is_empty(), "nothing moves before confirming");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(rig.sent().iter().any(|command| matches!(
        command,
        Command::Remove { files, operation } if files.len() == 1 && files[0].0 == A && operation.kind == "delete"
    )));
}

#[gpui::test]
fn every_write_is_shut_while_the_game_runs(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 2)), (B, fixture("B", 2))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.answer(Event::CanWrite(false), cx);
    let menu = rig.studio.read_with(cx, |studio, _| studio.menu());
    // 转换版本…, 编辑信息… and 删除…; 导出 PNG… writes nothing.
    for index in [0, 2, menu.len() - 1] {
        assert!(menu[index].is_disabled(), "{}", menu[index].label());
        assert_eq!(
            menu[index].hint().map(|hint| hint.to_string()),
            Some(crate::tr!("studio-write-blocked").to_owned())
        );
    }
    click(cx, "studio-replace");
    assert!(!has_dialog(cx), "the head's replace key is shut too");
    show_info(&rig, cx);
    click(cx, "studio-keep-schematics/b.litematic");
    assert!(!has_dialog(cx));
    click(cx, "studio-meta-edit");
    assert!(!has_dialog(cx));
    assert!(rig.sent().is_empty());
}

/// Opens the cherry file with the bundle's versions and the instance's game
/// version `game`, answering the block check when one is asked for.
fn cherry_with_game<'a>(
    game: &str,
    cx: &'a mut TestAppContext,
) -> (Rig, &'a mut VisualTestContext, i32) {
    let bytes = cherry();
    let newest = lumilio_schematic::convert::data_version(
        &lumilio_schematic::load_litematic(&bytes).unwrap(),
    );
    let files = [(A, bytes)];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.answer(Event::Versions(Ok(versions(newest))), cx);
    rig.answer(Event::Game(Ok(game.to_owned())), cx);
    let asked = rig.sent();
    if asked.contains(&Command::Catalogs {
        target: game.to_owned(),
        source_data_version: u32::try_from(newest).unwrap(),
    }) {
        rig.answer(
            Event::Catalogs {
                target: game.to_owned(),
                result: Ok(catalogs_1_13_2()),
            },
            cx,
        );
    }
    cx.run_until_parked();
    (rig, cx, newest)
}

fn caption(rig: &Rig, cx: &mut VisualTestContext) -> String {
    rig.studio
        .read_with(cx, |studio, _| studio.version_words().unwrap_or_default())
}

/// T23: the head names the file's and the game's versions; when the game
/// version lacks some of the file's blocks it says how many kinds and
/// offers the conversion, and 材料 marks those blocks and can show only
/// them. A game of the file's own version gets one name and neither.
#[gpui::test]
fn the_head_names_both_versions_and_flags_blocks_the_game_lacks(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("1.13.2", cx);
    assert_eq!(caption(&rig, cx), "26.3 投影 · 游戏 1.13.2");
    assert!(cx.debug_bounds("studio-compat").is_some(), "the orange tag");
    assert!(cx.debug_bounds("studio-compat-convert").is_some());
    assert!(
        cx.debug_bounds("studio-tile-missing-minecraft:cherry_log")
            .is_some()
    );
    assert!(cx.debug_bounds("studio-only-incompatible").is_some());
    assert!(
        cx.debug_bounds("studio-only-incompatible-label").is_some(),
        "the switch says what it does"
    );

    click(cx, "studio-compat-convert");
    let dialog = convert_dialog(&rig, cx);
    assert!(
        dialog
            .read_with(cx, |dialog, _| dialog.planned_target().map(str::to_owned))
            .as_deref()
            == Some("1.13.2"),
        "the game's version is chosen"
    );
}

/// 转换版本… opened before the game's version is known still starts on
/// that version once it arrives.
#[gpui::test]
fn the_convert_dialog_takes_the_game_version_when_it_arrives(cx: &mut TestAppContext) {
    let bytes = cherry();
    let newest = lumilio_schematic::convert::data_version(
        &lumilio_schematic::load_litematic(&bytes).unwrap(),
    );
    let files = [(A, bytes)];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.answer(Event::Versions(Ok(versions(newest))), cx);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    let dialog = convert_dialog(&rig, cx);
    assert_eq!(
        dialog.read_with(cx, |dialog, _| dialog.planned_target().map(str::to_owned)),
        None
    );

    rig.answer(Event::Game(Ok("1.13.2".to_owned())), cx);
    cx.run_until_parked();
    assert_eq!(
        dialog
            .read_with(cx, |dialog, _| dialog.planned_target().map(str::to_owned))
            .as_deref(),
        Some("1.13.2")
    );
}

/// The review hook opens the replace dialog with one row for each block
/// the game version lacks; asked for a dialog it does not know, it opens
/// none.
#[gpui::test]
fn the_review_hook_opens_replace_on_the_blocks_the_game_lacks(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("1.13.2", cx);
    rig.studio
        .update(cx, |studio, cx| studio.review_dialog("unknown", cx));
    cx.run_until_parked();
    assert!(!has_dialog(cx));

    rig.studio
        .update(cx, |studio, cx| studio.review_dialog("incompatible", cx));
    cx.run_until_parked();
    replace_dialog(&rig, cx);
    let missing = rig
        .studio
        .read_with(cx, |studio, _| studio.missing_blocks().len());
    assert!(missing > 0);
    for row in 0..missing {
        let row = format!("studio-replace-target-{row}");
        assert!(cx.debug_bounds(Box::leak(row.into_boxed_str())).is_some());
    }
}

#[gpui::test]
fn the_same_version_gets_one_name_and_no_tag(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("26.3", cx);
    assert_eq!(caption(&rig, cx), "26.3");
    assert!(cx.debug_bounds("studio-compat").is_none());
    assert!(cx.debug_bounds("studio-compat-convert").is_none());
    assert!(cx.debug_bounds("studio-only-incompatible").is_none());
    assert!(
        cx.debug_bounds("studio-tile-missing-minecraft:cherry_log")
            .is_none()
    );
}

/// T23: losses fall into lost, approximated and kept; a block the target
/// does not know is kept, written as it is.
#[test]
fn losses_fall_into_lost_approximated_and_kept() {
    use crate::schematic_studio::dialogs::Group;
    use lumilio_schematic::convert::{Loss, MISSING_BLOCK, Severity, UNKNOWN_PROPERTY};
    let loss = |kind: &'static str, severity| Loss {
        severity,
        kind,
        version: 1631,
        path: String::new(),
        detail: String::new(),
        count: 1,
    };
    assert_eq!(Group::of(&loss(MISSING_BLOCK, Severity::Lost)), Group::Kept);
    assert_eq!(
        Group::of(&loss(UNKNOWN_PROPERTY, Severity::Approximated)),
        Group::Approximated
    );
    assert_eq!(
        Group::of(&loss("flattening_unknown_block", Severity::Lost)),
        Group::Lost
    );
}

/// The replace dialog's block pickers call blocks by name and keep the ID
/// as the value; the search finds a block by either, and a block with no
/// name is its ID (D28).
#[test]
fn block_pickers_show_names_and_find_names_and_ids() {
    use crate::schematic_studio::dialogs::BlockChoice;
    use gpui_component::select::SelectItem as _;
    let mut names = lumilio_core::BlockNames::default();
    names.add(
        "zh_cn",
        [("block.minecraft.dirt".to_owned(), "泥土".to_owned())].into(),
    );
    let dirt = BlockChoice::new(&names, "minecraft:dirt");
    assert_eq!(dirt.title().as_ref(), "泥土");
    assert_eq!(dirt.value(), "minecraft:dirt");
    assert!(dirt.matches("泥"));
    assert!(dirt.matches("DIRT"));
    assert!(!dirt.matches("stone"));
    let unnamed = BlockChoice::new(&names, "mymod:thing");
    assert_eq!(unnamed.title().as_ref(), "mymod:thing");
}

/// Each of Nucleation's stable loss kinds gets its own sentence in the
/// launcher's words, never Nucleation's English detail; a kind it adds
/// later gets the general one.
#[test]
fn every_loss_kind_is_explained_in_the_launchers_words() {
    use crate::schematic_studio::dialogs::loss_words;
    use lumilio_schematic::convert::{Loss, Severity};
    let loss = |kind: &'static str| Loss {
        severity: Severity::Lost,
        kind,
        version: 1631,
        path: "entity[0] minecraft:tropical_fish".into(),
        detail: "name-resolved attribute modifier lost its uuid".into(),
        count: 1,
    };
    let general = loss_words(&loss("a_kind_added_later"), "1.20.1");
    assert_eq!(general, loss_words(&loss("other"), "1.20.1"));
    let kinds = [
        "flattening_ambiguous",
        "flattening_unknown_block",
        "item_flattening_damage",
        "component_dropped",
        "entity_merge_ambiguous",
        "rename_ambiguous",
        "fingerprint_collapse",
        "unsupported_in_target",
    ];
    let mut seen = std::collections::BTreeSet::new();
    for kind in kinds {
        let words = loss_words(&loss(kind), "1.20.1");
        assert_ne!(words, general, "{kind} has its own sentence");
        assert!(!words.contains("modifier"), "{kind}: {words}");
        assert!(seen.insert(words), "{kind} is not worded like another kind");
    }
}

/// T23: a conversion is saved beside the original as `name (y).litematic`,
/// a new file, and the original is not written; 覆盖原文件 writes over it.
/// Kept blocks can be handed to the replace dialog.
#[gpui::test]
fn a_conversion_is_saved_beside_and_the_original_stays(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("1.13.2", cx);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Ok(catalogs_1_13_2()),
        },
        cx,
    );
    assert!(cx.debug_bounds("studio-convert-save-as").is_some());
    assert!(cx.debug_bounds("studio-convert-replace-kept").is_some());
    click(cx, "studio-dialog-commit");
    let sent = rig.sent();
    let beside = "schematics/farms/a (1.13.2).litematic";
    let expected: Vec<_> = sent
        .iter()
        .filter_map(|command| match command {
            Command::Apply { file, expected, .. } => Some((file.clone(), expected.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(
        expected,
        [(beside.to_owned(), false)],
        "a new file, the original untouched"
    );
    rig.answer(
        Event::Applied {
            file: beside.into(),
            result: Ok(entry(1, "convert", beside)),
        },
        cx,
    );
    rig.studio.read_with(cx, |studio, _| {
        assert_eq!(
            studio.selected.as_deref(),
            Some(beside),
            "the new file is chosen"
        );
    });

    // Back on the original: over it this time.
    super::choose(cx, A);
    rig.serve(&[(A, cherry())], cx);
    rig.answer(Event::CanWrite(true), cx);
    rig.sent();
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Ok(catalogs_1_13_2()),
        },
        cx,
    );
    click(cx, "studio-convert-overwrite");
    assert!(cx.debug_bounds("studio-convert-save-as").is_none());
    click(cx, "studio-dialog-commit");
    let over: Vec<_> = rig
        .sent()
        .iter()
        .filter_map(|command| match command {
            Command::Apply { file, expected, .. } => Some((file.clone(), expected.is_some())),
            _ => None,
        })
        .collect();
    assert_eq!(over, [(A.to_owned(), true)]);
}

#[gpui::test]
fn kept_blocks_go_to_the_replace_dialog(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("1.13.2", cx);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Ok(catalogs_1_13_2()),
        },
        cx,
    );
    click(cx, "studio-convert-replace-kept");
    let dialog = rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Replace(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the replace dialog took over"),
    });
    let source = dialog.read_with(cx, |dialog, cx| dialog.source_id(cx));
    assert_eq!(source.as_deref(), Some("minecraft:cherry_log"));
}

/// T37: a kept block comes with a proposal from the target's blocks, and
/// the replace dialog starts its row on it once its table lists it.
#[gpui::test]
fn kept_blocks_come_with_a_suggested_replacement(cx: &mut TestAppContext) {
    let (rig, cx, _) = cherry_with_game("1.13.2", cx);
    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_convert(window, cx));
    cx.run_until_parked();
    rig.answer(
        Event::Catalogs {
            target: "1.13.2".into(),
            result: Ok(catalogs_1_13_2()),
        },
        cx,
    );
    assert!(
        cx.debug_bounds("studio-loss-kept-0-suggested").is_some(),
        "the proposal is shown under the kept block"
    );
    rig.sent();
    click(cx, "studio-convert-replace-kept");
    let dialog = replace_dialog(&rig, cx);
    // The file's own version's table arrives; the proposal is in it.
    let asked = rig.sent();
    assert!(
        asked
            .iter()
            .any(|command| matches!(command, Command::Catalogs { target, .. } if target == "26.3")),
        "{asked:?}"
    );
    let mut table = BlockCatalog::default();
    for id in [
        "minecraft:cherry_log",
        "minecraft:oak_log",
        "minecraft:stone",
    ] {
        table.add_block(id);
    }
    rig.answer(
        Event::Catalogs {
            target: "26.3".into(),
            result: Ok((table, None)),
        },
        cx,
    );
    cx.run_until_parked();
    let targets = dialog.read_with(cx, |dialog, cx| dialog.chosen_targets(cx));
    assert_eq!(targets, [Some("minecraft:oak_log".to_owned())]);
}

/// T39: a file that names no data version says so in the head, is not
/// converted to check it against the game, and cannot be converted.
#[gpui::test]
fn a_file_without_a_version_says_so_and_is_not_converted(cx: &mut TestAppContext) {
    let files = [(A, fixture("A", 3))];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    rig.studio.update(cx, |studio, cx| {
        if let crate::schematic_studio::Inspection::Ready(analysis) = &mut studio.inspection {
            analysis.info.data_version = None;
        }
        cx.notify();
    });
    cx.run_until_parked();
    rig.sent();
    rig.answer(Event::Versions(Ok(versions(4790))), cx);
    rig.answer(Event::Game(Ok("1.13.2".to_owned())), cx);
    assert!(cx.debug_bounds("studio-version-unknown").is_some());
    assert_eq!(caption(&rig, cx), "版本未知 · 游戏 1.13.2");
    assert!(
        !rig.sent()
            .iter()
            .any(|command| matches!(command, Command::Catalogs { .. })),
        "no block check, which would convert from a guessed version"
    );
    assert!(cx.debug_bounds("studio-compat").is_none());
    let convert = rig.studio.read_with(cx, |studio, _| {
        studio
            .menu()
            .into_iter()
            .find(|entry| entry.label().as_ref() == crate::tr!("studio-menu-convert"))
            .expect("the convert entry")
    });
    assert!(convert.is_disabled(), "a guessed version is not converted");
}

/// Two oak stairs facing north, one facing east, and three stone.
fn stairs() -> Vec<u8> {
    let mut schematic = UniversalSchematic::new("Stairs".to_owned());
    let mut region = Region::new("Main".to_owned(), (0, 0, 0), (6, 1, 1));
    let block = |text: &str| BlockState::from_block_string(text).unwrap();
    for x in 0..3 {
        region.set_block(x, 0, 0, &block("minecraft:stone"));
    }
    let stair = |facing: &str| {
        block(&format!(
            "minecraft:oak_stairs[facing={facing},half=bottom,shape=straight,waterlogged=false]"
        ))
    };
    region.set_block(3, 0, 0, &stair("north"));
    region.set_block(4, 0, 0, &stair("north"));
    region.set_block(5, 0, 0, &stair("east"));
    schematic.add_region(region);
    lumilio_schematic::to_litematic_bytes(&schematic).unwrap()
}

/// The block table a replacement offers in these tests: stone, glass and
/// stairs whose blockstates name facing, half and shape.
fn stairs_catalog() -> BlockCatalog {
    let mut catalog = BlockCatalog::default();
    for id in ["minecraft:stone", "minecraft:glass", "minecraft:oak_stairs"] {
        catalog.add_block(id);
    }
    for facing in ["north", "south", "east", "west"] {
        catalog.add_variant(
            "minecraft:spruce_stairs",
            &format!("facing={facing},half=bottom,shape=straight"),
        );
    }
    catalog
}

fn open_replace_on(
    rig: &Rig,
    cx: &mut VisualTestContext,
    state: Option<&str>,
) -> gpui::Entity<ReplaceDialog> {
    rig.studio.update_in(cx, |studio, window, cx| match state {
        Some(state) => studio.open_replace_for(state, window, cx),
        None => studio.open_replace(window, cx),
    });
    cx.run_until_parked();
    let dialog = replace_dialog(rig, cx);
    dialog.update(cx, |dialog, cx| {
        dialog.set_catalog(Ok(stairs_catalog()), cx)
    });
    cx.run_until_parked();
    dialog
}

fn count_now(dialog: &gpui::Entity<ReplaceDialog>, cx: &mut VisualTestContext) -> (u64, Vec<u64>) {
    cx.run_until_parked();
    match dialog.read_with(cx, |dialog, _| dialog.preview.clone()) {
        Preview::Count(total, rows) => (total, rows),
        other => panic!("counted: {other:?}"),
    }
}

/// T24: from a block in the picture each property is matched exactly; from
/// a kind, any state; 不限属性 takes every state. The target keeps the
/// properties it shares with each block it replaces.
#[gpui::test]
fn chips_match_exactly_or_any_and_the_target_keeps_shared_properties(cx: &mut TestAppContext) {
    let files = [(A, stairs())];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    let north = "minecraft:oak_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]";
    let dialog = open_replace_on(&rig, cx, Some(north));
    let (any, chips) = dialog.read_with(cx, |dialog, _| dialog.chips()).remove(0);
    assert!(!any, "from the picture: exact");
    assert!(chips.contains(&"facing=north".to_owned()), "{chips:?}");
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_target_for_test(0, "minecraft:spruce_stairs", window, cx)
        })
    });
    assert_eq!(count_now(&dialog, cx).0, 2, "the two facing north");
    dialog.update(cx, |dialog, cx| dialog.set_any(0, true, cx));
    assert_eq!(count_now(&dialog, cx).0, 3, "every state");
    click(cx, "studio-dialog-commit");
    let writes = applied(&rig.sent());
    let written = &writes[0].1;
    assert_eq!(
        count_of(written, "minecraft:spruce_stairs"),
        3,
        "{:?}",
        writes[0].3
    );
    let schematic = lumilio_schematic::load_litematic(written).unwrap();
    let facing = |x: i32| {
        schematic
            .get_block(x, 0, 0)
            .and_then(|block| block.get_property("facing").map(|v| v.to_string()))
    };
    assert_eq!(
        [facing(3), facing(4), facing(5)],
        [
            Some("north".into()),
            Some("north".into()),
            Some("east".into())
        ],
        "each keeps its facing"
    );
    click(cx, "studio-dialog-cancel");

    // From a kind's detail: any state.
    let dialog = open_replace_on(&rig, cx, Some("minecraft:oak_stairs"));
    assert!(dialog.read_with(cx, |dialog, _| dialog.chips())[0].0);
}

/// T24: rows replace together and the dialog says each one's count; the
/// write changes as many as it said, in one write; the targets used come
/// first next time.
#[gpui::test]
fn several_rows_write_once_with_the_counts_shown(cx: &mut TestAppContext) {
    let files = [(A, stairs())];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    let dialog = open_replace_on(&rig, cx, None);
    click(cx, "studio-replace-add-row");
    assert_eq!(
        dialog.read_with(cx, |dialog, cx| dialog.sources(cx)),
        [
            Some("minecraft:oak_stairs".into()),
            Some("minecraft:stone".into())
        ],
        "the first row is the most common block; the second starts on the next"
    );
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_target_for_test(0, "minecraft:spruce_stairs", window, cx);
            dialog.set_target_for_test(1, "minecraft:glass", window, cx);
        })
    });
    let (total, rows) = count_now(&dialog, cx);
    assert_eq!((total, rows.as_slice()), (6, &[3, 3][..]));
    let label = dialog.read_with(cx, |dialog, _| dialog.commit_label());
    assert_eq!(label.as_ref(), "替换 6 个方块");
    click(cx, "studio-dialog-commit");
    let sent = rig.sent();
    let writes = applied(&sent);
    assert_eq!(writes.len(), 1, "one write");
    assert_eq!(count_of(&writes[0].1, "minecraft:glass"), 3);
    assert_eq!(count_of(&writes[0].1, "minecraft:spruce_stairs"), 3);
    let recent = sent.iter().find_map(|command| match command {
        Command::SaveSchemes(schemes) => Some(schemes.recent.clone()),
        _ => None,
    });
    assert_eq!(
        recent.as_deref(),
        Some(
            &[
                "minecraft:spruce_stairs".to_owned(),
                "minecraft:glass".to_owned()
            ][..]
        )
    );
}

/// T24: a table is kept as a named 方案 and applied to another schematic;
/// the history line names it.
#[gpui::test]
fn a_scheme_is_saved_and_applied_elsewhere(cx: &mut TestAppContext) {
    let files = [(A, stairs()), (B, stairs())];
    let (rig, cx) = rig(cx);
    open(&rig, cx, &files, A);
    let dialog = open_replace_on(&rig, cx, Some("minecraft:oak_stairs"));
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_target_for_test(0, "minecraft:spruce_stairs", window, cx)
        })
    });
    let input = dialog.read_with(cx, |dialog, _| dialog.scheme_name_for_test());
    cx.update(|window, cx| input.update(cx, |input, cx| input.set_value("spruce", window, cx)));
    cx.run_until_parked();
    click(cx, "studio-replace-scheme-save");
    let saved = rig
        .sent()
        .into_iter()
        .find_map(|command| match command {
            Command::SaveSchemes(schemes) => Some(schemes),
            _ => None,
        })
        .expect("kept in launcher data");
    assert_eq!(saved.schemes["spruce"][0].target, "minecraft:spruce_stairs");
    rig.answer(Event::Schemes(Ok(saved)), cx);
    click(cx, "studio-dialog-cancel");

    super::choose(cx, B);
    rig.serve(&files, cx);
    rig.sent();
    let dialog = open_replace_on(&rig, cx, None);
    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| dialog.apply_scheme("spruce", window, cx))
    });
    assert_eq!(count_now(&dialog, cx).0, 3);
    click(cx, "studio-dialog-commit");
    let writes = applied(&rig.sent());
    assert_eq!(writes[0].0, B);
    assert!(
        writes[0].3.starts_with("方案「spruce」"),
        "the line names the scheme: {}",
        writes[0].3
    );
}
