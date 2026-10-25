//! When the shell feeds its pages: once it follows the launcher, then each
//! time a feature says the lists changed or the person asks to refresh.

use gpui::{Context, Subscription};

use super::{Feed, check_again, follow, reload};
use crate::features::activity::ActivityEvent;
use crate::features::discover::DiscoverEvent;
use crate::features::game::GameEvent;
use crate::features::launch::LaunchEvent;
use crate::features::library::LibraryEvent;
use crate::features::settings::SettingsEvent;
use crate::shell::LauncherShell;

impl LauncherShell {
    /// Keeps the pages up to date with the launcher from now on: what core
    /// publishes about the current game and account, and the lists.
    pub fn follow_launcher(&mut self, cx: &mut Context<Self>) {
        let Some(worker) = self.current_choices.worker.clone() else {
            return;
        };
        let feed = Feed::new(self.this.clone(), worker);
        self.feed = Some(feed.clone());
        cx.defer(move |cx| {
            follow(&feed, cx);
            reload(&feed, cx);
        });
    }

    /// Reads the lists again. Deferred: features tell the shell inside its
    /// own updates.
    pub(in crate::shell) fn reload_lists(&self, cx: &mut Context<Self>) {
        if let Some(feed) = self.feed.clone() {
            cx.defer(move |cx| reload(&feed, cx));
        }
    }

    /// The person asked to refresh: the lists, and every game's checks.
    pub(in crate::shell) fn refresh(&self, cx: &mut Context<Self>) {
        if let Some(feed) = self.feed.clone() {
            cx.defer(move |cx| {
                reload(&feed, cx);
                check_again(&feed, cx);
            });
        }
    }

    /// What a task that ran again may have changed in the games: Discover
    /// reads what they have installed again.
    fn installed_changed(&self, cx: &mut Context<Self>) {
        if let Some(feed) = self.feed.clone() {
            cx.defer(move |cx| super::lists::installed_changed(&feed, cx));
        }
    }

    /// Reads the lists again whenever a feature says they changed.
    pub(in crate::shell) fn follow_lists(&self, cx: &mut Context<Self>) -> Vec<Subscription> {
        vec![
            cx.subscribe(&self.discover, |shell, _, event: &DiscoverEvent, cx| {
                if matches!(event, DiscoverEvent::TasksChanged) {
                    shell.reload_lists(cx);
                }
            }),
            cx.subscribe(&self.library, |shell, _, event: &LibraryEvent, cx| {
                if *event == LibraryEvent::ListsChanged {
                    shell.reload_lists(cx);
                }
            }),
            cx.subscribe(
                &self.activity_page,
                |shell, _, event: &ActivityEvent, cx| match event {
                    ActivityEvent::ListsChanged => shell.reload_lists(cx),
                    ActivityEvent::Retried => shell.installed_changed(cx),
                },
            ),
            cx.subscribe(
                &self.settings_page,
                |shell, _, event: &SettingsEvent, cx| {
                    if matches!(event, SettingsEvent::TasksChanged) {
                        shell.reload_lists(cx);
                    }
                },
            ),
            cx.subscribe(&self.game, |shell, _, event: &GameEvent, cx| {
                if *event == GameEvent::ListsChanged {
                    shell.reload_lists(cx);
                }
            }),
            cx.subscribe(&self.launch, |shell, _, event: &LaunchEvent, cx| {
                if *event == LaunchEvent::Ended {
                    shell.reload_lists(cx);
                }
            }),
        ]
    }
}
