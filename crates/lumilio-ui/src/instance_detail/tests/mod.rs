mod content;
mod diagnostics;
mod history;
mod plugins;
mod screenshots;
mod servers;
mod settings;
mod view;
mod worlds;

use super::InstanceDetailView;
use super::intent::InstanceIntent;
use gpui::prelude::*;
use gpui::{Entity, Modifiers, TestAppContext};
use lumilio_core::{InstanceRecord, InstanceSettings, Loader};
use std::cell::RefCell;
use std::rc::Rc;

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
        source_project: None,
        settings: InstanceSettings {
            java_path: Some("/jdk/bin/java".into()),
            jvm_arguments: vec!["-Dx=y".into()],
            ..InstanceSettings::default()
        },
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
                Rc::new(move |intent, _, _| {
                    // Asking which plugin tabs show happens on every start;
                    // the plugin tests look at it, the others do not.
                    if intent != InstanceIntent::PluginTabs {
                        seen.borrow_mut().push(intent);
                    }
                }),
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
