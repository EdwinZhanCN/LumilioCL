use super::super::intent::InstanceIntent;
use super::super::{InstanceDetailView, TAB_HISTORY, TAB_SCREENSHOTS, TAB_SETTINGS};
use super::{click, record, rooted};
use gpui::TestAppContext;
use lumilio_core::{LauncherSettings, PluginTab};
use lumilio_plugin_api::{ActionId, ImageData, KeyKind, ListItem, View, Words};
use std::cell::RefCell;
use std::rc::Rc;

const PLUGIN: &str = "lumilio.litematica";

fn tab() -> PluginTab {
    PluginTab {
        plugin: PLUGIN.into(),
        title: Words::new("投影", "Schematics"),
    }
}

fn open(
    seen: Rc<RefCell<Vec<InstanceIntent>>>,
    cx: &mut TestAppContext,
) -> (
    gpui::Entity<InstanceDetailView>,
    &mut gpui::VisualTestContext,
) {
    let (view, cx) = rooted(cx, seen);
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.plugins_changed(vec![PLUGIN.into()], cx);
        view.plugin_tabs_arrived(vec![tab()], cx);
    });
    cx.run_until_parked();
    (view, cx)
}

fn pixel() -> ImageData {
    ImageData {
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
    }
}

fn key(id: &str, destructive: bool) -> View {
    View::Key {
        id: ActionId::new(id),
        label: "清空".into(),
        kind: KeyKind::Primary,
        destructive,
    }
}

fn shown(view: View) -> Result<Option<View>, String> {
    Ok(Some(view))
}

fn actions(seen: &Rc<RefCell<Vec<InstanceIntent>>>) -> Vec<String> {
    seen.borrow()
        .iter()
        .filter_map(|intent| match intent {
            InstanceIntent::PluginAction { action, .. } => Some(action.0.clone()),
            _ => None,
        })
        .collect()
}

#[gpui::test]
fn plugins_collapse_into_one_tab_and_built_in_positions_stay(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    // A second plugin contributes too; the bar still carries one 插件 tab.
    view.update(cx, |view, cx| {
        view.plugins_changed(vec![PLUGIN.into(), "other.plugin".into()], cx);
        view.plugin_tabs_arrived(
            vec![
                tab(),
                PluginTab {
                    plugin: "other.plugin".into(),
                    title: Words::new("地图", "Maps"),
                },
            ],
            cx,
        );
    });
    view.update(cx, |view, _| {
        assert_eq!(
            view.tab_labels(),
            [
                "概览", "内容", "世界", "截图", "插件", "历史", "诊断", "设置"
            ],
            "however many plugins contribute, the bar keeps one tab"
        );
        assert_eq!(view.shown_tab(), 0);
    });
    view.update(cx, |view, cx| view.select_tab(TAB_SETTINGS, cx));
    view.update(cx, |view, _| {
        assert_eq!(view.shown_tab(), 7, "设置 moves right by one tab");
        assert_eq!(view.tab, TAB_SETTINGS, "the built-in index does not change");
    });

    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    assert!(
        seen.borrow()
            .contains(&InstanceIntent::PluginView(PLUGIN.into()))
    );
    view.update(cx, |view, _| assert_eq!(view.shown_tab(), 4));

    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(5, window, cx)));
    view.update(cx, |view, _| {
        assert_eq!(view.tab, TAB_HISTORY);
        assert!(view.plugin_open.is_none());
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(3, window, cx)));
    view.update(cx, |view, _| assert_eq!(view.tab, TAB_SCREENSHOTS));
}

#[gpui::test]
fn a_list_row_and_a_key_send_their_actions(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(
            PLUGIN.into(),
            shown(View::Section {
                title: "投影".into(),
                children: vec![
                    View::List {
                        items: vec![
                            ListItem {
                                id: "a".into(),
                                title: "房子".into(),
                                subtitle: Some("Alex · 2×1×1".into()),
                                value: Some("2 个方块".into()),
                                image: Some(pixel()),
                                tags: vec!["Mod".into()],
                                open: Some(ActionId::new("open:a")),
                            },
                            ListItem {
                                id: "broken".into(),
                                title: "坏了".into(),
                                // An image of the wrong size is left out, not trusted.
                                image: Some(ImageData {
                                    width: 4,
                                    height: 4,
                                    rgba: vec![0; 3],
                                }),
                                ..ListItem::default()
                            },
                        ],
                    },
                    key("refresh", false),
                ],
            }),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("plugin-item-a").is_some());
    assert!(cx.debug_bounds("plugin-item-broken").is_some());

    click(cx, "plugin-item-a");
    click(cx, "plugin-key-refresh");
    click(cx, "plugin-item-broken");
    assert_eq!(
        actions(&seen),
        ["open:a", "refresh"],
        "a row without `open` is inert"
    );
}

#[gpui::test]
fn a_destructive_key_asks_before_sending(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), shown(key("wipe", true)), cx);
    });
    cx.run_until_parked();
    click(cx, "plugin-key-wipe");
    assert!(actions(&seen).is_empty(), "the host asks first");
}

