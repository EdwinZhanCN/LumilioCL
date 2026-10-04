use super::InstanceDetailView;
use super::Operated;
use super::TAB_CONTENT;
use super::TAB_DIAGNOSTICS;
use super::TAB_HISTORY;
use super::TAB_OVERVIEW;
use super::TAB_SETTINGS;
use super::TAB_WORLDS;
use super::editors::Editor;
use super::forms::memory_draft;
use super::intent::InstanceIntent;
use super::intent::Section;
use super::panels::{Arrived, Confirm};
use crate::toast::Toast;
use gpui::Entity;
use gpui::prelude::*;
use lumilio_core::{CrashHint, ProjectKind};
use lumilio_core::{InstanceRecord, InstanceSettings, LauncherSettings};
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};
use lumilio_core::Loader;
use std::cell::RefCell;

fn record() -> InstanceRecord {
    InstanceRecord {
        id: "survival".into(),
        name: "生存".into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        favorite: true,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: false,
        settings: InstanceSettings {
            java_path: Some("/jdk/bin/java".into()),
            jvm_arguments: vec!["-Dx=y".into()],
            ..InstanceSettings::default()
        },
    }
}

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

/// The view under the framework `Root`, which renders the dialog layer.
fn rooted(
    cx: &mut TestAppContext,
    seen: Rc<RefCell<Vec<InstanceIntent>>>,
) -> (Entity<InstanceDetailView>, &mut gpui::VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let slot: Rc<RefCell<Option<Entity<InstanceDetailView>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| {
            InstanceDetailView::new(
                "survival".into(),
                Rc::new(move |intent, _, _| seen.borrow_mut().push(intent)),
            )
        });
        *keep.borrow_mut() = Some(view.clone());
        gpui_component::Root::new(view, window, cx)
    });
    let view = slot.borrow().clone().expect("view");
    (view, cx)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} is not on screen"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
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

fn world(folder: &str) -> lumilio_core::WorldInfo {
    lumilio_core::WorldInfo {
        folder: folder.into(),
        name: folder.into(),
        last_played_ms: None,
        game_version: None,
        hardcore: false,
        has_icon: false,
        damaged: false,
        lock_touched_ms: None,
    }
}

fn item(name: &str, enabled: bool) -> lumilio_core::ContentItem {
    lumilio_core::ContentItem {
        file_name: if enabled {
            name.into()
        } else {
            format!("{name}.disabled")
        },
        display_name: name.into(),
        enabled,
        size: 2048,
        modified: 0,
        is_directory: false,
    }
}

#[gpui::test]
fn tabs_load_their_data_once_and_ignore_a_second_request_while_pending(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        InstanceDetailView::new(
            "survival".into(),
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        &[
            InstanceIntent::Load(Section::Problems),
            InstanceIntent::Load(Section::Size),
            InstanceIntent::Load(Section::History),
        ],
        "the overview asks for what it shows, once"
    );
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_tab(TAB_WORLDS, window, cx);
            view.open_tab(TAB_OVERVIEW, window, cx);
            view.open_tab(TAB_WORLDS, window, cx);
        })
    });
    let loads = seen
        .borrow()
        .iter()
        .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
        .count();
    assert_eq!(loads, 1, "a pending section is not requested again");
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx)
    });
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.open_tab(TAB_OVERVIEW, window, cx);
            view.open_tab(TAB_WORLDS, window, cx);
        })
    });
    let loads = seen
        .borrow()
        .iter()
        .filter(|intent| **intent == InstanceIntent::Load(Section::Worlds))
        .count();
    assert_eq!(loads, 1, "a loaded section is shown, not re-read");
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
fn a_world_can_be_entered_directly_unless_the_version_is_too_old(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        InstanceDetailView::new(
            "survival".into(),
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    let enter = cx.debug_bounds("world-play-0").expect("enter button");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::PlayWorld("w".into()))
    );

    // An old game version cannot go straight in: the button does nothing.
    let mut old = record();
    old.game_version = "1.16.5".into();
    view.update(cx, |view, cx| {
        view.busy = false;
        view.loaded(Ok((old, LauncherSettings::default())), cx);
    });
    cx.run_until_parked();
    let before = seen.borrow().len();
    let enter = cx.debug_bounds("world-play-0").expect("enter button");
    cx.simulate_click(enter.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(seen.borrow().len(), before);
}

