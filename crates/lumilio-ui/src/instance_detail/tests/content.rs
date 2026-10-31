use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Modifiers, TestAppContext};
use lumilio_core::{LauncherSettings, ProjectKind};

use super::super::intent::{InstanceIntent, Section};
use super::super::panels::Arrived;
use super::super::{InstanceDetailView, Operated, TAB_CONTENT};
use super::{click, item, known_version, listed, record, rooted};
use crate::toast::Toast;

/// A long version shares a row with the Update key and may not run
/// underneath it. The newer version is not in the row: the Update key
/// opens the dialog that names it, and its tooltip says it. The Update key
/// is an icon key as wide as ⇆, so a row with an update keeps its version
/// column in line with a row without one.
#[gpui::test]
fn a_long_version_stays_clear_of_the_update_key(cx: &mut TestAppContext) {
    let (view, cx) = rooted(cx, Rc::default());
    let mut fabric = record();
    fabric.loader = lumilio_core::Loader::Fabric;
    fabric.game_version = "26.3".into();
    let newest = known_version("v2", "1.11.7+26.3", "26.3");
    let mut list = listed(vec![
        item("iris-fabric-1.11.4+mc26.2.jar", true),
        item("sodium-fabric-0.8.jar", true),
    ]);
    let known = |slug: &str, title: &str, number: &str| lumilio_core::ContentSource {
        source: lumilio_core::DEFAULT_SOURCE.into(),
        source_name: "Modrinth".into(),
        page_url: String::new(),
        project_id: "P".into(),
        slug: slug.into(),
        title: title.into(),
        author: Some("coderbot".into()),
        icon_url: None,
        version_id: "v1".into(),
        version_number: number.into(),
    };
    list.entries[0].source = Some(known(
        "iris",
        "Iris Shaders",
        "1.11.4+26.2-fabric-with-a-long-suffix",
    ));
    list.entries[0].update = Some(newest);
    list.entries[1].source = Some(known("sodium", "Sodium", "0.8"));
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
    let switch = cx
        .debug_bounds("content-switch-version-1")
        .expect("⇆ on the row without an update");
    assert_eq!(update.size, switch.size, "Update is as wide as ⇆");
    assert_eq!(update.size.width, update.size.height, "an icon key");
    let other = cx
        .debug_bounds("content-version-1")
        .expect("the other row's version column");
    assert_eq!(
        version.left(),
        other.left(),
        "both rows' version columns start at one place"
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
        source: lumilio_core::DEFAULT_SOURCE.into(),
        source_name: "Modrinth".into(),
        page_url: String::new(),
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
        Some(&InstanceIntent::LoadVersions {
            source: lumilio_core::DEFAULT_SOURCE.into(),
            project: "P".into(),
        })
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
            source: lumilio_core::DEFAULT_SOURCE.into(),
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
    view.read_with(cx, |view, cx| {
        assert!(!view.busy);
        assert!(matches!(&view.data.content[0], Some(Ok(list)) if list.entries.len() == 2));
        assert_eq!(
            crate::toast::posted(cx),
            [Toast::error("没有成功").technical("disk full")],
            "the failure floats up; the page keeps its list"
        );
    });
}

/// After a switch the files on disk show at once (#126): the switched file
/// keeps what the source said about it, and the newest version waits for
/// the sources, which see only enabled files. A write that finishes while a
/// read is on its way reads again.
#[gpui::test]
fn a_switched_file_shows_at_once_and_keeps_its_source(cx: &mut TestAppContext) {
    let (view, cx) = rooted(cx, Rc::default());
    let mut list = listed(vec![item("apple.jar", true), item("mine.jar", true)]);
    list.entries[0].sha1 = Some("a1".into());
    list.entries[0].source = Some(lumilio_core::ContentSource {
        source: lumilio_core::DEFAULT_SOURCE.into(),
        source_name: "Modrinth".into(),
        page_url: String::new(),
        project_id: "P".into(),
        slug: "appleskin".into(),
        title: "AppleSkin".into(),
        author: Some("squeek502".into()),
        icon_url: None,
        version_id: "v1".into(),
        version_number: "3.0.10".into(),
    });
    list.entries[0].update = Some(known_version("v2", "3.0.11", "1.21.1"));
    let mods = Section::Content(ProjectKind::Mod);
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list.clone())), cx);
    });
    cx.update(|window, cx| view.update(cx, |view, cx| view.request(mods, window, cx)));
    let stale = view.update(cx, |view, cx| {
        view.operated(
            Operated {
                notice: "已停用 1 个文件".into(),
                technical: None,
                refresh: vec![mods],
            },
            cx,
        )
    });
    assert!(stale.contains(&mods), "a read on its way is read again");

    view.update(cx, |view, cx| {
        view.arrived(
            Arrived::ContentSoFar(
                ProjectKind::Mod,
                listed(vec![item("apple.jar", false), item("mine.jar", true)]).entries,
            ),
            cx,
        )
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        let Some(Ok(shown)) = &view.data.content[0] else {
            panic!("the files are listed");
        };
        let apple = &shown.entries[0];
        assert_eq!(apple.item.file_name, "apple.jar.disabled");
        assert_eq!(apple.source, list.entries[0].source);
        assert_eq!(apple.sha1.as_deref(), Some("a1"));
        assert_eq!(apple.update, None, "a disabled file is not offered updates");
        assert!(shown.entries[1].source.is_none());
        assert!(view.data.identifying(ProjectKind::Mod));
    });
    assert!(
        cx.debug_bounds("content-identifying").is_some(),
        "the toolbar says the sources are being asked"
    );

    let mut answered = list;
    answered.entries[0].item = item("apple.jar", false);
    answered.entries[0].update = None;
    view.update(cx, |view, cx| {
        view.arrived(Arrived::Content(ProjectKind::Mod, Ok(answered)), cx)
    });
    cx.run_until_parked();
    view.read_with(cx, |view, _| {
        assert!(!view.data.identifying(ProjectKind::Mod))
    });
    assert!(cx.debug_bounds("content-identifying").is_none());
}

