use super::super::intent::{InstanceIntent, Section};
use super::super::panels::{Arrived, Confirm};
use super::super::{InstanceDetailView, Operated, TAB_WORLDS};
use super::{click, record, rooted, world};
use gpui::{Modifiers, TestAppContext};
use lumilio_core::LauncherSettings;
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn a_world_can_be_entered_directly_unless_the_version_is_too_old(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        InstanceDetailView::new(
            "survival".into(),
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    let enter = cx.debug_bounds("world-play-0").expect("enter button");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::PlayWorld("w".into()))
    );

    // An old game version cannot go straight in: the button does nothing.
    let mut old = record();
    old.game_version = "1.16.5".into();
    view.update(cx, |view, cx| {
        view.busy = false;
        view.loaded(Ok((old, LauncherSettings::default())), cx);
    });
    cx.run_until_parked();
    let before = seen.borrow().len();
    let enter = cx.debug_bounds("world-play-0").expect("enter button");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(seen.borrow().len(), before);
}

#[gpui::test]
fn the_world_list_is_searched_and_a_world_can_be_imported(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("world-play-1").is_some());

    let search = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().world_search.clone()
    });
    cx.update(|window, cx| search.update(cx, |search, cx| search.set_value("bet", window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("world-play-1").is_none(), "one world left");
    click(cx, "world-play-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::PlayWorld("beta".into()))
    );

    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    click(cx, "world-import");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ImportWorld));
    assert!(
        !view.read_with(cx, |view, _| view.busy),
        "choosing a file is not a write; cancelling it must not leave the page busy"
    );
}

#[gpui::test]
fn deleting_a_world_asks_first_and_never_double_submits(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    let writes = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
        seen.borrow()
            .iter()
            .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
            .cloned()
            .collect::<Vec<_>>()
    };
    let ask = cx.debug_bounds("world-delete-0").expect("delete button");
    cx.simulate_click(ask.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(writes(&seen).is_empty(), "the click only asks, in a dialog");
    view.read_with(cx, |view, _| {
        assert_eq!(view.confirm, Some(Confirm::DeleteWorld("w".into())));
    });
    // Answering yes (the dialog's button calls this) sends it once.
    let yes = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.confirmed(&Confirm::DeleteWorld("w".into()), window, cx)
            })
        });
        cx.run_until_parked();
    };
    yes(cx);
    assert_eq!(writes(&seen), vec![InstanceIntent::DeleteWorld("w".into())]);
    // While it runs, the world cannot be deleted again.
    view.read_with(cx, |view, _| assert!(view.busy));
    let sent = writes(&seen).len();
    yes(cx);
    assert_eq!(writes(&seen).len(), sent);
    // The answer re-reads only what was looked at (worlds, no snapshots).
    let stale = view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: "世界已删除".into(),
                technical: None,
                refresh: vec![Section::Worlds, Section::Snapshots],
            },
            cx,
        )
    });
    assert_eq!(stale, vec![Section::Worlds]);
    view.read_with(cx, |view, _| assert!(!view.busy));
}

#[gpui::test]
fn while_a_game_runs_the_worlds_cannot_be_touched_and_a_dropped_archive_is_one_write(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.game_started_ms.is_some()));
    let before = seen.borrow().len();
    click(cx, "world-import");
    click(cx, "world-play-0");
    assert_eq!(
        seen.borrow().len(),
        before,
        "nothing is sent for a world while the game runs"
    );

    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.import_dropped(
                &[
                    "/tmp/a.txt".into(),
                    "/tmp/w.zip".into(),
                    "/tmp/x.zip".into(),
                ],
                window,
                cx,
            )
        })
    });
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::AddWorld("/tmp/w.zip".into()))
    );
}