#[gpui::test]
fn the_world_list_is_searched_and_a_world_can_be_imported(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("world-play-1").is_some());

    let search = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().world_search.clone()
    });
    cx.update(|window, cx| search.update(cx, |search, cx| search.set_value("bet", window, cx)));
    cx.run_until_parked();
    assert!(cx.debug_bounds("world-play-1").is_none(), "one world left");
    click(cx, "world-play-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::PlayWorld("beta".into()))
    );

    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    click(cx, "world-import");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ImportWorld));
    assert!(
        !view.read_with(cx, |view, _| view.busy),
        "choosing a file is not a write; cancelling it must not leave the page busy"
    );
}

#[gpui::test]
fn a_crashed_session_leads_to_the_log_and_a_clean_one_does_not(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let session = |started, outcome| lumilio_core::HistoryEvent::Session {
        started,
        seconds: 60,
        exit_code: None,
        outcome,
    };
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_HISTORY, cx);
        view.history_sub = 1;
        view.arrived(
            Arrived::History(Ok(lumilio_core::HistoryRead {
                events: vec![
                    session(100, lumilio_core::SessionOutcome::Clean),
                    session(200, lumilio_core::SessionOutcome::Crashed),
                ],
                skipped: 0,
            })),
            cx,
        );
    });
    cx.run_until_parked();
    click(cx, "session-inspect");
    view.read_with(cx, |view, _| {
        assert_eq!(view.tab, TAB_DIAGNOSTICS);
        assert_eq!(view.diag_sub, 1);
    });
    assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|intent| matches!(intent, InstanceIntent::OpenCrash(_))),
        "the reports are not known yet"
    );

    // The list arrives: the report written during that session opens.
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: None,
                crashes: vec![
                    lumilio_core::CrashReport {
                        file_name: "elsewhen.txt".into(),
                        modified: 9000,
                    },
                    lumilio_core::CrashReport {
                        file_name: "crash-2.txt".into(),
                        modified: 262,
                    },
                ],
            })),
            cx,
        )
    });
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenCrash("crash-2.txt".into()))
    );
}

#[gpui::test]
fn deleting_a_world_asks_first_and_never_double_submits(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    let writes = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
        seen.borrow()
            .iter()
            .filter(|intent| !matches!(intent, InstanceIntent::Load(_)))
            .cloned()
            .collect::<Vec<_>>()
    };
    let ask = cx.debug_bounds("world-delete-0").expect("delete button");
    cx.simulate_click(ask.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(writes(&seen).is_empty(), "the click only asks, in a dialog");
    view.read_with(cx, |view, _| {
        assert_eq!(view.confirm, Some(Confirm::DeleteWorld("w".into())));
    });
    // Answering yes (the dialog's button calls this) sends it once.
    let yes = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.confirmed(&Confirm::DeleteWorld("w".into()), window, cx)
            })
        });
        cx.run_until_parked();
    };
    yes(cx);
    assert_eq!(writes(&seen), vec![InstanceIntent::DeleteWorld("w".into())]);
    // While it runs, the world cannot be deleted again.
    view.read_with(cx, |view, _| assert!(view.busy));
    let sent = writes(&seen).len();
    yes(cx);
    assert_eq!(writes(&seen).len(), sent);
    // The answer re-reads only what was looked at (worlds, no snapshots).
    let stale = view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: "世界已删除".into(),
                technical: None,
                refresh: vec![Section::Worlds, Section::Snapshots],
            },
            cx,
        )
    });
    assert_eq!(stale, vec![Section::Worlds]);
    view.read_with(cx, |view, _| assert!(!view.busy));
}

/// Files Modrinth does not know.
fn listed(items: Vec<lumilio_core::ContentItem>) -> lumilio_core::ContentList {
    lumilio_core::ContentList {
        entries: items
            .into_iter()
            .map(|item| lumilio_core::ContentEntry {
                item,
                sha1: None,
                source: None,
                update: None,
            })
            .collect(),
        sources_unavailable: false,
    }
}

fn known_version(id: &str, number: &str, game: &str) -> lumilio_core::Version {
    lumilio_core::Version {
        id: id.into(),
        project_id: "P".into(),
        name: number.into(),
        number: number.into(),
        channel: lumilio_core::ReleaseChannel::Release,
        game_versions: vec![game.into()],
        loaders: vec!["fabric".into()],
        published: "2026-09-01T00:00:00Z".into(),
        files: vec![lumilio_core::VersionFile {
            url: "https://cdn/x.jar".into(),
            filename: format!("apple-{number}.jar"),
            primary: true,
            size: 1,
            sha1: Some("new".into()),
        }],
        dependencies: Vec::new(),
        downloads: 0,
        changelog: "Updated for 26.3".into(),
    }
}

