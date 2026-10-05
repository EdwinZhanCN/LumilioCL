use super::super::intent::{InstanceIntent, Section};
use super::super::panels::Arrived;
use super::super::{InstanceDetailView, TAB_DIAGNOSTICS, TAB_HISTORY};
use super::{click, record, rooted};
use gpui::{Entity, Modifiers, TestAppContext};
use lumilio_core::{LauncherSettings, PluginFinding};
use std::cell::RefCell;
use std::rc::Rc;

fn finding() -> PluginFinding {
    PluginFinding {
        plugin: "test.analyzer".into(),
        finding: lumilio_plugin_api::Finding {
            rule: "memory".into(),
            severity: lumilio_core::Severity::Error,
            title: "插件提供的标题".into(),
            advice: "插件提供的建议".into(),
            evidence: Some("OOM".into()),
        },
    }
}

#[gpui::test]
fn disabling_a_plugin_removes_cached_and_late_analysis_but_keeps_builtin_problems(
    cx: &mut TestAppContext,
) {
    let (view, cx) = rooted(cx, Rc::default());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.plugins_changed(vec!["test.analyzer".into()], cx);
        view.crash = Some((
            "crash.txt".into(),
            Some(Ok(("raw report".into(), vec![finding()]))),
        ));
        view.arrived(
            Arrived::Problems(Ok(vec![
                lumilio_core::Problem {
                    severity: lumilio_core::Severity::Error,
                    kind: lumilio_core::ProblemKind::Finding(finding()),
                },
                lumilio_core::Problem {
                    severity: lumilio_core::Severity::Error,
                    kind: lumilio_core::ProblemKind::NoJava { required: Some(21) },
                },
            ])),
            cx,
        );
        view.plugins_changed(Vec::new(), cx);
    });
    view.read_with(cx, |view, _| {
        assert_eq!(
            view.data.problems.as_ref().unwrap().as_ref().unwrap().len(),
            1
        );
        let (_, Some(Ok((text, findings)))) = view.crash.as_ref().unwrap() else {
            panic!()
        };
        assert_eq!(text, "raw report");
        assert!(findings.is_empty());
    });
    view.update(cx, |view, cx| {
        view.crash_arrived(
            "crash.txt".into(),
            Ok(("late report".into(), vec![finding()])),
            cx,
        );
        view.arrived(
            Arrived::Problems(Ok(vec![lumilio_core::Problem {
                severity: lumilio_core::Severity::Error,
                kind: lumilio_core::ProblemKind::Finding(finding()),
            }])),
            cx,
        );
    });
    view.read_with(cx, |view, _| {
        assert!(
            view.data
                .problems
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .is_empty()
        );
        assert!(matches!(&view.crash, Some((_, Some(Ok((_, findings))))) if findings.is_empty()));
    });
    view.update(cx, |view, cx| {
        view.plugins_changed(vec!["test.analyzer".into()], cx);
        view.crash_arrived(
            "crash.txt".into(),
            Ok(("report".into(), vec![finding()])),
            cx,
        );
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: None,
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("crash-finding-0").is_some(),
        "plugin data renders through generic report UI"
    );
}

#[gpui::test]
fn a_crashed_session_leads_to_the_log_and_a_clean_one_does_not(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let session = |started, outcome| lumilio_core::HistoryEvent::Session {
        started,
        seconds: 60,
        exit_code: None,
        outcome,
    };
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_HISTORY, cx);
        view.history_sub = 1;
        view.arrived(
            Arrived::History(Ok(lumilio_core::HistoryRead {
                events: vec![
                    session(100, lumilio_core::SessionOutcome::Clean),
                    session(200, lumilio_core::SessionOutcome::Crashed),
                ],
                skipped: 0,
            })),
            cx,
        );
    });
    cx.run_until_parked();
    click(cx, "session-inspect");
    view.read_with(cx, |view, _| {
        assert_eq!(view.tab, TAB_DIAGNOSTICS);
        assert_eq!(view.diag_sub, 1);
    });
    assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|intent| matches!(intent, InstanceIntent::OpenCrash(_))),
        "the reports are not known yet"
    );

    // The list arrives: the report written during that session opens.
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: None,
                crashes: vec![
                    lumilio_core::CrashReport {
                        file_name: "elsewhen.txt".into(),
                        modified: 9000,
                    },
                    lumilio_core::CrashReport {
                        file_name: "crash-2.txt".into(),
                        modified: 262,
                    },
                ],
            })),
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenCrash("crash-2.txt".into()))
    );
}

#[gpui::test]
fn the_log_tab_loads_once_and_shows_only_the_report_last_asked_for(cx: &mut TestAppContext) {
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
        view.diag_sub = 1;
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx)));
    assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some("[main] Done".into()),
                crashes: vec![lumilio_core::CrashReport {
                    file_name: "crash-1.txt".into(),
                    modified: 5,
                }],
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let open = cx.debug_bounds("crash-open").expect("view button");
    cx.simulate_click(open.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenCrash("crash-1.txt".into()))
    );
    view.update(cx, |view, cx| {
        view.crash_arrived("crash-0.txt".into(), Ok(("old".into(), Vec::new())), cx);
    });
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.crash, Some((file, None)) if file == "crash-1.txt"));
    });
    view.update(cx, |view, cx| {
        view.crash_arrived(
            "crash-1.txt".into(),
            Ok(("OOM".into(), vec![finding()])),
            cx,
        );
    });
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.crash, Some((_, Some(Ok((text, _))))) if text == "OOM"));
    });
    cx.run_until_parked();
    let export = cx.debug_bounds("instance-log-export").expect("export log");
    cx.simulate_click(export.center(), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ExportLog(None)));
    let export = cx.debug_bounds("crash-export").expect("export report");
    cx.simulate_click(export.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::ExportLog(Some("crash-1.txt".into())))
    );
}