fn pending(path: &str) -> lumilio_core::PendingDownload {
    lumilio_core::PendingDownload {
        title: "Corail Tombstone".into(),
        file_name: "tombstone.jar".into(),
        path: path.into(),
        sha1: Some("aa".into()),
        size: 7,
        page: Some("https://www.curseforge.com/minecraft/mc-mods/tombstone/files/1".into()),
    }
}

#[gpui::test]
fn files_only_the_website_hands_out_are_listed_with_a_way_to_add_them(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    view.update(cx, |view, cx| {
        view.loaded(Ok((record(), LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(
            Arrived::Content(ProjectKind::Mod, Ok(listed(vec![item("a.jar", true)]))),
            cx,
        );
        view.set_pending(
            vec![
                pending("mods/tombstone.jar"),
                pending("resourcepacks/x.zip"),
            ],
            cx,
        );
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("content-pending").is_some());
    assert!(
        cx.debug_bounds("content-pending-page-0").is_some(),
        "its download page"
    );
    assert!(
        cx.debug_bounds("content-pending-page-1").is_none(),
        "a resource pack waits on its own tab"
    );
    click(cx, "content-pending-add");
    assert_eq!(seen.borrow().last(), Some(&InstanceIntent::AddPendingFiles));

    view.update(cx, |view, cx| view.set_pending(Vec::new(), cx));
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("content-pending").is_none(),
        "nothing left to add"
    );
}

#[gpui::test]
fn a_changelog_given_one_at_a_time_is_asked_for_and_shown(cx: &mut TestAppContext) {
    let seen: Rc<RefCell<Vec<InstanceIntent>>> = Rc::default();
    let (view, cx) = rooted(cx, seen.clone());
    let mut fabric = record();
    fabric.loader = lumilio_core::Loader::Fabric;
    fabric.game_version = "26.3".into();
    let mut list = listed(vec![item("apple.jar", true)]);
    list.entries[0].source = Some(lumilio_core::ContentSource {
        source: "lumilio.curseforge".into(),
        source_name: "CurseForge".into(),
        page_url: String::new(),
        project_id: "P".into(),
        slug: "P".into(),
        title: "AppleSkin".into(),
        author: None,
        icon_url: None,
        version_id: "v1".into(),
        version_number: "3.0.10".into(),
    });
    view.update(cx, |view, cx| {
        view.loaded(Ok((fabric, LauncherSettings::default())), cx);
        view.select_tab(TAB_CONTENT, cx);
        view.arrived(Arrived::Content(ProjectKind::Mod, Ok(list)), cx);
    });
    click(cx, "content-switch-version-0");
    assert_eq!(
        seen.borrow().last(),
        Some(&InstanceIntent::LoadVersions {
            source: "lumilio.curseforge".into(),
            project: "P".into(),
        })
    );
    let mut quiet = known_version("v1", "3.0.10", "26.3");
    quiet.changelog = String::new();
    quiet.changelog_format = lumilio_core::TextFormat::Html;
    let wanted = view.update(cx, |view, cx| {
        view.versions_arrived("P", Ok(vec![quiet]), cx)
    });
    assert_eq!(
        wanted,
        Some(("lumilio.curseforge".into(), "P".into(), "v1".into())),
        "the chosen version's changelog is asked for"
    );
    let again = view.update(cx, |view, cx| {
        let mut quiet = known_version("v1", "3.0.10", "26.3");
        quiet.changelog = String::new();
        view.versions_arrived("P", Ok(vec![quiet]), cx)
    });
    assert_eq!(again, None, "once");
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("switch-changelog-text").is_none(),
        "none yet"
    );
    view.update(cx, |view, cx| {
        view.changelog_arrived("P", "v1", Ok("<p>Fixed crash</p>".into()), cx)
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("switch-changelog-text").is_some());
}
