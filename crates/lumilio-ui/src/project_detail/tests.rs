use super::*;

#[test]
fn sizes_read_naturally() {
    assert_eq!(size_label(37), "37 B");
    assert_eq!(size_label(2048), "2 KB");
    assert_eq!(size_label(1_500_000), "1.4 MB");
    assert_eq!(size_label(3 * 1024 * 1024 * 1024), "3.0 GB");
}

#[test]
fn dates_are_days_or_nothing() {
    assert_eq!(date_label("2024-01-05T10:00:00Z"), "2024-01-05");
    assert_eq!(date_label("soon"), "");
    assert_eq!(date_label(""), "");
}

#[test]
fn version_lists_fold_after_three() {
    let v = |n: &[&str]| n.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(versions_label(&v(&["1.21"])), "1.21");
    assert_eq!(
        versions_label(&v(&["1.21", "1.20", "1.19"])),
        "1.21, 1.20, 1.19"
    );
    assert_eq!(
        versions_label(&v(&["1.21", "1.20", "1.19", "1.18", "1.17"])),
        "1.21, 1.20, 1.19 +2"
    );
    assert_eq!(versions_label(&[]), "");
}

use gpui::TestAppContext;
use lumilio_core::{GalleryImage, ProjectLinks, SideSupport, VersionFile};

fn detail() -> ProjectDetail {
    ProjectDetail {
        project: Project {
            id: "P".to_owned(),
            slug: "cool".to_owned(),
            title: "Cool".to_owned(),
            description: "summary".to_owned(),
            body: "# Hello".to_owned(),
            kind: ProjectKind::Mod,
            categories: vec!["magic".to_owned()],
            loaders: vec!["fabric".to_owned()],
            downloads: 10,
            followers: 2,
            published: String::new(),
            updated: "2026-01-01T00:00:00Z".to_owned(),
            client_side: SideSupport::Required,
            server_side: SideSupport::Unsupported,
            license: Some("MIT".to_owned()),
            links: ProjectLinks::default(),
            gallery: vec![GalleryImage {
                url: "https://cdn.example/a.png".to_owned(),
                title: "A".to_owned(),
                description: String::new(),
                featured: true,
                ordering: 0,
            }],
            icon_url: None,
            game_versions: vec!["1.21".to_owned()],
        },
        versions: vec![Version {
            id: "v1".to_owned(),
            project_id: "P".to_owned(),
            name: "Cool 1".to_owned(),
            number: "1".to_owned(),
            channel: ReleaseChannel::Release,
            game_versions: vec!["1.21".to_owned()],
            loaders: vec!["fabric".to_owned()],
            published: "2026-01-01T00:00:00Z".to_owned(),
            files: vec![VersionFile {
                url: "https://cdn.example/a.jar".to_owned(),
                filename: "a.jar".to_owned(),
                primary: true,
                size: 2048,
                sha1: None,
            }],
            dependencies: Vec::new(),
            downloads: 5,
            changelog: String::new(),
        }],
        owner: Some("someone".to_owned()),
    }
}

#[gpui::test]
fn each_tab_shows_its_own_content_and_versions_install_what_was_chosen(cx: &mut TestAppContext) {
    use std::cell::RefCell;
    cx.update(gpui_component::init);
    let seen: Rc<RefCell<Vec<DetailIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        ProjectDetailView::new(
            "cool",
            "https://modrinth.com/mod/cool",
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.set_target(
            Some(InstallTarget {
                name: "Pack".to_owned(),
                game_version: "1.21".to_owned(),
                loader: Loader::Fabric,
            }),
            cx,
        );
        view.set_state(DetailState::Ready(Box::new(detail())), cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("detail-description").is_some());
    assert!(cx.debug_bounds("detail-versions").is_none());

    view.update(cx, |view, cx| view.select_tab(1, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("detail-versions").is_some(), "versions tab");
    assert!(cx.debug_bounds("detail-description").is_none());

    // Saving the listed version as a file asks for exactly that version.
    let save = cx
        .debug_bounds("detail-version-save-0")
        .expect("a version can be saved as a file");
    cx.simulate_click(save.center(), gpui::Modifiers::none());
    assert!(matches!(
        seen.borrow().last(),
        Some(DetailIntent::SaveAs { version_id, file_name, .. })
            if version_id == "v1" && file_name == "a.jar"
    ));

    view.update(cx, |view, cx| view.select_tab(2, cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("detail-gallery").is_some(), "gallery tab");
    assert!(cx.debug_bounds("detail-versions").is_none());
}

/// Not a check: prints the per-frame cost of a long description.
/// `cargo test -p lumilio-ui detail_frame_cost -- --ignored --nocapture`
#[gpui::test]
#[ignore = "a measurement, not a check"]
fn detail_frame_cost_of_a_long_description(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let (view, cx) = cx.add_window_view(|_, _| {
        ProjectDetailView::new(
            "cool",
            "https://modrinth.com/mod/cool",
            Rc::new(|_, _, _| {}),
        )
    });
    let mut detail = detail();
    detail.project.body = (0..120)
        .map(|n| {
            format!(
                "## Section {n}\n\nSome **bold** and _italic_ text with a [link](https://example.com) \
                 and `code`, long enough to wrap across a couple of lines in the window.\n\n\
                 - first item\n- second item\n- third item\n"
            )
        })
        .collect::<String>();
    view.update(cx, |view, cx| {
        view.set_state(DetailState::Ready(Box::new(detail)), cx);
    });
    cx.run_until_parked();
    let frames = 30;
    let start = std::time::Instant::now();
    for _ in 0..frames {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    eprintln!(
        "detail frame: {:?} each over {frames} frames",
        start.elapsed() / frames
    );
}