#[gpui::test]
fn the_diagnostics_tab_loads_the_part_it_shows_and_lists_one_folder_at_a_time(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx)));
    assert!(
        seen.borrow()
            .contains(&InstanceIntent::Load(Section::Problems)),
        "问题 is the first part"
    );
    assert!(
        !seen
            .borrow()
            .contains(&InstanceIntent::Load(Section::Files))
    );

    view.update(cx, |view, _| view.diag_sub = 2);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_diagnostics(window, cx));
        view.update(cx, |view, cx| view.open_diagnostics(window, cx));
    });
    let loads = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
        seen.borrow()
            .iter()
            .filter(|intent| **intent == InstanceIntent::Load(Section::Files))
            .count()
    };
    assert_eq!(loads(&seen), 1, "asked once while the answer is on its way");

    let entry = |name: &str, is_dir: bool| lumilio_core::FileEntry {
        name: name.into(),
        is_dir,
        size: 12,
        modified: 5,
    };
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Files(
                String::new(),
                Ok(vec![entry("config", true), entry("options.txt", false)]),
            ),
            cx,
        )
    });
    click(cx, "file-open-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenFolder("config".into()))
    );
    let count = seen.borrow().len();
    // The answer has not come: a second click does not ask again.
    click(cx, "file-open-0");
    assert_eq!(seen.borrow().len(), count);

    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Files("config".into(), Ok(vec![entry("sodium.json", false)])),
            cx,
        )
    });
    click(cx, "file-reveal-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::RevealPath("config/sodium.json".into()))
    );
}

/// Gpui hands a wheel event to every scrollable under the pointer, so a
/// log that scrolls inside a page that scrolls moved both at once.
#[gpui::test]
fn scrolling_the_log_leaves_the_page_where_it_is(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

    let (view, cx) = rooted(cx, Rc::default());
    let text: String = (0..300)
        .map(|n| format!("[{n}] [main/INFO]: line {n}\n"))
        .collect();
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some(text),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.simulate_resize(gpui::size(px(1080.), px(500.)));
    cx.run_until_parked();

    let wheel = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
    };
    let lines = cx.debug_bounds("instance-log-lines").expect("log lines");
    let at = point(lines.center().x, lines.top() + px(40.));
    wheel(cx, at);
    let after = cx.debug_bounds("instance-log-lines").expect("log lines");
    assert_eq!(
        after.origin.y, lines.origin.y,
        "the page stayed put while the log scrolled"
    );
    let scrolled = view.read_with(cx, |view, _| view.log_scroll.offset().y);
    assert!(scrolled < px(0.), "the log itself scrolled ({scrolled:?})");
}

#[gpui::test]
fn the_log_view_filters_by_level_and_search_and_keeps_stack_traces_with_their_line(
    cx: &mut TestAppContext,
) {
    let (view, cx) = rooted(cx, Rc::default());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some(
                    "[1] [main/INFO]: loaded sodium\n[2] [main/WARN]: slow tick\n[3] [main/ERROR]: boom\n\tat a.B(B.java)"
                        .into(),
                ),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
    };
    assert_eq!(shown(&view, cx).lines().count(), 4);
    view.update(cx, |view, cx| {
        view.log_level = 1;
        cx.notify();
    });
    assert_eq!(
        shown(&view, cx),
        "[3] [main/ERROR]: boom\n\tat a.B(B.java)",
        "the trace line belongs to the error"
    );
    view.update(cx, |view, _| view.log_level = 2);
    assert_eq!(shown(&view, cx).lines().count(), 3);
    view.update(cx, |view, _| view.log_level = 0);
    let search = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().log_search.clone()
    });
    cx.update(|window, cx| search.update(cx, |search, cx| search.set_value("SODIUM", window, cx)));
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: loaded sodium");
}

#[gpui::test]
fn the_running_games_output_replaces_the_log_file_until_it_stops(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some("[1] [main/INFO]: from the file".into()),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
    };
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");

    view.update(cx, |view, cx| {
        view.game_output(
            vec![
                "[2] [main/INFO]: live one".into(),
                "[3] [main/WARN]: live two".into(),
            ],
            true,
            cx,
        )
    });
    assert_eq!(
        shown(&view, cx),
        "[2] [main/INFO]: live one\n[3] [main/WARN]: live two"
    );
    // The level filter works on the live lines too.
    view.update(cx, |view, _| view.log_level = 2);
    assert_eq!(shown(&view, cx), "[3] [main/WARN]: live two");

    let before = seen
        .borrow()
        .iter()
        .filter(|i| **i == InstanceIntent::Load(Section::Logs))
        .count();
    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    let after = seen
        .borrow()
        .iter()
        .filter(|i| **i == InstanceIntent::Load(Section::Logs))
        .count();
    assert_eq!(
        after,
        before + 1,
        "the file is read again once the game ends"
    );
    view.update(cx, |view, _| view.log_level = 0);
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");
}