/// A long version shares a row with the Update key and may not run
/// underneath it. The newer version is not in the row: the Update key
/// opens the dialog that names it.
#[gpui::test]
fn a_long_version_stays_clear_of_the_update_key(cx: &mut TestAppContext) {
    let (view, cx) = rooted(cx, Rc::default());
    let mut fabric = record();
    fabric.loader = lumilio_core::Loader::Fabric;
    fabric.game_version = "26.3".into();
    let newest = known_version("v2", "1.11.7+26.3", "26.3");
    let mut list = listed(vec![item("iris-fabric-1.11.4+mc26.2.jar", true)]);
    list.entries[0].source = Some(lumilio_core::ContentSource {
        project_id: "P".into(),
        slug: "iris".into(),
        title: "Iris Shaders".into(),
        author: Some("coderbot".into()),
        icon_url: None,
        version_id: "v1".into(),
        version_number: "1.11.4+26.2-fabric-with-a-long-suffix".into(),
    });
    list.entries[0].update = Some(newest);
    view.update(cx, |view, cx| {
        view.loaded(Ok((fabric, LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list)), cx);
    });
    cx.run_until_parked();
    let version = cx
        .debug_bounds("content-version-0")
        .expect("version column");
    let update = cx
        .debug_bounds("content-switch-version-0")
        .expect("update key");
    let actions = cx.debug_bounds("content-actions-0").expect("action column");
    assert!(
        update.left() >= actions.left() && update.right() <= actions.right(),
        "the Update key ({update:?}) sits inside its column ({actions:?})"
    );
    assert!(
        version.right() <= update.left(),
        "the version column ({version:?}) ends before the Update key ({update:?})"
    );
}

#[gpui::test]
fn an_identified_file_switches_version_in_a_dialog_and_bulk_acts_on_the_selection(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let mut fabric = record();
    fabric.loader = lumilio_core::Loader::Fabric;
    fabric.game_version = "26.3".into();
    let newest = known_version("v2", "3.0.11", "26.3");
    let mut list = listed(vec![item("apple.jar", true), item("mine.jar", true)]);
    list.entries[0].source = Some(lumilio_core::ContentSource {
        project_id: "P".into(),
        slug: "appleskin".into(),
        title: "AppleSkin".into(),
        author: Some("squeek502".into()),
        icon_url: None,
        version_id: "v1".into(),
        version_number: "3.0.10".into(),
    });
    list.entries[0].update = Some(newest.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((fabric, LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list)), cx);
    });
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("content-switch-version-1").is_none(),
        "an unknown file cannot switch versions"
    );
    click(cx, "content-switch-version-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::LoadVersions("P".into()))
    );
    view.update(cx, |view, cx| {
        view.versions_arrived(
            "P",
            Ok(vec![
                newest.clone(),
                known_version("v1", "3.0.10", "26.3"),
                known_version("v0", "2.0", "1.20"),
            ]),
            cx,
        )
    });
    cx.run_until_parked();
    // Opened as "update": the newest compatible version is chosen.
    click(cx, "switch-commit");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::SwitchContent {
            kind: ProjectKind::Mod,
            file_name: "apple.jar".into(),
            project: "P".into(),
            version_id: "v2".into(),
        })
    );
    view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: "已换成 apple-3.0.11.jar".into(),
                technical: None,
                refresh: vec![Section::Content(ProjectKind::Mod)],
            },
            cx,
        );
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("dialog-layer").is_none(),
        "the dialog closed"
    );

    // Select both rows: the bulk bar replaces the toolbar and acts on both.
    view.update(cx, |view, cx| {
        view.selected.insert("apple.jar".into());
        view.selected.insert("mine.jar".into());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("content-bulk").is_some());
    click(cx, "content-bulk-disable");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::SetContent {
            kind: ProjectKind::Mod,
            files: vec!["apple.jar".into(), "mine.jar".into()],
            enabled: false,
        })
    );
    view.read_with(cx, |view, _| assert!(view.selected.is_empty()));
}

