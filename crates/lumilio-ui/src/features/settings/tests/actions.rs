//! Settings' work with a real launcher behind it, on its own runtime.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Context, Entity, TestAppContext, VisualTestContext, Window, div};
use lumilio_core::{MotionPreference, Preferences};

use super::super::{Settings, SettingsEvent};
use crate::live::LiveIntent;
use crate::testing::{Launcher, launcher, toasts, until};

struct Host;

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

type Events = Rc<RefCell<Vec<SettingsEvent>>>;

/// Settings in a window, with what it told the shell and the application.
fn settings<'a>(
    launcher: &Launcher,
    cx: &'a mut TestAppContext,
) -> (Entity<Settings>, Events, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    cx.update(gpui_component::init);
    let (_, cx) = cx.add_window_view(|_, _| Host);
    let worker = launcher.worker.clone();
    let settings = cx.new(|_| Settings::new(Some(worker)));
    let events: Events = Rc::default();
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&settings, move |_, event: &SettingsEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    (settings, events, cx)
}

fn answer(settings: &Entity<Settings>, intent: LiveIntent, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        settings.update(cx, |settings, cx| settings.answer(intent, window, cx));
    });
}

fn saved(launcher: &Launcher) -> lumilio_core::LauncherSettings {
    let service = launcher.worker.service().clone();
    launcher
        .runtime
        .block_on(async move { service.settings().await })
}

#[gpui::test]
fn reading_the_settings_fills_the_page_and_tells_which_plugins_there_are(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (settings, events, cx) = settings(&launcher, cx);
    settings.update(cx, |settings, cx| settings.load(cx));
    until(cx, "the disk to be measured", |cx| {
        settings.read_with(cx, |settings, _| {
            settings
                .settings()
                .is_some_and(|view| view.storage.is_some())
        })
    });

    assert_eq!(
        events.borrow().as_slice(),
        [SettingsEvent::PluginsRead(Vec::new())]
    );
}

#[gpui::test]
fn saved_preferences_are_kept_shown_again_and_handed_to_the_application(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (settings, events, cx) = settings(&launcher, cx);
    let preferences = Preferences {
        motion: MotionPreference::Reduce,
        ..Preferences::default()
    };
    answer(
        &settings,
        LiveIntent::SetPreferences(preferences.clone()),
        cx,
    );
    until(cx, "the preferences to be saved and read again", |cx| {
        settings.read_with(cx, |settings, _| {
            settings
                .settings()
                .is_some_and(|view| view.preferences.motion == MotionPreference::Reduce)
        })
    });

    assert_eq!(saved(&launcher).preferences, preferences);
    let window = cx.update(|window, _| window.window_handle());
    assert_eq!(
        events.borrow().first(),
        Some(&SettingsEvent::PreferencesSaved {
            preferences: Box::new(preferences),
            window
        }),
        "the application applies them in this window"
    );
    assert!(toasts(cx).is_empty(), "the change shows by itself");
}

#[gpui::test]
fn the_updater_is_asked_for_a_check_and_told_when_it_is_turned_off(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (settings, events, cx) = settings(&launcher, cx);
    answer(&settings, LiveIntent::CheckUpdates, cx);
    answer(&settings, LiveIntent::SetAutoUpdate(false), cx);
    until(cx, "the switch to be saved", |_| {
        events.borrow().contains(&SettingsEvent::AutoUpdate(false))
    });

    assert_eq!(events.borrow()[0], SettingsEvent::CheckUpdates);
    assert!(!saved(&launcher).auto_update);
}

#[gpui::test]
fn a_setting_that_cannot_be_saved_is_told_and_kept_as_it_was(cx: &mut TestAppContext) {
    let launcher = launcher(Vec::new());
    let (settings, _, cx) = settings(&launcher, cx);
    answer(
        &settings,
        LiveIntent::SetMemory {
            min_mb: Some(8192),
            max_mb: Some(1024),
        },
        cx,
    );
    until(cx, "the failure to be told", |cx| !toasts(cx).is_empty());

    assert_eq!(toasts(cx), ["没能保存内存设置"]);
    assert_eq!(saved(&launcher).default_max_memory_mb, None);
}
