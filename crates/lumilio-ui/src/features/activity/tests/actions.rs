//! Activity's actions with a real launcher behind them, on its own runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::RetryAction;

use super::super::{Activity, ActivityEvent};
use crate::testing::{Launcher, launcher, toasts, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

/// Activity in a window, with what it told the application so far.
fn activity<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (
    Entity<Activity>,
    Rc<RefCell<Vec<ActivityEvent>>>,
    &'a mut VisualTestContext,
) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let activity = cx.new(|_| Activity::new(Some(worker)));
    let events: Rc<RefCell<Vec<ActivityEvent>>> = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&activity, move |_, event: &ActivityEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (activity, events, cx)
}

#[gpui::test]
fn a_retry_is_told_when_it_starts_and_ends_and_the_application_hears_of_it(
    cx: &mut TestAppContext,
) {
    let launcher = launcher(Vec::new());
    let (activity, events, cx) = activity(&launcher, cx);
    activity.update(cx, |activity, cx| {
        activity.retry(
            RetryAction::RepairInstance {
                instance: "gone".into(),
            },
            cx,
        );
    });
    until(cx, "the retry to end", |_| {
        events.borrow().contains(&ActivityEvent::Retried)
    });

    assert_eq!(toasts(cx), ["已重新开始，进度在动态里", "还是没有成功"]);
    let events = events.borrow();
    assert_eq!(
        events.last(),
        Some(&ActivityEvent::ListsChanged),
        "the lists are read again once it ended"
    );
}

#[gpui::test]
fn clearing_the_ended_tasks_reads_the_lists_again_quietly(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (activity, events, cx) = activity(&launcher, cx);
    activity.update(cx, |activity, cx| activity.clear_finished(cx));
    until(cx, "the list to be cleared", |_| {
        !events.borrow().is_empty()
    });

    assert_eq!(events.borrow().as_slice(), [ActivityEvent::ListsChanged]);
    assert!(toasts(cx).is_empty(), "the shorter list says it");
}