#[gpui::test]
fn content_switches_send_one_change_and_a_failure_keeps_the_list(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        InstanceDetailView::new(
            "survival".into(),
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(
            Arrived::Content(
                ProjectKind::Mod,
                Ok(listed(vec![item("a.jar", true), item("b.jar", false)])),
            ),
            cx,
        );
    });
    cx.run_until_parked();
    let switch = cx.debug_bounds("content-switch-1").expect("second switch");
    cx.simulate_click(switch.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::SetContent {
            kind: ProjectKind::Mod,
            files: vec!["b.jar.disabled".into()],
            enabled: true,
        })
    );
    let sent = seen.borrow().len();
    let other = cx.debug_bounds("content-switch-0").expect("first switch");
    cx.simulate_click(other.center(), Modifiers::none());
    assert_eq!(seen.borrow().len(), sent, "busy: no second change");
    view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: "没有成功".into(),
                technical: Some("disk full".into()),
                refresh: Vec::new(),
            },
            cx,
        )
    });
    view.read_with(cx, |view, _| {
        assert!(!view.busy);
        assert!(matches!(&view.data.content[0], Some(Ok(list)) if list.entries.len() == 2));
        assert_eq!(
            view.pending_toasts(),
            [Toast::error("没有成功").technical("disk full")],
            "the failure floats up; the page keeps its list"
        );
    });
}

#[gpui::test]
fn the_log_tab_loads_once_and_shows_only_the_report_last_asked_for(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        InstanceDetailView::new(
            "survival".into(),
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx)));
    assert!(seen.borrow().contains(&InstanceIntent::Load(Section::Logs)));
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some("[main] Done".into()),
                crashes: vec![lumilio_core::CrashReport {
                    file_name: "crash-1.txt".into(),
                    modified: 5,
                }],
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let open = cx.debug_bounds("crash-open").expect("view button");
    cx.simulate_click(open.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenCrash("crash-1.txt".into()))
    );
    view.update(cx, |view, cx| {
        view.crash_arrived("crash-0.txt".into(), Ok(("old".into(), Vec::new())), cx);
    });
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.crash, Some((file, None)) if file == "crash-1.txt"));
    });
    view.update(cx, |view, cx| {
        view.crash_arrived(
            "crash-1.txt".into(),
            Ok(("OOM".into(), vec![CrashHint::OutOfMemory])),
            cx,
        );
    });
    view.read_with(cx, |view, _| {
        assert!(matches!(&view.crash, Some((_, Some(Ok((text, _))))) if text == "OOM"));
    });
    cx.run_until_parked();
    let export = cx.debug_bounds("instance-log-export").expect("export log");
    cx.simulate_click(export.center(), Modifiers::none());
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::ExportLog(None)));
    let export = cx.debug_bounds("crash-export").expect("export report");
    cx.simulate_click(export.center(), Modifiers::none());
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::ExportLog(Some("crash-1.txt".into())))
    );
}

#[gpui::test]
fn the_diagnostics_tab_loads_the_part_it_shows_and_lists_one_folder_at_a_time(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_tab(TAB_DIAGNOSTICS, window, cx)));
    assert!(
        seen.borrow()
            .contains(&InstanceIntent::Load(Section::Problems)),
        "问题 is the first part"
    );
    assert!(
        !seen
            .borrow()
            .contains(&InstanceIntent::Load(Section::Files))
    );

    view.update(cx, |view, _| view.diag_sub = 2);
    cx.update(|window, cx| {
        view.update(cx, |view, cx| view.open_diagnostics(window, cx));
        view.update(cx, |view, cx| view.open_diagnostics(window, cx));
    });
    let loads = |seen: &Rc<RefCell<Vec<InstanceIntent>>>| {
        seen.borrow()
            .iter()
            .filter(|intent| **intent == InstanceIntent::Load(Section::Files))
            .count()
    };
    assert_eq!(loads(&seen), 1, "asked once while the answer is on its way");

    let entry = |name: &str, is_dir: bool| lumilio_core::FileEntry {
        name: name.into(),
        is_dir,
        size: 12,
        modified: 5,
    };
    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Files(
                String::new(),
                Ok(vec![entry("config", true), entry("options.txt", false)]),
            ),
            cx,
        )
    });
    click(cx, "file-open-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::OpenFolder("config".into()))
    );
    let count = seen.borrow().len();
    // The answer has not come: a second click does not ask again.
    click(cx, "file-open-0");
    assert_eq!(seen.borrow().len(), count);

    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::Files("config".into(), Ok(vec![entry("sodium.json", false)])),
            cx,
        )
    });
    click(cx, "file-reveal-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::RevealPath("config/sodium.json".into()))
    );
}

