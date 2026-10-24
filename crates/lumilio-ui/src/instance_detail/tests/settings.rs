use std::cell::RefCell;
use std::rc::Rc;

use gpui::TestAppContext;
use lumilio_core::{LauncherSettings, Loader};

use super::super::editors::Editor;
use super::super::forms::memory_draft;
use super::super::intent::{InstanceIntent, Section};
use super::super::{
    SETTINGS_ADVANCED, SETTINGS_GAME, SETTINGS_JAVA, SETTINGS_PERFORMANCE, SETTINGS_RUNTIME,
    TAB_SETTINGS,
};
use super::{click, record, rooted};

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
        view.settings_sub = SETTINGS_PERFORMANCE;
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
            SETTINGS_GAME,
            &["isettings-window", "isettings-after", "isettings-quick"][..],
        ),
        (
            SETTINGS_RUNTIME,
            &["isettings-version", "isettings-loader", "isettings-files"][..],
        ),
        (SETTINGS_JAVA, &["isettings-java", "isettings-jvm"][..]),
        (
            SETTINGS_PERFORMANCE,
            &["instance-memory-edit", "isettings-machine-memory"][..],
        ),
        (
            SETTINGS_ADVANCED,
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
        view.settings_sub = SETTINGS_GAME;
        cx.notify();
    });
    cx.run_until_parked();
    click(cx, "isettings-window-edit");
    assert!(
        cx.debug_bounds("settings-save").is_some(),
        "the dialog opened"
    );
    assert_eq!(
        seen.borrow().len(),
        4,
        "only the overview's loads and the Java group's so far"
    );
    assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Java)));

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
        view.settings_sub = SETTINGS_RUNTIME;
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
        view.select_settings_group(SETTINGS_JAVA, cx);
    });
    cx.run_until_parked();
    click(cx, "isettings-java-edit");
    assert!(cx.debug_bounds("settings-browse").is_some());
}

/// A game that follows a preset: it changed the volume in the game, keeps its
/// own render distance, and owns the mouse inversion the preset does not share.
fn options() -> lumilio_core::InstanceOptions {
    use lumilio_core::GameSettingValue::{Boolean, Decimal, Integer};
    let preset: lumilio_core::SettingsPreset = serde_json::from_value(serde_json::json!({
        "id": 1, "name": "Daily", "revision": 1,
        "values": {
            "soundCategory_master": {"Decimal": "0.8"},
            "renderDistance": {"Integer": 16}
        }
    }))
    .unwrap();
    let mut options = lumilio_core::InstanceOptions::default();
    options.game_version = "1.21.1".into();
    options.file = lumilio_core::OptionsFile::Ready;
    options.view.instance = "survival".into();
    options.view.preset = Some(preset);
    options
        .view
        .overrides
        .insert("renderDistance".into(), Integer(12));
    options.view.pending = vec![
        serde_json::from_value(serde_json::json!({
            "setting": "soundCategory_master",
            "before": {"Decimal": "0.8"},
            "value": {"Decimal": "0.4"},
            "shared_before": {"Decimal": "0.8"}
        }))
        .unwrap(),
    ];
    options
        .values
        .insert("soundCategory_master".into(), Decimal("0.4".into()));
    options.values.insert("renderDistance".into(), Integer(12));
    options.values.insert("invertYMouse".into(), Boolean(false));
    options
}

#[gpui::test]
fn options_are_edited_outside_the_game_and_game_changes_are_decided_together(
    cx: &mut TestAppContext,
) {
    use lumilio_core::{GameSettingValue, OptionChange, SettingResolution};

    use super::super::panels::Arrived;
    use super::super::{Operated, SETTINGS_OPTIONS};
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_settings_group(SETTINGS_OPTIONS, cx);
        view.select_tab(TAB_SETTINGS, cx);
    });
    cx.run_until_parked();
    assert!(
        seen.borrow()
            .contains(&InstanceIntent::Load(Section::GameOptions)),
        "opening the group reads the game's options"
    );
    view.update(cx, |view, cx| {
        view.arrived(Arrived::GameOptions(Ok(Box::new(options()))), cx)
    });
    click(cx, "options-preset-change");
    click(cx, "options-renderDistance-edit");
    // Changes made in the game are selected, then decided together.
    assert!(cx.debug_bounds("options-resolve-local").is_none());
    click(cx, "options-review-check-soundCategory_master-box");
    click(cx, "options-resolve-local");
    view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: String::new(),
                technical: None,
                refresh: Vec::new(),
            },
            cx,
        );
    });
    // An option the game owns switches at once.
    click(cx, "options-invertYMouse-switch-cap");
    let writes: Vec<_> = seen
        .borrow()
        .iter()
        .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
        .cloned()
        .collect();
    let expected = options().view;
    assert_eq!(
        writes,
        [
            InstanceIntent::ChoosePreset,
            InstanceIntent::EditOption("renderDistance".into()),
            InstanceIntent::ResolveOptions {
                expected: expected.clone(),
                selected: vec!["soundCategory_master".into()],
                resolution: SettingResolution::Instance,
            },
            InstanceIntent::SetOption {
                expected,
                key: "invertYMouse".into(),
                change: OptionChange::Set(GameSettingValue::Boolean(true)),
            },
        ]
    );
}

#[gpui::test]
fn a_running_game_keeps_its_options_still(cx: &mut TestAppContext) {
    use super::super::SETTINGS_OPTIONS;
    use super::super::panels::Arrived;
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let mut running = options();
    running.view.is_busy = true;
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_settings_group(SETTINGS_OPTIONS, cx);
        view.select_tab(TAB_SETTINGS, cx);
        view.arrived(Arrived::GameOptions(Ok(Box::new(running))), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("options-running").is_some());
    click(cx, "options-renderDistance-edit");
    click(cx, "options-preset-change");
    assert!(
        seen.borrow()
            .iter()
            .all(|intent| matches!(intent, InstanceIntent::Load(_))),
        "nothing is asked of a running game"
    );
}
