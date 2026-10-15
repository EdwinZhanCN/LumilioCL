//! The resident viewport (T10, T30, T32): PNG export through the dialog and
//! the save prompt, a click inspecting one block, the read-out plate, and
//! releasing the old viewport's worker.

use gpui::{TestAppContext, VisualTestContext};
use lumilio_schematic::inspect::{Contents, Grid, Stack};
use lumilio_schematic_render::{Angle, Frame, Picked, View};

use super::{A, B, Rig, choose, click, fixture, rig};
use crate::model_view::{Inspected, Viewer};
use crate::schematic_studio::dialogs::{DialogForm as _, OpenDialog};
use crate::schematic_studio::export::{MAX_SIDE, encode_png, export_view, side};
use crate::schematic_studio::viewport::split_state;
use crate::schematic_studio::{Event, InspectorTab};
use lumilio_schematic::replace::PropertyMatch;

fn open_b(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    rig.serve(&files, cx);
    choose(cx, B);
    rig.serve(&files, cx);
    (rig, cx)
}

fn viewer(rig: &Rig, cx: &mut VisualTestContext) -> gpui::Entity<Viewer> {
    rig.studio
        .read_with(cx, |studio, _| studio.viewer.clone())
        .expect("a viewport")
}

#[test]
fn sides_are_whole_pixels_up_to_the_cap() {
    assert_eq!(side(" 1920 "), Some(1920));
    assert_eq!(side("4096"), Some(MAX_SIDE));
    for bad in ["4097", "0", "-5", "12.5", "", "宽"] {
        assert_eq!(side(bad), None, "{bad:?}");
    }
}

#[test]
fn an_export_view_takes_the_angle_and_a_clear_background() {
    let current = View {
        background: Some([0.1, 0.2, 0.3, 1.]),
        ..View::new().orbit(17., 3.).zoomed(2.)
    };
    assert_eq!(export_view(current, None, false), current);
    let north = export_view(current, Some(Angle::North), true);
    assert_eq!((north.yaw_deg, north.pitch_deg, north.zoom), (180., 0., 1.));
    assert_eq!(north.background, Some([0., 0., 0., 0.]));
}

#[test]
fn a_frame_becomes_a_png_of_its_size_with_its_alpha() {
    // 3 × 2, blue first: the first pixel opaque red, the rest clear.
    let mut bgra = vec![0u8; 3 * 2 * 4];
    bgra[..4].copy_from_slice(&[0, 0, 255, 255]);
    let png = encode_png(Frame {
        width: 3,
        height: 2,
        bgra,
    })
    .unwrap();
    let image = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (3, 2));
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(2, 1).0[3], 0, "empty space stays clear");
}

/// The menu opens the dialog; committing asks the viewport's worker for a
/// frame of the requested size and view, then the save prompt chooses where
/// the PNG goes and only then is it written.
#[gpui::test]
fn export_renders_the_requested_size_and_saves_where_the_person_chooses(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let mailbox = viewer(&rig, cx).update(cx, |viewer, _| viewer.start_idle_worker());
    let menu = rig.studio.read_with(cx, |studio, _| studio.menu());
    let png = menu
        .iter()
        .find(|entry| entry.label().as_ref() == crate::tr!("studio-menu-png"))
        .expect("the PNG entry");
    assert!(!png.is_disabled(), "a chosen file can be exported");
    cx.update(|window, cx| png.choose(window, cx));
    cx.run_until_parked();
    let dialog = rig
        .studio
        .read_with(cx, |studio, _| studio.export.clone())
        .and_then(|dialog| dialog.upgrade())
        .expect("the export dialog");

    cx.update(|window, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.set_size("5000", "300", window, cx);
            assert!(!dialog.ready(cx), "more than 4096 is refused");
            dialog.set_size("640", "300", window, cx);
            assert!(dialog.ready(cx));
            dialog.set_choice(Some(Angle::Top), true);
            dialog.commit(window, cx);
        })
    });
    cx.run_until_parked();
    let Some(crate::model_view::Work::Job(crate::model_view::WorkerJob::Export { request, reply })) =
        mailbox.try_take()
    else {
        panic!("an export job for the worker");
    };
    assert_eq!((request.width, request.height), (640, 300));
    assert_eq!(request.view.pitch_deg, View::MAX_PITCH);
    assert_eq!(request.view.background, Some([0., 0., 0., 0.]));

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("b.png");
    let chosen = target.clone();
    reply
        .try_send(Ok(Frame {
            width: 640,
            height: 300,
            bgra: vec![0; 640 * 300 * 4],
        }))
        .unwrap();
    cx.run_until_parked();
    assert!(cx.did_prompt_for_new_path(), "the host's save dialog asks");
    assert!(
        !target.exists(),
        "nothing is written before a place is chosen"
    );
    cx.simulate_new_path_selection(move |_| Some(chosen));
    cx.run_until_parked();
    let image = image::load_from_memory(&std::fs::read(&target).unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(image.dimensions(), (640, 300));
    assert_eq!(image.get_pixel(0, 0).0[3], 0);
}