/// Gpui hands a wheel event to every scrollable under the pointer, so a
/// log that scrolls inside a page that scrolls moved both at once.
#[gpui::test]
fn scrolling_the_log_leaves_the_page_where_it_is(cx: &mut TestAppContext) {
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};

    let (view, cx) = rooted(cx, Rc::default());
    let text: String = (0..300)
        .map(|n| format!("[{n}] [main/INFO]: line {n}\n"))
        .collect();
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some(text),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.simulate_resize(gpui::size(px(1080.), px(500.)));
    cx.run_until_parked();

    let wheel = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-120.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
    };
    let lines = cx.debug_bounds("instance-log-lines").expect("log lines");
    let at = point(lines.center().x, lines.top() + px(40.));
    wheel(cx, at);
    let after = cx.debug_bounds("instance-log-lines").expect("log lines");
    assert_eq!(
        after.origin.y, lines.origin.y,
        "the page stayed put while the log scrolled"
    );
    let scrolled = view.read_with(cx, |view, _| view.log_scroll.offset().y);
    assert!(scrolled < px(0.), "the log itself scrolled ({scrolled:?})");
}

#[gpui::test]
fn the_log_view_filters_by_level_and_search_and_keeps_stack_traces_with_their_line(
    cx: &mut TestAppContext,
) {
    let (view, cx) = rooted(cx, Rc::default());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some(
                    "[1] [main/INFO]: loaded sodium\n[2] [main/WARN]: slow tick\n[3] [main/ERROR]: boom\n\tat a.B(B.java)"
                        .into(),
                ),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
    };
    assert_eq!(shown(&view, cx).lines().count(), 4);
    view.update(cx, |view, cx| {
        view.log_level = 1;
        cx.notify();
    });
    assert_eq!(
        shown(&view, cx),
        "[3] [main/ERROR]: boom\n\tat a.B(B.java)",
        "the trace line belongs to the error"
    );
    view.update(cx, |view, _| view.log_level = 2);
    assert_eq!(shown(&view, cx).lines().count(), 3);
    view.update(cx, |view, _| view.log_level = 0);
    let search = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().log_search.clone()
    });
    cx.update(|window, cx| search.update(cx, |search, cx| search.set_value("SODIUM", window, cx)));
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: loaded sodium");
}

#[gpui::test]
fn the_running_games_output_replaces_the_log_file_until_it_stops(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.diag_sub = 1;
        view.select_tab(TAB_DIAGNOSTICS, cx);
        view.arrived(
            Arrived::Logs(Ok(lumilio_core::GameLogs {
                latest: Some("[1] [main/INFO]: from the file".into()),
                crashes: Vec::new(),
            })),
            cx,
        );
    });
    cx.run_until_parked();
    let shown = |view: &Entity<InstanceDetailView>, cx: &mut gpui::VisualTestContext| {
        view.read_with(cx, |view, cx| view.visible_log(cx).unwrap())
    };
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");

    view.update(cx, |view, cx| {
        view.game_output(
            vec![
                "[2] [main/INFO]: live one".into(),
                "[3] [main/WARN]: live two".into(),
            ],
            true,
            cx,
        )
    });
    assert_eq!(
        shown(&view, cx),
        "[2] [main/INFO]: live one\n[3] [main/WARN]: live two"
    );
    // The level filter works on the live lines too.
    view.update(cx, |view, _| view.log_level = 2);
    assert_eq!(shown(&view, cx), "[3] [main/WARN]: live two");

    let before = seen
        .borrow()
        .iter()
        .filter(|i| **i == InstanceIntent::Load(Section::Logs))
        .count();
    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    let after = seen
        .borrow()
        .iter()
        .filter(|i| **i == InstanceIntent::Load(Section::Logs))
        .count();
    assert_eq!(
        after,
        before + 1,
        "the file is read again once the game ends"
    );
    view.update(cx, |view, _| view.log_level = 0);
    assert_eq!(shown(&view, cx), "[1] [main/INFO]: from the file");
}

#[gpui::test]
fn the_header_starts_the_game_and_becomes_a_stop_button_while_it_runs(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    click(cx, "instance-play");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Play));
    assert!(cx.debug_bounds("instance-stop").is_none());

    view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("instance-play").is_none());
    click(cx, "instance-stop");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Stop));

    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("instance-play").is_some());
}

