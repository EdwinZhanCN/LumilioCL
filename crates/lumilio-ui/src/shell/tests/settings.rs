use std::cell::RefCell;
use std::rc::Rc;

use gpui::{AppContext as _, Entity, Modifiers, TestAppContext};

use super::super::{LauncherShell, Route};
use crate::live::{LiveIntent, SettingsView, UpdateStatus};

/// The shell shows the Settings feature, gives it the update status and its
/// tab, and passes it the page's own intents: they never reach the
/// application.
#[gpui::test]
fn the_shell_shows_settings_and_keeps_its_intents_from_the_application(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<LiveIntent>>> = Rc::default();
    let sink = seen.clone();
    let slot: Rc<RefCell<Option<Entity<LauncherShell>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let shell = cx.new(|cx| {
            LauncherShell::new(cx)
                .with_live(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)))
        });
        *keep.borrow_mut() = Some(shell.clone());
        gpui_component::Root::new(shell, window, cx)
    });
    let shell = slot.borrow().clone().expect("shell");
    cx.simulate_resize(gpui::size(gpui::px(1080.), gpui::px(720.)));
    shell.update(cx, |shell, cx| {
        shell.show(Route::Settings);
        shell.update_live(|model| model.update_status = UpdateStatus::Checking, cx);
        shell.show_tab(Route::Settings, 0, cx);
        shell.settings_page().update(cx, |settings, cx| {
            settings.show_settings(SettingsView::default(), cx);
        });
    });
    cx.run_until_parked();

    let switch = cx
        .debug_bounds("settings-translation-switch-cap")
        .expect("the translation switch on 通用");
    cx.simulate_click(switch.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(seen.borrow().is_empty(), "Settings saves it itself");

    shell.update(cx, |shell, cx| shell.show_tab(Route::Settings, 5, cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("settings-update-status").is_some(),
        "the 关于 tab is open"
    );
    let tab = shell.read_with(cx, |shell, cx| shell.settings_page().read(cx).choices().tab);
    assert_eq!(tab, 5);
}