#[gpui::test]
fn every_component_renders_and_a_failed_read_says_so(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(
            PLUGIN.into(),
            shown(View::Detail {
                title: "房子".into(),
                subtitle: Some("Alex".into()),
                image: Some(pixel()),
                facts: vec![("尺寸".into(), "2 × 1 × 1".into())],
                children: vec![
                    View::Table {
                        columns: vec!["方块".into(), "数量".into()],
                        rows: vec![vec!["minecraft:stone".into(), "2".into()]],
                    },
                    View::Tags(vec!["a".into()]),
                    View::Text {
                        text: "说明".into(),
                        tone: lumilio_plugin_api::Tone::Mono,
                    },
                    View::Image(pixel()),
                    View::Empty {
                        title: "空".into(),
                        message: "无".into(),
                    },
                    key("back", false),
                ],
            }),
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("plugin-key-back").is_some());

    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), Err("boom".into()), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("plugin-key-back").is_none());
}

#[gpui::test]
fn turning_the_plugin_off_removes_its_tab_and_leaves_an_open_one(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), shown(key("back", false)), cx);
        view.plugins_changed(Vec::new(), cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, _| {
        assert_eq!(view.tab_labels().len(), 7, "no plugin tab is left");
        assert!(view.plugin_open.is_none(), "back on a built-in tab");
        assert!(view.plugin_pages.is_empty());
    });
    assert!(cx.debug_bounds("plugin-key-back").is_none());
    // A late tab list from before the switch cannot bring the tab back.
    view.update(cx, |view, cx| view.plugin_tabs_arrived(vec![tab()], cx));
    view.update(cx, |view, _| assert_eq!(view.tab_labels().len(), 7));
}

#[gpui::test]
fn a_long_material_list_shares_the_detail_scroll_and_can_return_to_the_inline_header(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(
            PLUGIN.into(),
            shown(View::Detail {
                title: "大投影".into(),
                subtitle: None,
                image: None,
                facts: Vec::new(),
                children: vec![
                    View::Table {
                        columns: vec!["方块".into(), "数量".into()],
                        rows: (0..400)
                            .map(|n| vec![format!("minecraft:block_{n}"), n.to_string()])
                            .collect(),
                    },
                    // Listed after the table, shown before it.
                    key("export", false),
                    View::Model {
                        file: "schematics/house.litematic".into(),
                    },
                ],
            }),
            cx,
        );
    });
    cx.run_until_parked();
    let table = cx.debug_bounds("plugin-table-0").expect("table");
    assert!(table.size.height > gpui::px(360.), "{table:?}");
    let key = cx.debug_bounds("plugin-key-export").expect("key");
    assert!(key.origin.y < table.origin.y, "the key is above the table");
    let title = cx.debug_bounds("plugin-detail-title").expect("title");
    let preview = cx.debug_bounds("model-open").expect("preview entry");
    assert!(f32::from(key.center().y - title.center().y).abs() <= 1.);
    assert!(f32::from(preview.center().y - title.center().y).abs() <= 1.);
    assert!(preview.origin.x >= title.right());
    let floating = cx.debug_bounds("plugin-back-to-top").expect("back to top");
    view.update(cx, |view, cx| {
        view.plugin_pane_scroll
            .set_offset(gpui::point(gpui::px(0.), gpui::px(-400.)));
        cx.notify();
    });
    cx.run_until_parked();
    view.update(cx, |view, _| {
        assert!(view.plugin_pane_scroll.offset().y < gpui::px(0.));
    });
    assert_eq!(
        cx.debug_bounds("plugin-back-to-top")
            .expect("floating button"),
        floating,
        "the button stays anchored while the detail moves"
    );
    click(cx, "plugin-back-to-top");
    view.update(cx, |view, _| {
        assert_eq!(view.plugin_pane_scroll.offset().y, gpui::px(0.));
    });
    assert_eq!(cx.debug_bounds("plugin-detail-title"), Some(title));
}

#[gpui::test]
fn a_model_loads_in_its_modal_once_and_late_assets_do_not_enter_a_replacement(
    cx: &mut TestAppContext,
) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = open(seen.clone(), cx);
    cx.update(|window, cx| view.update(cx, |view, cx| view.open_shown(4, window, cx)));
    let model = View::Model {
        file: "schematics/house.litematic".into(),
    };
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), shown(model.clone()), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-viewport").is_none());
    assert!(
        !seen
            .borrow()
            .iter()
            .any(|intent| matches!(intent, InstanceIntent::LoadModel { .. }))
    );
    click(cx, "model-open");
    assert!(cx.debug_bounds("model-viewport").is_some());
    let requests: Vec<_> = seen
        .borrow()
        .iter()
        .filter_map(|intent| match intent {
            InstanceIntent::LoadModel {
                plugin,
                file,
                request,
            } => Some((plugin.clone(), file.clone(), *request)),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        (&requests[0].0, &requests[0].1),
        (&PLUGIN.to_owned(), &"schematics/house.litematic".to_owned())
    );
    let old_id = requests[0].2;
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), shown(model), cx)
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| {
        view.plugin_model_arrived(old_id, Err("obsolete result".into()), cx)
    });
    cx.run_until_parked();
    click(cx, "model-open");
    assert!(cx.debug_bounds("model-loading").is_some());
    assert!(cx.debug_bounds("model-error").is_none());
    let newest = seen
        .borrow()
        .iter()
        .rev()
        .find_map(|intent| match intent {
            InstanceIntent::LoadModel { request, .. } => Some(*request),
            _ => None,
        })
        .unwrap();
    assert_ne!(newest, old_id);
    view.update(cx, |view, cx| {
        view.plugin_model_arrived(
            newest,
            Ok(lumilio_core::ModelPreview {
                schematic: Vec::new(),
                pack: None,
            }),
            cx,
        )
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("model-error").is_some());
    assert!(cx.debug_bounds("model-loading").is_none());
}