#[gpui::test]
fn the_overview_sizes_the_game_lists_recent_things_and_acts_on_a_problem(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    for section in [Section::Problems, Section::Size, Section::History] {
        assert!(
            seen.borrow().contains(&InstanceIntent::Load(section)),
            "the overview asks for {section:?}"
        );
    }
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Size(Ok(3 * 1024 * 1024)), cx);
        view.arrived(
            Arrived::Problems(Ok(vec![lumilio_core::Problem {
                severity: lumilio_core::Severity::Error,
                kind: lumilio_core::ProblemKind::DamagedFiles { count: 2 },
            }])),
            cx,
        );
        view.arrived(
            Arrived::History(Ok(lumilio_core::HistoryRead {
                events: vec![lumilio_core::HistoryEvent::Session {
                    started: 5,
                    seconds: 90,
                    exit_code: None,
                    outcome: lumilio_core::SessionOutcome::Clean,
                }],
                skipped: 0,
            })),
            cx,
        );
    });
    cx.run_until_parked();
    click(cx, "problem-action-0");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::Repair));

    view.update(cx, |view, cx| {
        view.busy = false;
        cx.notify();
    });
    click(cx, "overview-all-sessions");
    view.read_with(cx, |view, _| {
        assert_eq!(view.tab, TAB_HISTORY);
        assert_eq!(view.history_sub, 1, "straight to the play records");
    });
}

#[gpui::test]
fn a_question_asked_before_the_game_is_loaded_waits_for_it(cx: &mut TestAppContext) {
    let (view, cx) = rooted(cx, Rc::default());
    view.update(cx, |view, cx| view.ask_later(InstanceIntent::AskCopy, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(view.editor.is_none(), "nothing to copy yet");
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert_eq!(view.editor, Some(Editor::Copy));
        assert!(view.ask_later.is_none(), "asked once");
    });
}

#[gpui::test]
fn a_snapshot_is_taken_from_a_dialog_with_a_note_and_a_range(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_HISTORY, cx);
        view.history_sub = 2;
        view.arrived(Arrived::Snapshots(Ok(Vec::new())), cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("alpha"), world("beta")])), cx);
    });
    cx.run_until_parked();
    click(cx, "snapshot-create");
    view.read_with(cx, |view, _| {
        assert_eq!(view.editor, Some(Editor::Snapshot))
    });

    // A note and the second world.
    let note = view.read_with(cx, |view, _| {
        view.fields.as_ref().unwrap().snapshot_note.clone()
    });
    cx.update(|window, cx| {
        note.update(cx, |note, cx| {
            note.set_value("  before the farm  ", window, cx)
        })
    });
    view.update(cx, |view, cx| {
        view.snapshot_scope = 2;
        cx.notify();
    });
    click(cx, "instance-snapshot-go");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::CreateSnapshotAs {
            note: "before the farm".into(),
            world: Some("beta".into()),
        })
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

#[gpui::test]
fn while_a_game_runs_the_worlds_cannot_be_touched_and_a_dropped_archive_is_one_write(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_WORLDS, cx);
        view.arrived(Arrived::Worlds(Ok(vec![world("w")])), cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| view.game_output(Vec::new(), true, cx));
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert!(view.game_started_ms.is_some()));
    let before = seen.borrow().len();
    click(cx, "world-import");
    click(cx, "world-play-0");
    assert_eq!(
        seen.borrow().len(),
        before,
        "nothing is sent for a world while the game runs"
    );

    view.update(cx, |view, cx| view.game_output(Vec::new(), false, cx));
    cx.run_until_parked();
    cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.import_dropped(
                &[
                    "/tmp/a.txt".into(),
                    "/tmp/w.zip".into(),
                    "/tmp/x.zip".into(),
                ],
                window,
                cx,
            )
        })
    });
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::AddWorld("/tmp/w.zip".into()))
    );
}

#[gpui::test]
fn a_response_for_another_instance_cannot_replace_the_view(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let view = cx.new(|_| InstanceDetailView::new("survival".into(), Rc::new(|_, _, _| {})));
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx)
    });
    let mut other = record();
    other.id = "other".into();
    other.name = "另一个游戏".into();
    view.update(cx, |view, cx| {
        view.loaded(Ok((other, LauncherSettings::default())), cx)
    });
    assert_eq!(
        view.read_with(cx, |view, _| view.title().to_owned()),
        "生存"
    );
}
