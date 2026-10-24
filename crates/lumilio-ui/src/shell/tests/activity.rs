use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};

use super::super::{LauncherShell, Route};

#[gpui::test]
fn a_failed_task_offers_retry_and_open_and_a_game_that_is_gone_offers_no_open(
    cx: &mut TestAppContext,
) {
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader, RetryAction, TaskCategory};

    use crate::live::{ActivityRow, ActivityState, LiveIntent, library_card};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let record = InstanceRecord {
        id: "here".to_owned(),
        name: "在".to_owned(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Fabric,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings::default(),
        source_project: None,
    };
    let failed = |instance: &str| ActivityRow {
        task: None,
        amount: None,
        unit: lumilio_core::ProgressUnit::Items,
        rate: None,
        instance: Some(instance.to_owned()),
        retry: Some(RetryAction::RepairInstance {
            instance: instance.to_owned(),
        }),
        category: TaskCategory::Repair,
        title: "修复".into(),
        detail: String::new(),
        when: None,
        fraction: None,
        state: ActivityState::Failed("no route".into()),
        cancel: None,
        pause: None,
        paused: false,
        downloads: None,
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Activity);
        shell.update_live(
            |model| {
                model.set_library(vec![library_card(&record, 10)]);
                model.set_activity(vec![failed("here"), failed("gone")], 1_000);
            },
            cx,
        );
    });
    cx.run_until_parked();

    let retry = cx.debug_bounds("live-retry-0").expect("retry");
    cx.simulate_click(retry.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::RetryTask(RetryAction::RepairInstance {
            instance: "here".into()
        }))
    );
    let open = cx.debug_bounds("live-open-0").expect("open");
    cx.simulate_click(open.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&LiveIntent::OpenInstance("here".into()))
    );
    assert!(cx.debug_bounds("live-retry-1").is_some());
    assert!(
        cx.debug_bounds("live-open-1").is_none(),
        "a deleted game cannot be opened"
    );
}

/// A running task that downloads: its files open under it, its downloads
/// pause and resume from the row, and the toolbar reads the traffic.
#[gpui::test]
fn a_downloading_task_shows_its_files_and_pauses_from_its_row(cx: &mut TestAppContext) {
    use lumilio_core::{FileDownload, SourceSwitch, TaskCategory, TaskDownloads, Traffic};

    use crate::live::{ActivityRow, ActivityState, LiveIntent};

    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let (shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx).with_live(Rc::new(move |intent, _, _| {
            sink.borrow_mut().push(intent);
        }))
    });
    let running = |paused: bool| ActivityRow {
        task: Some(7),
        amount: Some((40, 100)),
        unit: lumilio_core::ProgressUnit::Bytes,
        rate: None,
        instance: None,
        retry: None,
        category: TaskCategory::Install,
        title: "安装".into(),
        detail: String::new(),
        when: None,
        fraction: Some(0.4),
        state: ActivityState::Running,
        cancel: Some(7),
        pause: Some(7),
        paused,
        downloads: Some(TaskDownloads {
            files: vec![FileDownload {
                id: "client".into(),
                name: "client.jar".into(),
                source: Some("https://piston-data.mojang.com/client.jar".into()),
                attempt: 2,
                connections: 4,
                done: 40,
                total: Some(100),
            }],
            switches: vec![SourceSwitch {
                file: "client.jar".into(),
                from: "https://mirror.test/client.jar".into(),
                reason: "HTTP status 404".into(),
                to: Some("https://piston-data.mojang.com/client.jar".into()),
            }],
            retries: 1,
        }),
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Activity);
        shell.update_live(
            |model| {
                model.set_activity(vec![running(false)], 1_000);
                model.set_traffic(
                    Traffic {
                        open: 3,
                        limit: 16,
                        received: 0,
                    },
                    1_000,
                );
            },
            cx,
        );
    });
    cx.run_until_parked();

    assert!(cx.debug_bounds("live-transfer-display").is_some());
    assert!(cx.debug_bounds("live-task-files-0").is_none());
    let detail = cx.debug_bounds("live-task-detail-0").expect("details key");
    cx.simulate_click(detail.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("live-task-files-0").is_some());

    let pause = cx.debug_bounds("live-pause-0").expect("pause key");
    cx.simulate_click(pause.center(), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&LiveIntent::PauseTask(7)));

    shell.update(cx, |shell, cx| {
        shell.update_live(|model| model.set_activity(vec![running(true)], 2_000), cx);
    });
    cx.run_until_parked();
    let resume = cx.debug_bounds("live-pause-0").expect("resume key");
    cx.simulate_click(resume.center(), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&LiveIntent::ResumeTask(7)));
    assert!(
        cx.debug_bounds("live-task-files-0").is_some(),
        "the detail stays open across readings"
    );
}
