use super::super::intent::{InstanceIntent, Section};
use super::super::panels::Arrived;
use super::super::{InstanceDetailView, Operated, TAB_CONTENT};
use super::{click, item, known_version, listed, record, rooted};
use crate::toast::Toast;
use gpui::{Modifiers, TestAppContext};
use lumilio_core::{LauncherSettings, ProjectKind};
use std::cell::RefCell;
use std::rc::Rc;

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
