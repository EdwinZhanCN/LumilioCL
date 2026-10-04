use super::super::{LauncherShell, Route};
use gpui::{Modifiers, TestAppContext};
use std::cell::RefCell;
use std::rc::Rc;

#[gpui::test]
fn the_settings_page_shows_each_tab_and_edits_open_a_dialog(cx: &mut TestAppContext) {
    use crate::kit::ViewIntent;
    use crate::live::{LiveIntent, SettingsView};
    use crate::pages::settings::TAB_GROUP;
    use gpui::{AppContext as _, Entity};
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
    shell.update(cx, |shell, _| shell.show(Route::Settings));
    // Before the settings arrive the page says so instead of showing blanks.
    shell.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-appearance").is_none());

    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                model.settings = Some(SettingsView {
                    max_memory_mb: Some(4096),
                    total_memory_mb: Some(16_384),
                    data_dir: "/data".into(),
                    ..SettingsView::default()
                })
            },
            cx,
        );
    });
    cx.run_until_parked();
    for selector in [
        "settings-appearance",
        "settings-after-launch",
        "settings-foreground",
        "settings-motion",
        "settings-language",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} on 通用");
    }

    let tabs = [
        (1, "settings-memory"),
        (2, "settings-java-roots"),
        (3, "settings-data-dir"),
        (4, "settings-version"),
    ];
    for (tab, selector) in tabs {
        shell.update(cx, |shell, cx| {
            shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, tab), cx)
        });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} on tab {tab}"
        );
    }

    // An edit button opens its dialog; cancelling sends nothing.
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 1), cx)
    });
    cx.run_until_parked();
    let edit = cx
        .debug_bounds("settings-memory-edit")
        .expect("edit button");
    cx.simulate_click(edit.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("settings-save").is_some(),
        "the dialog opened"
    );
    assert!(seen.borrow().is_empty());
}
