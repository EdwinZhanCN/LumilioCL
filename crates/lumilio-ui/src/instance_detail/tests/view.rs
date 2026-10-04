use super::super::editors::Editor;
use super::super::intent::{InstanceIntent, Section};
use super::super::panels::Arrived;
use super::super::{InstanceDetailView, TAB_HISTORY, TAB_OVERVIEW, TAB_WORLDS};
use super::{click, record, rooted, world};
use gpui::TestAppContext;
use gpui::prelude::*;
use lumilio_core::LauncherSettings;
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn tabs_load_their_data_once_and_ignore_a_second_request_while_pending(cx: &mut TestAppContext) {
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
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            InstanceIntent::PluginTabs,
            InstanceIntent::Load(Section::Problems),
            InstanceIntent::Load(Section::Size),
            InstanceIntent::Load(Section::History),
        ],
        "the page asks which plugin tabs show and, for the overview, what it shows, once"
    );
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_tab(TAB_WORLDS, window, cx);
            view.open_tab(TAB_OVERVIEW, window, cx);
            view.open_tab(TAB_WORLDS, window, cx);
        })
    });
    let loads = seen
        .borrow()
        .iter()
        .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
        .count();
    assert_eq!(loads, 1, "a pending section is not requested again");
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_tab(TAB_OVERVIEW, window, cx);
            view.open_tab(TAB_WORLDS, window, cx);
        })
    });
    let loads = seen
        .borrow()
        .iter()
        .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
        .count();
    assert_eq!(loads, 1, "a loaded section is shown, not re-read");
}

#[gpui::test]
fn the_header_starts_the_game_and_becomes_a_stop_button_while_it_runs(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    click(cx, "instance-play");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Play));
    assert!(cx.debug_bounds("instance-stop").is_none());

    view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("instance-play").is_none());
    click(cx, "instance-stop");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Stop));

    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("instance-play").is_some());
}

#[gpui::test]
fn the_overview_sizes_the_game_lists_recent_things_and_acts_on_a_problem(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    for section in [Section::Problems, Section::Size, Section::History] {
        assert!(
            seen.borrow().contains(&InstanceIntent::Load(section)),
            "the overview asks for {section:?}"
        );
    }
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Size(Ok(3 * 1024 * 1024)), cx);
        view.arrived(
            Arrived::Problems(Ok(vec![lumilio_core::Problem {
                severity: lumilio_core::Severity::Error,
                kind: lumilio_core::ProblemKind::DamagedFiles { count: 2 },
            }])),
            cx,
        );
        view.arrived(
            Arrived::History(Ok(lumilio_core::HistoryRead {
                events: vec![lumilio_core::HistoryEvent::Session {
                    started: 5,
                    seconds: 90,
                    exit_code: None,
                    outcome: lumilio_core::SessionOutcome::Clean,
                }],
                skipped: 0,
            })),
            cx,
        );
    });
    cx.run_until_parked();
    click(cx, "problem-action-0");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Repair));

    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    click(cx, "overview-all-sessions");
    view.read_with(cx, |view, _| {
        assert_eq!(view.tab, TAB_HISTORY);
        assert_eq!(view.history_sub, 1, "straight to the play records");
    });
}

#[gpui::test]
fn a_question_asked_before_the_game_is_loaded_waits_for_it(cx: &mut TestAppContext) {
    let (view, cx) = rooted(cx, Rc::default());
    view.update(cx, |view, cx| view.ask_later(InstanceIntent::AskCopy, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.editor.is_none(), "nothing to copy yet");
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.editor, Some(Editor::Copy));
        assert!(view.ask_later.is_none(), "asked once");
    });
}

#[gpui::test]
fn a_response_for_another_instance_cannot_replace_the_view(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let view = cx.new(|_| InstanceDetailView::new("survival".into(), Rc::new(|_, _, _| {})));
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    let mut other = record();
    other.id = "other".into();
    other.name = "另一个游戏".into();
    view.update(cx, |view, cx| {
        view.loaded(Ok((other, LauncherSettings::default())), cx)
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.title().to_owned()),
        "生存"
    );
}
