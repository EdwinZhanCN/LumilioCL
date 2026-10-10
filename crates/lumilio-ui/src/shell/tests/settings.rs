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
    cx.simulate_resize(gpui::size(gpui::px(1080.), gpui::px(720.)));
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
        "settings-after-launch",
        "settings-foreground",
        "settings-motion",
        "settings-language",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} on 通用");
    }

    let tabs = [
        (1, "settings-appearance"),
        (1, "settings-theme-light"),
        (1, "settings-theme-dark"),
        (1, "settings-theme-local"),
        (1, "settings-font-sans"),
        (1, "settings-font-mono"),
        (1, "settings-font-cjk"),
        (1, "settings-scale"),
        (1, "settings-wallpaper"),
        (2, "settings-memory"),
        (3, "settings-java-roots"),
        (4, "settings-data-dir"),
        (5, "settings-version"),
        (6, "settings-plugin-empty"),
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

    // Mirror presets send their stable identity; core merges the saved rules.
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 4), cx)
    });
    cx.run_until_parked();
    for (selector, preset) in [
        ("settings-bmclapi-add", lumilio_core::MirrorPreset::Bmclapi),
        ("settings-mcim-add", lumilio_core::MirrorPreset::Mcim),
        (
            "settings-tencent-maven-add",
            lumilio_core::MirrorPreset::TencentMaven,
        ),
    ] {
        let before = shell.read_with(cx, |shell, _| {
            shell
                .live
                .as_ref()
                .unwrap()
                .settings
                .as_ref()
                .unwrap()
                .mirrors
                .clone()
        });
        let button = cx.debug_bounds(selector).expect("preset add button");
        cx.simulate_click(button.center(), Modifiers::none());
        cx.run_until_parked();
        let expected = preset.merge(&before);
        assert_eq!(
            seen.borrow().as_slice(),
            [LiveIntent::AddMirrorPreset(preset)]
        );
        seen.borrow_mut().clear();
        shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| {
                    model.settings.as_mut().unwrap().mirrors = expected;
                },
                cx,
            )
        });
        cx.run_until_parked();
        let button = cx.debug_bounds(selector).unwrap();
        cx.simulate_click(button.center(), Modifiers::none());
        cx.run_until_parked();
        assert!(
            seen.borrow().is_empty(),
            "added presets cannot be submitted twice"
        );
    }
    for (selector, preference) in [
        (
            "settings-download-source-key-0",
            lumilio_core::DownloadSourcePreference::OfficialOnly,
        ),
        (
            "settings-download-source-key-2",
            lumilio_core::DownloadSourcePreference::MirrorFirst,
        ),
        (
            "settings-download-source-key-1",
            lumilio_core::DownloadSourcePreference::OfficialFirst,
        ),
    ] {
        let key = cx.debug_bounds(selector).expect("source choice");
        cx.simulate_click(key.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            seen.borrow().as_slice(),
            [LiveIntent::SetDownloadSource(preference)]
        );
        seen.borrow_mut().clear();
        shell.update(cx, |shell, cx| {
            shell.update_live(
                |model| model.settings.as_mut().unwrap().download_source = preference,
                cx,
            )
        });
        cx.run_until_parked();
    }
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 6), cx)
    });
    cx.run_until_parked();

    // The plugin row sends the stable plugin ID, with its requested new state.
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                let view = model.settings.as_mut().unwrap();
                view.plugins = vec![lumilio_core::PluginInfo {
                    manifest: lumilio_plugin_api::Manifest {
                        id: "test.fake".into(),
                        name: lumilio_plugin_api::Words::new("测试插件", "Test plugin"),
                        description: lumilio_plugin_api::Words::new("测试说明", "For tests"),
                        version: "1".into(),
                        api: lumilio_plugin_api::API_VERSION,
                        default_enabled: true,
                        permissions: Vec::new(),
                        settings: Vec::new(),
                    },
                    state: Default::default(),
                    status: lumilio_core::PluginStatus::Enabled,
                }];
            },
            cx,
        );
    });
    cx.run_until_parked();
    let toggle = cx
        .debug_bounds("test.fake-enabled-cap")
        .expect("plugin switch");
    cx.simulate_click(toggle.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SetPluginEnabled {
            id: "test.fake".into(),
            enabled: false
        }]
    );
    seen.borrow_mut().clear();

    // An edit button opens its dialog; cancelling sends nothing.
    shell.update(cx, |shell, cx| {
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 2), cx)
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

    // 外观: a chosen font shows in its row, and 恢复默认 in its dialog clears
    // it through the preferences.
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    shell.update(cx, |shell, cx| {
        shell.update_live(
            |model| {
                let view = model.settings.as_mut().unwrap();
                view.preferences.look.mono_font = Some("Menlo".into());
            },
            cx,
        );
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 1), cx)
    });
    cx.run_until_parked();
    let edit = cx
        .debug_bounds("settings-font-mono-edit")
        .expect("font edit button");
    cx.simulate_click(edit.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("settings-save").is_some(),
        "the font dialog opened"
    );
    let reset = cx.debug_bounds("settings-reset").expect("restore defaults");
    cx.simulate_click(reset.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        seen.borrow().as_slice(),
        [LiveIntent::SetPreferences(
            lumilio_core::Preferences::default()
        )]
    );
}