#[gpui::test]
fn exporting_before_the_preview_is_ready_stays_in_the_dialog(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let menu = rig.studio.read_with(cx, |studio, _| studio.menu());
    let png = menu
        .iter()
        .find(|entry| entry.label().as_ref() == crate::tr!("studio-menu-png"))
        .unwrap();
    cx.update(|window, cx| png.choose(window, cx));
    cx.run_until_parked();
    let dialog = rig
        .studio
        .read_with(cx, |studio, _| studio.export.clone())
        .and_then(|dialog| dialog.upgrade())
        .unwrap();
    cx.update(|window, cx| dialog.update(cx, |dialog, cx| dialog.commit(window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-dialog-error").is_some());
    assert!(!cx.did_prompt_for_new_path());
}

/// Choosing another file drops the old viewport: its worker's mailbox is
/// closed, so the thread stops, and an export still waiting on it ends.
#[gpui::test]
fn switching_files_releases_the_old_worker_and_ends_its_export(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let old = viewer(&rig, cx);
    let mailbox = old.update(cx, |viewer, _| viewer.start_idle_worker());
    let waiting = old.read_with(cx, |viewer, _| viewer.export(View::new(), 64, 64).unwrap());
    let weak = old.downgrade();
    drop(old);
    choose(cx, A);
    assert!(
        weak.upgrade().is_none(),
        "the old viewport and its frames are gone"
    );
    assert!(mailbox.is_closed(), "the old worker was told to stop");
    assert!(waiting.recv_blocking().is_err(), "its export ends");
}

#[test]
fn a_state_splits_into_its_id_and_properties() {
    assert_eq!(
        split_state("minecraft:oak_stairs[facing=east, half=top]"),
        (
            "minecraft:oak_stairs".to_owned(),
            vec![
                ("facing".to_owned(), "east".to_owned()),
                ("half".to_owned(), "top".to_owned())
            ]
        )
    );
    assert_eq!(
        split_state("minecraft:stone"),
        ("minecraft:stone".to_owned(), Vec::new())
    );
}

fn stairs() -> Picked {
    Picked {
        position: [5, 0, 0],
        state: "minecraft:oak_stairs[facing=north]".into(),
    }
}

fn inspected(contents: Option<Contents>) -> Inspected {
    Inspected {
        picked: stairs(),
        contents,
    }
}

fn inspect(rig: &Rig, cx: &mut VisualTestContext, contents: Option<Contents>) {
    viewer(rig, cx).update(cx, |viewer, cx| {
        viewer.inspected_arrived(Some(inspected(contents)), cx)
    });
    cx.run_until_parked();
}

/// T32, D31: a click in the picture inspects that one block. Its card takes
/// the plate's place (at the lower left while the block's place on screen
/// is not known); the materials keep their tab and their choice, which only
/// the card's 选中这种 changes.
#[gpui::test]
fn a_click_inspects_one_block_and_leaves_the_chosen_kind_alone(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    viewer(&rig, cx).update(cx, |viewer, cx| viewer.set_drawn_for_test(cx));
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::History;
        cx.notify();
    });
    inspect(&rig, cx, None);
    rig.studio.read_with(cx, |studio, _| {
        assert_eq!(studio.chosen, None, "no kind is chosen by a click");
        assert_eq!(studio.tab, InspectorTab::History, "the column stays");
    });
    let card = cx.debug_bounds("studio-inspect").expect("the inspect card");
    let picture = cx.debug_bounds("studio-viewport").unwrap();
    let tools = cx.debug_bounds("model-tools").unwrap();
    assert!(
        card.left() < picture.center().x && card.bottom() <= tools.top(),
        "at the lower left, above the tools: {card:?} in {picture:?}"
    );
    assert!(
        cx.debug_bounds("studio-read-out").is_none(),
        "the card takes the plate's place"
    );
    assert!(cx.debug_bounds("studio-inspect-id").is_some());
    assert!(
        cx.debug_bounds("studio-inspect-property-0").is_some(),
        "a tag per property"
    );
    assert!(cx.debug_bounds("studio-inspect-contents").is_none());

    click(cx, "studio-inspect-kind");
    rig.studio.read_with(cx, |studio, _| {
        assert_eq!(studio.chosen.as_deref(), Some("minecraft:oak_stairs"));
        assert_eq!(
            studio.tab,
            InspectorTab::Materials,
            "materials come forward"
        );
    });
    assert!(
        cx.debug_bounds("studio-detail").is_some(),
        "the kind's detail"
    );
    assert!(
        cx.debug_bounds("studio-inspect").is_some(),
        "the card stays"
    );

    // An empty click lets the block go; the chosen kind stays.
    viewer(&rig, cx).update(cx, |viewer, cx| viewer.inspected_arrived(None, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-inspect").is_none());
    assert!(
        cx.debug_bounds("studio-read-out").is_some(),
        "the plate again"
    );
    rig.studio
        .read_with(cx, |studio, _| assert!(studio.chosen.is_some()));

    inspect(&rig, cx, None);
    click(cx, "studio-inspect-close");
    assert!(
        cx.debug_bounds("studio-inspect").is_none(),
        "closed by its key"
    );
    assert!(viewer(&rig, cx).read_with(cx, |viewer, _| viewer.inspected().is_none()));
}

/// The card floats over the block and moves with it: over it while there is
/// room, under it near the picture's top, inside the picture always.
#[gpui::test]
fn the_card_floats_over_the_inspected_block(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    viewer(&rig, cx).update(cx, |viewer, cx| viewer.set_drawn_for_test(cx));
    inspect(&rig, cx, None);
    let picture = cx.debug_bounds("model-viewport").unwrap();
    let at = |fraction: f32, along: gpui::Pixels, from: gpui::Pixels| from + along * fraction;
    for (rect, over) in [
        ([0.45, 0.6, 0.5, 0.68], true),
        ([0.1, 0.02, 0.15, 0.1], false),
    ] {
        viewer(&rig, cx).update(cx, |viewer, cx| {
            viewer.set_mark_rect_for_test(Some(rect));
            cx.notify();
        });
        cx.run_until_parked();
        let card = cx.debug_bounds("studio-inspect").unwrap();
        let (top, bottom) = (
            at(rect[1], picture.size.height, picture.top()),
            at(rect[3], picture.size.height, picture.top()),
        );
        if over {
            assert!(card.bottom() <= top, "over the block: {card:?}");
            let centre = at((rect[0] + rect[2]) / 2., picture.size.width, picture.left());
            assert!(
                card.left() < centre && centre < card.right(),
                "centred on it: {card:?}"
            );
        } else {
            assert!(
                card.top() >= bottom,
                "no room over it, so under it: {card:?}"
            );
        }
        assert!(
            card.left() >= picture.left() && card.right() <= picture.right(),
            "inside the picture: {card:?} in {picture:?}"
        );
    }
}

#[test]
fn the_card_keeps_inside_the_picture_and_falls_back_to_the_corner() {
    use crate::schematic_studio::inspect_card::placement;
    let size = (800., 600.);
    // At the picture's right edge it is pulled in.
    let edge = placement(Some([0.97, 0.5, 1.0, 0.55]), size);
    assert!(edge.left + 216. <= 800. - 8., "{edge:?}");
    assert_eq!(edge.bottom, Some(600. - 300. + 8.));
    // Behind the eye or off the picture: the lower left, over the tools.
    let corner = placement(None, size);
    assert_eq!((corner.left, corner.top), (12., None));
    assert_eq!(placement(Some([1.2, 0.5, 1.3, 0.6]), size), corner);
    // A block filling the picture leaves no room either side.
    assert_eq!(placement(Some([0.1, 0.01, 0.9, 0.95]), size), corner);
}

/// The card shows a container's slots as its screen lays them out, asks the
/// viewport's worker once for each item's picture, and keeps the given name
/// and text; the rest is behind 技术详情.
#[gpui::test]
fn the_inspect_card_shows_what_a_block_entity_holds(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let mut names = lumilio_core::BlockNames::default();
    names.add(
        "zh_cn",
        [
            (
                "block.minecraft.oak_stairs".to_owned(),
                "橡木楼梯".to_owned(),
            ),
            ("item.minecraft.diamond".to_owned(), "钻石".to_owned()),
        ]
        .into(),
    );
    rig.answer(Event::Names(Ok(names)), cx);
    let mailbox = viewer(&rig, cx).update(cx, |viewer, cx| {
        viewer.set_drawn_for_test(cx);
        viewer.start_idle_worker()
    });
    let stack = |slot, count| Stack {
        slot: Some(slot),
        id: "minecraft:diamond".into(),
        count,
    };
    inspect(
        &rig,
        cx,
        Some(Contents {
            id: "minecraft:chest".into(),
            name: Some("Loot".into()),
            items: Some(vec![stack(0, 3), stack(5, 2)]),
            grid: Some(Grid {
                columns: 9,
                slots: 27,
            }),
            text: vec!["Hello".into()],
            data: "Items: []".into(),
        }),
    );
    assert!(
        cx.debug_bounds("studio-inspect-slot-26").is_some(),
        "27 slots"
    );
    assert!(cx.debug_bounds("studio-inspect-slot-27").is_none());
    let (first, tenth) = (
        cx.debug_bounds("studio-inspect-slot-0").unwrap(),
        cx.debug_bounds("studio-inspect-slot-9").unwrap(),
    );
    assert!(
        tenth.top() > first.bottom() && tenth.left() == first.left(),
        "rows of nine"
    );
    let card = cx.debug_bounds("studio-inspect").unwrap();
    let last = cx.debug_bounds("studio-inspect-slot-8").unwrap();
    assert!(last.right() <= card.right(), "a row fits the card");

    let asked: Vec<Vec<String>> = std::iter::from_fn(|| mailbox.try_take())
        .filter_map(|work| match work {
            crate::model_view::Work::Job(crate::model_view::WorkerJob::ItemIcons {
                items,
                reply,
                ..
            }) => {
                let picture = Frame {
                    width: 32,
                    height: 32,
                    bgra: vec![255; 32 * 32 * 4],
                };
                reply.try_send(Ok(vec![Some(picture)])).unwrap();
                Some(items)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        asked,
        [vec!["minecraft:diamond".to_owned()]],
        "once per item"
    );
    cx.run_until_parked();
    rig.studio.read_with(cx, |studio, _| {
        assert!(studio.item_icons.contains_key("minecraft:diamond"));
    });
    assert!(cx.debug_bounds("studio-inspect-given-name").is_some());
    assert!(cx.debug_bounds("studio-inspect-text").is_some());
    assert!(cx.debug_bounds("studio-inspect-data").is_some(), "技术详情");
    assert_eq!(
        crate::tr!("studio-inspect-stack", name = "钻石", count = 5),
        "钻石 ×5"
    );
    assert_eq!(
        rig.studio
            .read_with(cx, |studio, _| studio.name_of("minecraft:oak_stairs")),
        "橡木楼梯"
    );
}

/// D27: a tile chooses its kind; the detail's 替换… opens on the kind, any
/// state.
#[gpui::test]
fn a_chosen_tile_shows_its_detail_and_replaces_the_kind(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    rig.answer(Event::CanWrite(true), cx);
    click(cx, "studio-tile-minecraft:oak_stairs");
    let detail = cx.debug_bounds("studio-detail").expect("the kind's detail");
    let materials = cx.debug_bounds("studio-inspector").unwrap();
    assert!(
        detail.left() >= materials.left() && detail.bottom() <= materials.bottom(),
        "the detail sits in the materials column: {detail:?} in {materials:?}"
    );
    assert!(cx.debug_bounds("studio-detail-count").is_some());
    assert!(
        cx.debug_bounds("studio-state-minecraft:oak_stairs-0")
            .is_some(),
        "a chip per state"
    );
    assert!(cx.debug_bounds("studio-detail-undrawable").is_none());

    click(cx, "studio-detail-replace");
    assert!(cx.update(gpui_component::WindowExt::has_active_dialog));
    let dialog = rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Replace(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the replace dialog is open"),
    });
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
    let plan = dialog
        .read_with(cx, |dialog, cx| dialog.plan(cx))
        .expect("source and target are complete");
    assert_eq!(plan.rows[0].source.id, "minecraft:oak_stairs");
    assert!(
        plan.rows[0]
            .source
            .properties
            .iter()
            .all(|(_, matched)| !matches!(matched, PropertyMatch::Exact(_))),
        "a kind matches any of its states: {:?}",
        plan.rows[0].source.properties
    );

    click(cx, "studio-dialog-cancel");
    click(cx, "studio-detail-close");
    assert!(cx.debug_bounds("studio-detail").is_none());
}

/// The plate names what the pointer is on with the game's name, not the
/// ID; off the picture it gives the controls (D27).
#[gpui::test]
fn the_read_out_names_the_block_under_the_pointer(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let mut names = lumilio_core::BlockNames::default();
    names.add(
        "zh_cn",
        [(
            "block.minecraft.oak_stairs".to_owned(),
            "橡木楼梯".to_owned(),
        )]
        .into(),
    );
    rig.answer(Event::Names(Ok(names)), cx);
    assert!(
        cx.debug_bounds("studio-read-out").is_none(),
        "not drawn yet"
    );
    viewer(&rig, cx).update(cx, |viewer, cx| viewer.set_drawn_for_test(cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-read-out-hint").is_some());
    assert!(
        cx.debug_bounds("studio-inspect").is_none(),
        "no card before a click"
    );

    viewer(&rig, cx).update(cx, |viewer, cx| viewer.hovered_for_test(Some(stairs()), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-read-out-name").is_some());
    let (_, name, words) = rig
        .studio
        .read_with(cx, |studio, _| studio.read_out_words(&stairs().state));
    assert_eq!(name, "橡木楼梯");
    assert_eq!(words, [crate::tr!("studio-prop-facing-north").to_owned()]);
    let plate = cx.debug_bounds("studio-read-out").unwrap();
    let tools = cx.debug_bounds("model-tools").expect("the floating tools");
    assert!(plate.bottom() <= tools.top(), "{plate:?} above {tools:?}");

    // A block without a known name falls back to its ID.
    let (_, name, _) = rig
        .studio
        .read_with(cx, |studio, _| studio.read_out_words("minecraft:stone"));
    assert_eq!(name, "minecraft:stone");
}

#[gpui::test]
fn a_block_the_pack_cannot_draw_says_so_in_its_detail(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    viewer(&rig, cx).update(cx, |viewer, _| {
        viewer.set_undrawable_for_test(vec!["minecraft:oak_stairs".into()]);
    });
    click(cx, "studio-tile-minecraft:oak_stairs");
    assert!(cx.debug_bounds("studio-detail-undrawable").is_some());
}

/// T31: the chosen kind keeps its colour, a hovered tile's kind takes over
/// while the pointer is on it, and 只看这种 hides the rest; closing the
/// detail shows every block again. Only the focus changes, never the mesh.
#[gpui::test]
fn the_picture_follows_the_chosen_and_hovered_tiles(cx: &mut TestAppContext) {
    use lumilio_schematic_render::Focus;
    let (rig, cx) = open_b(cx);
    let focus = |rig: &Rig, cx: &mut VisualTestContext| {
        viewer(rig, cx).read_with(cx, |viewer, _| viewer.block_focus().clone())
    };
    assert_eq!(focus(&rig, cx), Focus::All);
    click(cx, "studio-tile-minecraft:stone");
    let stone = || Focus::Fade("minecraft:stone".into());
    assert_eq!(focus(&rig, cx), stone());

    let stairs = cx
        .debug_bounds("studio-tile-minecraft:oak_stairs")
        .unwrap()
        .center();
    cx.simulate_mouse_move(stairs, None, gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        focus(&rig, cx),
        Focus::Fade("minecraft:oak_stairs".into()),
        "the hovered tile's kind"
    );
    let away = cx.debug_bounds("studio-viewport").unwrap().center();
    cx.simulate_mouse_move(away, None, gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(focus(&rig, cx), stone(), "back to the chosen kind");

    click(cx, "studio-detail-only");
    assert_eq!(focus(&rig, cx), Focus::Only("minecraft:stone".into()));
    click(cx, "studio-detail-only");
    assert_eq!(focus(&rig, cx), stone());
    click(cx, "studio-detail-only");
    click(cx, "studio-detail-close");
    assert_eq!(focus(&rig, cx), Focus::All, "every block again");
}

/// The tiles are the file's block kinds, most first, filtered by name.
#[gpui::test]
fn the_tiles_follow_the_search_by_name_or_id(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    let mut names = lumilio_core::BlockNames::default();
    names.add(
        "zh_cn",
        [("block.minecraft.stone".to_owned(), "石头".to_owned())].into(),
    );
    rig.answer(Event::Names(Ok(names)), cx);
    let search = rig
        .studio
        .read_with(cx, |studio, _| studio.block_search.clone().unwrap());
    for (query, stone, stairs) in [
        ("石头", true, false),
        ("oak", false, true),
        ("", true, true),
    ] {
        cx.update(|window, cx| search.update(cx, |input, cx| input.set_value(query, window, cx)));
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("studio-tile-minecraft:stone").is_some(),
            stone,
            "{query:?}"
        );
        assert_eq!(
            cx.debug_bounds("studio-tile-minecraft:oak_stairs")
                .is_some(),
            stairs,
            "{query:?}"
        );
    }
    click(cx, "studio-tile-minecraft:stone");
    rig.studio.read_with(cx, |studio, _| {
        assert_eq!(studio.chosen.as_deref(), Some("minecraft:stone"));
    });
}

/// T25: with a box selected in the picture, 材料 counts only its blocks
/// under a line giving its size, 清除选区 brings the whole file back, and
/// 替换方块… starts limited to the box.
#[gpui::test]
fn a_selection_scopes_the_materials_and_the_replacement(cx: &mut TestAppContext) {
    use lumilio_schematic::replace::Scope;
    let (rig, cx) = open_b(cx);
    rig.answer(Event::CanWrite(true), cx);
    assert!(cx.debug_bounds("studio-scope").is_none(), "the whole file");
    assert!(cx.debug_bounds("studio-tile-minecraft:stone").is_some());
    let viewer = viewer(&rig, cx);
    let cells = ([1, 0, 0], [2, 1, 0]);
    viewer.update(cx, |viewer, cx| {
        viewer.selected_for_test(cells, vec![("minecraft:oak_stairs".into(), 2)], cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("studio-scope").is_some(), "the box's line");
    assert!(
        cx.debug_bounds("studio-tile-minecraft:oak_stairs")
            .is_some()
    );
    assert!(
        cx.debug_bounds("studio-tile-minecraft:stone").is_none(),
        "only what the box holds"
    );

    rig.studio
        .update_in(cx, |studio, window, cx| studio.open_replace(window, cx));
    cx.run_until_parked();
    let dialog = rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Replace(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the replace dialog is open"),
    });
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
    let plan = dialog
        .read_with(cx, |dialog, cx| dialog.plan(cx))
        .expect("complete");
    assert_eq!(
        plan.scope,
        Scope::Cells {
            min: cells.0,
            max: cells.1
        },
        "the replacement starts limited to the box"
    );
    click(cx, "studio-dialog-cancel");

    click(cx, "studio-scope-clear");
    assert!(cx.debug_bounds("studio-scope").is_none());
    assert!(cx.debug_bounds("studio-tile-minecraft:stone").is_some());
    viewer.read_with(cx, |viewer, _| assert_eq!(viewer.selection(), None));
}

/// T24: the inspect card's 替换… starts the replace dialog on that block's
/// own state, each property matched exactly.
#[gpui::test]
fn the_inspect_card_replaces_the_inspected_state(cx: &mut TestAppContext) {
    let (rig, cx) = open_b(cx);
    rig.answer(Event::CanWrite(true), cx);
    viewer(&rig, cx).update(cx, |viewer, cx| viewer.set_drawn_for_test(cx));
    inspect(&rig, cx, None);
    click(cx, "studio-inspect-replace");
    let dialog = rig.studio.read_with(cx, |studio, _| match &studio.dialog {
        Some(OpenDialog::Replace(dialog)) => dialog.upgrade().expect("open"),
        _ => panic!("the replace dialog is open"),
    });
    let (any, chips) = dialog.read_with(cx, |dialog, _| dialog.chips()).remove(0);
    assert!(!any, "exact");
    assert!(chips.contains(&"facing=north".to_owned()), "{chips:?}");
}
