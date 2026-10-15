//! 资源来源 under 信息 › 技术信息 (T17, T21, D27): the sources the viewport's
//! assets came from, with what each provides; each mod and pack has a
//! switch, and switching one or taking an added pack off reloads the
//! viewport.

use std::path::PathBuf;

use gpui::TestAppContext;
use lumilio_core::{AssetPlan, ModelPreview, ProvidedSource, SourceRole};

use super::{A, B, choose, click, fixture, rig};
use crate::schematic_studio::sources::source_words;
use crate::schematic_studio::{Command, Event, InspectorTab};

fn source(role: SourceRole, blocks: u32, assets: u32) -> ProvidedSource {
    ProvidedSource {
        enabled: matches!(role, SourceRole::Base { .. }),
        role,
        blocks,
        assets,
        custom_models: 0,
    }
}

fn sources() -> Vec<ProvidedSource> {
    vec![
        source(
            SourceRole::Base {
                version: "1.20.1".into(),
            },
            1000,
            4000,
        ),
        source(SourceRole::Mod("/game/mods/create.jar".into()), 120, 900),
        source(
            SourceRole::ResourcePack("/game/resourcepacks/Faithful.zip".into()),
            0,
            300,
        ),
        source(
            SourceRole::Added {
                path: "/packs/Extra.zip".into(),
                missing: false,
            },
            1,
            2,
        ),
    ]
}

#[test]
fn each_source_is_named_with_what_it_provides() {
    let words: Vec<(String, String)> = sources().iter().map(source_words).collect();
    assert_eq!(
        words,
        [
            ("1.20.1 · 下载的 bundle".into(), "1000 个方块".into()),
            ("create.jar".into(), "120 个方块".into()),
            ("Faithful.zip".into(), "300 个资源文件".into()),
            ("Extra.zip".into(), "1 个方块".into()),
        ]
    );
    let gone = source(
        SourceRole::Added {
            path: "/packs/Gone.zip".into(),
            missing: true,
        },
        0,
        0,
    );
    assert_eq!(source_words(&gone).1, "找不到文件");
    let machines = ProvidedSource {
        custom_models: 8,
        ..source(SourceRole::Mod("/game/mods/machines.jar".into()), 40, 300)
    };
    assert_eq!(source_words(&machines).1, "40 个方块 · 8 个模型画不出");
}

/// Whether row `index`'s switch is drawn on: its cap sits at the trailing
/// end.
fn switched_on(cx: &mut gpui::VisualTestContext, index: usize) -> bool {
    let mut at = |selector: String| {
        cx.debug_bounds(Box::leak(selector.into_boxed_str()))
            .unwrap_or_else(|| panic!("row {index} has a switch"))
    };
    let switch = at(format!("studio-source-switch-{index}"));
    let cap = at(format!("studio-source-switch-{index}-cap"));
    cap.left() > switch.left() + gpui::px(10.)
}

/// The rig showing technical details with `B`'s preview naming `provided`;
/// returns the preview's request.
fn showing(
    rig: &super::Rig,
    provided: Vec<ProvidedSource>,
    cx: &mut gpui::VisualTestContext,
) -> u64 {
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    rig.serve(&files, cx);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::Info;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "studio-technical");
    choose(cx, B);
    rig.serve(&files, cx);
    let request = rig.models.borrow().last().unwrap().1;
    let preview = ModelPreview {
        schematic: files[1].1.clone(),
        pack: None,
        plan: AssetPlan {
            provided,
            ..AssetPlan::default()
        },
        entities: Default::default(),
    };
    rig.studio.update(cx, |studio, cx| {
        assert!(studio.model_arrived(request, Ok(preview), cx));
    });
    cx.run_until_parked();
    request
}

/// T21: every mod and pack row has a switch, drawn as the plan says. A
/// flip shows at once, asks for the switch to be kept and, once it is,
/// loads the viewport again while the list stays; one that is not kept
/// flips back with a toast.
#[gpui::test]
fn each_mod_and_pack_has_a_switch_and_flipping_one_reloads(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    let request = showing(&rig, sources(), cx);
    assert!(
        cx.debug_bounds("studio-source-switch-0").is_none(),
        "the base is always drawn with"
    );
    for row in 1..4 {
        assert!(!switched_on(cx, row), "row {row} is off by default");
    }

    rig.sent();
    let old = rig.viewer(cx).unwrap();
    click(cx, "studio-source-switch-1");
    assert_eq!(
        rig.sent(),
        [Command::SetSource {
            path: PathBuf::from("/game/mods/create.jar"),
            on: true
        }]
    );
    assert!(switched_on(cx, 1), "the flip shows at once");
    rig.answer(
        Event::SourceSwitched {
            path: "/game/mods/create.jar".into(),
            on: true,
            result: Ok(()),
        },
        cx,
    );
    assert!(old.upgrade().is_none(), "the viewport loads again");
    assert_ne!(rig.models.borrow().last().unwrap().1, request);
    assert!(switched_on(cx, 1), "the list stays until the new load");

    click(cx, "studio-source-switch-2");
    assert!(switched_on(cx, 2));
    let kept = rig.viewer(cx).unwrap();
    rig.studio.update(cx, |studio, cx| {
        studio.source_switched(
            std::path::Path::new("/game/resourcepacks/Faithful.zip"),
            true,
            Err("disk full".into()),
            cx,
        );
        // Read before a frame hands the toast to the window.
        assert_eq!(studio.toasts.len(), 1, "a toast says nothing changed");
    });
    cx.run_until_parked();
    assert!(!switched_on(cx, 2), "not kept: it flips back");
    assert!(kept.upgrade().is_some(), "and the viewport stays");
}