/// Settings is an app shell: on the 插件 tab the list and the detail each
/// scroll on their own, and scrolling one never moves the other or the tabs.
#[gpui::test]
fn the_plugin_list_stays_put_while_its_detail_scrolls(cx: &mut TestAppContext) {
    use crate::kit::ViewIntent;
    use crate::live::SettingsView;
    use crate::pages::settings::TAB_GROUP;
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point, px};
    use lumilio_plugin_api::Permission;

    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    cx.simulate_resize(gpui::size(px(1080.), px(600.)));
    shell.update(cx, |shell, cx| {
        shell.show(Route::Settings);
        shell.apply_view_intent(ViewIntent::Choose(TAB_GROUP, 6), cx);
        shell.update_live(
            |model| {
                let plugin = |index: usize| lumilio_core::PluginInfo {
                    manifest: lumilio_plugin_api::Manifest {
                        id: format!("test.p{index}"),
                        name: lumilio_plugin_api::Words::new(
                            format!("插件 {index}"),
                            format!("Plugin {index}"),
                        ),
                        description: lumilio_plugin_api::Words::default(),
                        version: "1".into(),
                        api: lumilio_plugin_api::API_VERSION,
                        default_enabled: true,
                        // Long enough that the detail has to scroll.
                        permissions: (0..40)
                            .map(|folder| Permission::ReadGameFiles {
                                under: format!("folder{folder}"),
                            })
                            .collect(),
                        settings: Vec::new(),
                    },
                    state: Default::default(),
                    status: lumilio_core::PluginStatus::Enabled,
                };
                model.settings = Some(SettingsView {
                    plugins: (0..40).map(plugin).collect(),
                    ..SettingsView::default()
                });
            },
            cx,
        );
    });
    cx.run_until_parked();

    let wheel = |cx: &mut gpui::VisualTestContext, at: gpui::Point<gpui::Pixels>| {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-200.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
    };
    let at = |cx: &mut gpui::VisualTestContext, selector: &'static str| {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is drawn"))
    };
    let pane = at(cx, "settings-plugin-pane");
    assert!(
        pane.bottom() <= px(600.) + px(1.),
        "the detail is confined to the window ({pane:?})"
    );
    let item = at(cx, "settings-plugin-test.p0");
    let toggle = at(cx, "test.p0-enabled-cap");

    wheel(cx, pane.center());
    assert_eq!(
        at(cx, "settings-plugin-test.p0"),
        item,
        "the list stays put"
    );
    assert!(
        at(cx, "test.p0-enabled-cap").origin.y < toggle.origin.y - px(10.),
        "the detail scrolls"
    );

    wheel(cx, item.center());
    assert!(
        at(cx, "settings-plugin-test.p0").origin.y < item.origin.y - px(10.),
        "the list scrolls on its own"
    );
}
