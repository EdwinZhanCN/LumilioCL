use std::collections::BTreeSet;
use std::rc::Rc;

use gpui::{Context, IntoElement, Render, Window};

use super::activity_actions::Activity;
use super::view::{ActivityCtx, activity};
use crate::kit::{self, Emit, ViewIntent};
use crate::live::{ActivityRow, LiveHandler, TrafficReading};
use crate::theme::ShellColors;

/// What the page shows of the rest of the launcher, as the shell knows it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActivityInputs {
    pub activity: Vec<ActivityRow>,
    pub traffic: TrafficReading,
    /// The games still in the library: a task about one of them opens it.
    pub games: BTreeSet<String>,
}

/// The page's tab and the tasks whose files are open.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActivityView {
    pub tab: usize,
    /// Running tasks whose downloads the page shows, by task id.
    pub open: BTreeSet<u64>,
}

impl ActivityView {
    pub(super) fn apply(&mut self, intent: ViewIntent) {
        match intent {
            ViewIntent::ActivityTab(tab) => self.tab = tab,
            ViewIntent::TaskDetail(task) => {
                let was_open = self.open.remove(&task);
                if !was_open {
                    self.open.insert(task);
                }
            }
            _ => {}
        }
    }
}

impl Activity {
    /// Where the page sends what other features answer: the shell's handler.
    pub fn set_handler(&mut self, handler: LiveHandler) {
        self.handler = Some(handler);
    }

    /// What the page shows, as the shell knows it.
    pub fn set_inputs(&mut self, inputs: ActivityInputs, cx: &mut Context<Self>) {
        if self.inputs != inputs {
            self.inputs = inputs;
            cx.notify();
        }
    }

    #[must_use]
    pub fn view(&self) -> &ActivityView {
        &self.view
    }

    /// Opens a tab directly, for design review and tests.
    pub fn show_tab(&mut self, tab: usize, cx: &mut Context<Self>) {
        self.view.tab = tab;
        cx.notify();
    }

    /// Applies a choice made on the page: a tab, or a task's files opened or
    /// closed.
    pub fn apply_view_intent(&mut self, intent: ViewIntent, cx: &mut Context<Self>) {
        self.view.apply(intent);
        cx.notify();
    }
}

impl Render for Activity {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let emit: Emit = Rc::new(move |intent, _, cx| {
            let _ = this.update(cx, |activity, cx| activity.apply_view_intent(intent, cx));
        });
        let ctx = ActivityCtx {
            colors: ShellColors::current(cx),
            inputs: &self.inputs,
            view: &self.view,
            emit,
            handler: self
                .handler
                .clone()
                .unwrap_or_else(|| Rc::new(|_, _, _| {})),
        };
        kit::page("live-activity", activity(&ctx)).into_any_element()
    }
}
