//! What the window holds on to: pictures within their budget, and pages that
//! leave the screen letting go of their plugin pages.

use super::super::LauncherShell;
use super::continuing;
use crate::instance_detail::{Arrived, InstanceIntent, TAB_SCREENSHOTS};
use gpui::{App, ImageSource, Resource, TestAppContext};
use lumilio_core::{
    InstanceRecord, InstanceSettings, LauncherSettings, Loader, PluginTab, ScreenshotInfo,
};
use lumilio_plugin_api::{ImageData, View, Words};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Whether GPUI's own table holds it: that table never lets a picture go.
fn kept_forever(resource: &Resource, cx: &App) -> bool {
    ImageSource::Resource(resource.clone()).is_asset_cached(cx)
}

fn record() -> InstanceRecord {
    InstanceRecord {
        id: "one".into(),
        name: "生存".into(),
        game_version: "1.21.1".into(),
        loader: Loader::Vanilla,
        loader_version: None,
        favorite: false,
        created_at: 1,
        last_played: None,
        play_seconds: 0,
        installed: true,
        settings: InstanceSettings::default(),
        source_project: None,
    }
}

#[gpui::test]
fn pictures_in_the_window_go_through_the_picture_budget(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    let view = shell.update(cx, |shell, cx| {
        shell.open_live_instance("one".into(), |_| Rc::new(|_, _, _| {}), cx)
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_SCREENSHOTS, cx);
        view.arrived(
            Arrived::Screenshots(Ok(vec![ScreenshotInfo {
                file: "a.png".into(),
                path: "/game/screenshots/a.png".into(),
                modified_ms: 1,
                size: 1,
            }])),
            cx,
        );
    });
    cx.run_until_parked();
    let thumbnail = PathBuf::from("/thumbnails/a.png");
    view.update(cx, |view, cx| {
        view.thumbnail_arrived("a.png".into(), Ok(thumbnail.clone()), cx)
    });
    cx.run_until_parked();
    let resource = Resource::Path(thumbnail.into());
    cx.update(|_, cx| {
        assert!(crate::pictures::pages(cx).read(cx).contains(&resource));
        assert!(!kept_forever(&resource, cx));
    });
}

#[gpui::test]
fn the_wallpaper_keeps_to_its_own_cache(cx: &mut TestAppContext) {
    let path = PathBuf::from("/pictures/wallpaper.png");
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(crate::theme::Wallpaper {
            path: Some(path.clone()),
            ..Default::default()
        });
    });
    let (_shell, cx) = cx.add_window_view(|_, cx| {
        LauncherShell::new(cx)
            .with_home(continuing(), cx)
            .with_intent_handler(Rc::new(|_, _, _| {}))
    });
    cx.run_until_parked();
    let resource = Resource::Path(path.into());
    cx.update(|_, cx| {
        assert!(crate::pictures::wallpaper(cx).read(cx).contains(&resource));
        assert!(!crate::pictures::pages(cx).read(cx).contains(&resource));
        assert!(!kept_forever(&resource, cx));
    });
}

#[gpui::test]
fn leaving_a_game_page_drops_its_plugin_page_and_back_asks_for_it_again(cx: &mut TestAppContext) {
    const PLUGIN: &str = "lumilio.litematica";
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let sink = seen.clone();
    let view = shell.update(cx, |shell, cx| {
        shell.open_live_instance(
            "one".into(),
            move |_| Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
            cx,
        )
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.plugins_changed(vec![PLUGIN.into()], cx);
        view.plugin_tabs_arrived(
            vec![PluginTab {
                plugin: PLUGIN.into(),
                title: Words::new("投影", "Schematics"),
            }],
            cx,
        );
    });
    cx.run_until_parked();
    let asked = || {
        seen.borrow()
            .iter()
            .filter(|intent| matches!(intent, InstanceIntent::PluginView(_)))
            .count()
    };
    let tab = cx
        .debug_bounds("live-instance-tabs-tab-4")
        .expect("the plugins tab");
    cx.simulate_click(tab.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(asked(), 1);
    let pixel = ImageData {
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
    };
    view.update(cx, |view, cx| {
        view.plugin_view_arrived(PLUGIN.into(), Ok(Some(View::Image(pixel))), cx)
    });
    cx.run_until_parked();

    shell.update(cx, |shell, cx| {
        shell.open_live_instance("two".into(), |_| Rc::new(|_, _, _| {}), cx);
    });
    cx.run_until_parked();
    assert_eq!(asked(), 1, "a page off screen asks for nothing");
    let back = cx.debug_bounds("navigation-back").expect("back button");
    cx.simulate_click(back.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        asked(),
        2,
        "its plugin page was let go, so it is asked for again"
    );
}

#[gpui::test]
fn a_game_starting_drops_the_pages_kept_for_back(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (shell, cx) =
        cx.add_window_view(|_, cx| LauncherShell::new(cx).with_live(Rc::new(|_, _, _| {})));
    let game = shell.update(cx, |shell, cx| {
        shell
            .open_live_instance("one".into(), |_| Rc::new(|_, _, _| {}), cx)
            .downgrade()
    });
    cx.update(|window, cx| {
        shell.update(cx, |shell, cx| {
            shell.go_to(crate::route::Route::Library, window, cx)
        })
    });
    cx.run_until_parked();
    assert!(game.upgrade().is_some(), "kept for Back");

    shell.update(cx, |shell, cx| {
        shell.apply_launch_signal(lumilio_core::LaunchSignal::Running, cx)
    });
    cx.run_until_parked();
    assert!(game.upgrade().is_none(), "the game page is gone");
    shell.read_with(cx, |shell, _| {
        assert!(
            shell
                .history
                .iter()
                .all(|location| matches!(location, super::super::locations::Location::Page(_)))
        );
        assert!(shell.can_go_back(), "the landmark pages stay");
    });
}
