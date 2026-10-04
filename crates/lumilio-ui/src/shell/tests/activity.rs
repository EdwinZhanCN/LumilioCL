use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn a_failed_task_offers_retry_and_open_and_a_game_that_is_gone_offers_no_open(
    cx: &mut TestAppContext,
) {
    use crate::live::{ActivityRow, ActivityState, LiveIntent, library_card};
    use lumilio_core::{InstanceRecord, InstanceSettings, Loader, RetryAction, TaskCategory};

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
        fraction: None,
        state: ActivityState::Failed("no route".into()),
        cancel: None,
    };
    shell.update(cx, |shell, cx| {
        shell.show(Route::Activity);
        shell.update_live(
            |model| {
                model.set_library(vec![library_card(&record, 10)], None);
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
