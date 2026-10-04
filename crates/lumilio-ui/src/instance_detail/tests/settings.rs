use super::super::TAB_SETTINGS;
use super::super::editors::Editor;
use super::super::forms::memory_draft;
use super::super::intent::{InstanceIntent, Section};
use super::{click, record, rooted};
use gpui::TestAppContext;
use lumilio_core::{LauncherSettings, Loader};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn memory_form_preserves_other_overrides_and_uses_core_validation() {
    let saved = record().settings;
    let mut defaults = LauncherSettings::default();
    defaults.default_min_memory_mb = Some(512);
    defaults.default_max_memory_mb = Some(2048);
    let next = memory_draft(&saved, &defaults, "", "4096").unwrap();
    assert_eq!(next.java_path, saved.java_path);
    assert_eq!(next.jvm_arguments, saved.jvm_arguments);
    assert_eq!(next.min_memory_mb, None);
    assert_eq!(next.max_memory_mb, Some(4096));
    assert_eq!(memory_draft(&saved, &defaults, "", "").unwrap(), saved);
    for (min, max) in [
        ("-1", ""),
        ("x", ""),
        ("0", ""),
        ("4096", ""),
        ("", "1048577"),
        ("4096", "1024"),
    ] {
        assert!(memory_draft(&saved, &defaults, min, max).is_err());
    }
}

#[gpui::test]
fn memory_is_read_only_on_the_page_and_edited_in_a_dialog(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_SETTINGS, cx);
        view.settings_sub = 3;
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("instance-memory-save").is_none(),
        "the page has no inline form"
    );
    click(cx, "instance-memory-edit");
    assert!(
        cx.debug_bounds("dialog-layer").is_some(),
        "the dialog opened"
    );
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let fields = view.fields.as_ref().unwrap();
            fields
                .max
                .update(cx, |input, cx| input.set_value("4096", window, cx));
        })
    });
    click(cx, "instance-memory-save");
    let mut next = record();
    next.settings.max_memory_mb = Some(4096);
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            InstanceIntent::Load(Section::Problems),
            InstanceIntent::Load(Section::Size),
            InstanceIntent::Load(Section::History),
            InstanceIntent::SaveMemory(next.settings.clone())
        ]
    );
    click(cx, "instance-memory-save");
    assert_eq!(seen.borrow().len(), 4, "no duplicate submit while busy");

    // A failure keeps the dialog and the draft, and says so inside it.
    view.update(cx, |view, cx| view.saved(true, Err("disk full".into()), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("instance-editor-error").is_some());
    view.read_with(cx, |view, cx| {
        assert_eq!(view.fields.as_ref().unwrap().max.read(cx).value(), "4096");
        assert_eq!(view.record.as_ref().unwrap().settings.max_memory_mb, None);
        assert_eq!(view.editor, Some(Editor::Memory));
    });

    // "Follow the defaults" clears both and saves at once.
    click(cx, "instance-memory-reset");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::SaveMemory(record().settings))
    );

    // Success closes the dialog and the page shows the saved value.
    view.update(cx, |view, cx| {
        view.saved(true, Ok((next, LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("dialog-layer").is_none(),
        "the dialog itself closed, not only its content"
    );
    view.read_with(cx, |view, _| assert_eq!(view.editor, None));
}

#[gpui::test]
fn the_settings_groups_show_effective_values_and_restoring_removes_the_override(
    cx: &mut TestAppContext,
) {
    use lumilio_core::{InstanceLaunch, LaunchTuning};
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let mut own = record();
    own.settings.launch = InstanceLaunch {
        fullscreen: Some(true),
        ..InstanceLaunch::default()
    };
    let mut defaults = LauncherSettings::default();
    defaults.launch = LaunchTuning {
        window_width: Some(1280),
        window_height: Some(720),
        ..LaunchTuning::default()
    };
    view.update(cx, |view, cx| {
        view.loaded(Ok((own.clone(), defaults)), cx);
        view.set_machine_memory(Some(16_384), cx);
        view.select_tab(TAB_SETTINGS, cx);
    });
    cx.run_until_parked();

    // Every group's rows are there.
    for (sub, selectors) in [
        (
            0,
            &["isettings-window", "isettings-after", "isettings-quick"][..],
        ),
        (
            1,
            &["isettings-version", "isettings-loader", "isettings-files"][..],
        ),
        (2, &["isettings-java", "isettings-jvm"][..]),
        (3, &["instance-memory-edit", "isettings-machine-memory"][..]),
        (
            4,
            &["isettings-game-args", "isettings-env", "isettings-commands"][..],
        ),
    ] {
        view.update(cx, |view, cx| {
            view.settings_sub = sub;
            cx.notify();
        });
        cx.run_until_parked();
        for selector in selectors {
            assert!(
                cx.debug_bounds(selector).is_some(),
                "{selector} in group {sub}"
            );
        }
    }

    // The window row follows the defaults for size and shows its own fullscreen.
    view.update(cx, |view, cx| {
        view.settings_sub = 0;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "isettings-window-edit");
    assert!(
        cx.debug_bounds("settings-save").is_some(),
        "the dialog opened"
    );
    assert_eq!(seen.borrow().len(), 3, "only the overview's loads so far");

    // Restoring defaults sends the settings with the override removed.
    click(cx, "settings-reset");
    let mut cleared = own.settings.clone();
    cleared.launch = InstanceLaunch::default();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::SaveSettings(cleared))
    );

    // The answer shows a toast and the new values; a failure keeps the page.
    view.update(cx, |view, cx| {
        view.settings_saved(Err("disk full".into()), cx);
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(!view.busy));
}

#[gpui::test]
fn an_invalid_draft_is_explained_in_the_dialog_and_never_sent(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
    });
    click(cx, "instance-rename");
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            let fields = view.fields.as_ref().unwrap();
            fields
                .name
                .update(cx, |input, cx| input.set_value("  ", window, cx));
        })
    });
    click(cx, "instance-rename-save");
    assert!(cx.debug_bounds("instance-editor-error").is_some());
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|intent| matches!(intent, InstanceIntent::Rename(_))),
        "an empty name is never sent"
    );
}

#[gpui::test]
fn the_runtime_group_offers_a_change_and_a_repair(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_SETTINGS, cx);
        view.settings_sub = 1;
    });
    cx.run_until_parked();
    click(cx, "isettings-version-change");
    click(cx, "isettings-loader-change");
    click(cx, "isettings-repair");
    let writes: Vec<_> = seen
        .borrow()
        .iter()
        .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
        .cloned()
        .collect();
    assert_eq!(
        writes,
        [
            InstanceIntent::OpenRuntimeChange,
            InstanceIntent::OpenRuntimeChange,
            InstanceIntent::Repair
        ]
    );
    assert_eq!(
        view.read_with(cx, |view, _| view.runtime()),
        Some(("1.21.1".into(), Loader::Vanilla, None))
    );
}

#[gpui::test]
fn the_java_dialog_can_browse_the_disk_for_a_path(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_SETTINGS, cx);
        view.select_settings_group(2, cx);
    });
    cx.run_until_parked();
    click(cx, "isettings-java-edit");
    assert!(cx.debug_bounds("settings-browse").is_some());
}
