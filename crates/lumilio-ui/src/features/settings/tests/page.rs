//! The page alone in a window: what each tab shows and what its controls ask
//! for, recorded instead of done.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext, px, size};

use super::super::{Settings, TAB_GROUP};
use crate::kit::ViewIntent;
use crate::live::{LiveIntent, SettingsView};

type Seen = Rc<RefCell<Vec<LiveIntent>>>;

/// Settings in a window of this height, with what its controls asked for.
fn page(height: f32, cx: &mut TestAppContext) -> (Entity<Settings>, Seen, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_reduce_motion(true);
    });
    let seen: Seen = Rc::default();
    let sink = seen.clone();
    let slot: Rc<RefCell<Option<Entity<Settings>>>> = Rc::default();
    let keep = slot.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let settings = cx.new(|_| {
            let mut settings = Settings::new(None);
            settings.set_handler(Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)));
            settings
        });
        *keep.borrow_mut() = Some(settings.clone());
        gpui_component::Root::new(settings, window, cx)
    });
    let settings = slot.borrow().clone().expect("settings");
    cx.simulate_resize(size(px(1080.), px(height)));
    (settings, seen, cx)
}

fn tab(settings: &Entity<Settings>, tab: usize, cx: &mut VisualTestContext) {
    settings.update(cx, |settings, cx| {
        settings.apply_view_intent(ViewIntent::Choose(TAB_GROUP, tab), cx)
    });
    cx.run_until_parked();
}

/// Changes the settings shown, as a new reading would.
fn reread(
    settings: &Entity<Settings>,
    change: impl FnOnce(&mut SettingsView),
    cx: &mut VisualTestContext,
) {
    settings.update(cx, |settings, cx| {
        let mut view = settings.settings().cloned().unwrap_or_default();
        change(&mut view);
        settings.show_settings(view, cx);
    });
    cx.run_until_parked();
}

fn plugin(id: String, name: String, permissions: usize) -> lumilio_core::PluginInfo {
    lumilio_core::PluginInfo {
        manifest: lumilio_plugin_api::Manifest {
            id,
            name: lumilio_plugin_api::Words::new(name.clone(), name),
            description: lumilio_plugin_api::Words::new("测试说明", "For tests"),
            version: "1".into(),
            api: lumilio_plugin_api::API_VERSION,
            default_enabled: true,
            permissions: (0..permissions)
                .map(|folder| lumilio_plugin_api::Permission::ReadGameFiles {
                    under: format!("folder{folder}"),
                })
                .collect(),
            settings: Vec::new(),
        },
        state: Default::default(),
        status: lumilio_core::PluginStatus::Enabled,
    }
}

#[gpui::test]
fn the_settings_page_shows_each_tab_and_edits_open_a_dialog(cx: &mut TestAppContext) {
    let (settings, seen, cx) = page(720., cx);
    // Before the settings arrive the page says so instead of showing blanks.
    cx.run_until_parked();
    assert!(cx.debug_bounds("settings-appearance").is_none());

    reread(
        &settings,
        |view| {
            *view = SettingsView {
                max_memory_mb: Some(4096),
                total_memory_mb: Some(16_384),
                data_dir: "/data".into(),
                ..SettingsView::default()
            };
        },
        cx,
    );
    for selector in [
        "settings-after-launch",
        "settings-foreground",
        "settings-motion",
        "settings-language",
        "settings-translation",
        "settings-translation-provider",
        "settings-translation-cache",
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
    for (index, selector) in tabs {
        tab(&settings, index, cx);
        assert!(
            cx.debug_bounds(selector).is_some(),
            "{selector} on tab {index}"
        );
    }

    // Mirror presets send their stable identity; core merges the saved rules.
    tab(&settings, 4, cx);
    for (selector, preset) in [
        ("settings-bmclapi-add", lumilio_core::MirrorPreset::Bmclapi),
        ("settings-mcim-add", lumilio_core::MirrorPreset::Mcim),
        (
            "settings-tencent-maven-add",
            lumilio_core::MirrorPreset::TencentMaven,
        ),
    ] {
        let before = settings.read_with(cx, |settings, _| {
            settings.settings().expect("read").mirrors.clone()
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
        reread(&settings, |view| view.mirrors = expected, cx);
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
        reread(&settings, |view| view.download_source = preference, cx);
    }

    // The plugin row sends the stable plugin ID, with its requested new state.
    tab(&settings, 6, cx);
    reread(
        &settings,
        |view| view.plugins = vec![plugin("test.fake".into(), "测试插件".into(), 0)],
        cx,
    );
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
    tab(&settings, 2, cx);
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
    reread(
        &settings,
        |view| view.preferences.look.mono_font = Some("Menlo".into()),
        cx,
    );
    tab(&settings, 1, cx);
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
    use gpui::{ScrollDelta, ScrollWheelEvent, TouchPhase, point};

    let (settings, _, cx) = page(600., cx);
    tab(&settings, 6, cx);
    reread(
        &settings,
        |view| {
            // Long enough that the detail has to scroll.
            view.plugins = (0..40)
                .map(|index| plugin(format!("test.p{index}"), format!("插件 {index}"), 40))
                .collect();
        },
        cx,
    );

    let wheel = |cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>| {
        cx.simulate_event(ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(-200.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        cx.run_until_parked();
    };
    let at = |cx: &mut VisualTestContext, selector: &'static str| {
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