/// A studio opened again shows each switch as the launcher kept it: the
/// switches live in launcher data, and the plan carries them.
#[gpui::test]
fn a_reopened_studio_shows_the_kept_switches(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    let mut kept = sources();
    kept[1].enabled = true;
    showing(&rig, kept, cx);
    assert!(switched_on(cx, 1));
    assert!(!switched_on(cx, 2));
}

#[gpui::test]
fn technical_details_list_the_sources_and_taking_a_pack_off_reloads(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    let files = [(A, fixture("A", 3)), (B, fixture("B", 5))];
    rig.answer(Event::Listed(Ok(vec![A.into(), B.into()])), cx);
    rig.serve(&files, cx);
    rig.studio.update(cx, |studio, cx| {
        studio.tab = InspectorTab::Info;
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("studio-sources-add").is_none(),
        "技术信息 is folded until asked for (D27)"
    );
    click(cx, "studio-technical");
    assert!(cx.debug_bounds("studio-sources-add").is_some());
    choose(cx, B);
    rig.serve(&files, cx);
    assert!(
        cx.debug_bounds("studio-source-0").is_none(),
        "not loaded yet"
    );

    let request = rig.models.borrow().last().unwrap().1;
    let preview = ModelPreview {
        schematic: files[1].1.clone(),
        pack: None,
        plan: AssetPlan {
            provided: sources(),
            ..AssetPlan::default()
        },
        entities: Default::default(),
    };
    rig.studio.update(cx, |studio, cx| {
        assert!(studio.model_arrived(request, Ok(preview), cx));
    });
    cx.run_until_parked();
    for row in 0..4 {
        assert!(
            cx.debug_bounds(Box::leak(format!("studio-source-{row}").into_boxed_str()))
                .is_some(),
            "row {row}"
        );
    }
    assert!(
        cx.debug_bounds("studio-source-remove-1").is_none(),
        "only an added pack can be taken off"
    );

    rig.sent();
    let old = rig.viewer(cx).unwrap();
    click(cx, "studio-source-remove-3");
    assert_eq!(
        rig.sent(),
        [Command::RemovePack {
            path: PathBuf::from("/packs/Extra.zip")
        }]
    );
    rig.answer(Event::PacksChanged(Ok(())), cx);
    assert!(old.upgrade().is_none(), "the viewport loads again");
    assert_ne!(rig.models.borrow().last().unwrap().1, request);
    assert!(cx.debug_bounds("studio-source-0").is_none());

    // A refused change keeps the viewport.
    let kept = rig.viewer(cx).unwrap();
    rig.answer(Event::PacksChanged(Err("not a zip".into())), cx);
    assert!(kept.upgrade().is_some());
}

/// T36: each mod and pack row moves past its neighbour in the list; the
/// first cannot go up nor the last down. A kept order loads the viewport
/// again; one that is not kept says so and keeps it.
#[gpui::test]
fn a_source_moves_past_its_neighbour_and_reloads(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    let request = showing(&rig, sources(), cx);
    assert!(
        cx.debug_bounds("studio-source-up-0").is_none(),
        "the base stays first"
    );
    rig.sent();
    click(cx, "studio-source-up-1");
    assert!(rig.sent().is_empty(), "the lowest mod or pack cannot go up");
    click(cx, "studio-source-down-3");
    assert!(rig.sent().is_empty(), "the highest cannot go down");

    let old = rig.viewer(cx).unwrap();
    click(cx, "studio-source-down-1");
    assert_eq!(
        rig.sent(),
        [Command::MoveSource {
            path: PathBuf::from("/game/mods/create.jar"),
            past: PathBuf::from("/game/resourcepacks/Faithful.zip"),
        }]
    );
    rig.answer(Event::SourceMoved(Ok(true)), cx);
    assert!(old.upgrade().is_none(), "the viewport loads again");
    assert_ne!(rig.models.borrow().last().unwrap().1, request);

    let kept = rig.viewer(cx).unwrap();
    rig.answer(Event::SourceMoved(Err("disk full".into())), cx);
    assert!(kept.upgrade().is_some());
}

/// T36: a path typed into the field is added like a picked one.
#[gpui::test]
fn a_typed_path_is_added(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx);
    showing(&rig, sources(), cx);
    let input = rig
        .studio
        .read_with(cx, |studio, _| studio.pack_path.clone())
        .expect("the path field");
    cx.update(|window, cx| {
        input.update(cx, |input, cx| {
            input.set_value("\"/packs/Redstone Display\"", window, cx);
        });
    });
    cx.run_until_parked();
    rig.sent();
    click(cx, "studio-sources-path-add");
    assert_eq!(
        rig.sent(),
        [Command::AddPack {
            path: PathBuf::from("/packs/Redstone Display")
        }]
    );
    assert!(input.read_with(cx, |input, _| input.value().is_empty()));
}
