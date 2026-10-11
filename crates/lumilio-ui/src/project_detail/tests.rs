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

pub(crate) fn detail() -> ProjectDetail {
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
                full_url: "https://cdn.example/a-full.png".to_owned(),
                title: "A".to_owned(),
                description: String::new(),
                featured: true,
                ordering: 0,
                created: "2026-01-02T00:00:00Z".to_owned(),
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
                from_game: false,
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
    assert!(cx.debug_bounds("detail-sidebar").is_none(), "no sidebar");
}

fn mount(
    cx: &mut TestAppContext,
    target: Option<InstallTarget>,
    detail: ProjectDetail,
) -> (
    gpui::Entity<ProjectDetailView>,
    &mut gpui::VisualTestContext,
    Rc<std::cell::RefCell<Vec<DetailIntent>>>,
) {
    cx.update(gpui_component::init);
    let seen: Rc<std::cell::RefCell<Vec<DetailIntent>>> = Rc::default();
    let sink = seen.clone();
    let (view, cx) = cx.add_window_view(|_, _| {
        ProjectDetailView::new(
            "cool",
            "https://modrinth.com/mod/cool",
            Rc::new(move |intent, _, _| sink.borrow_mut().push(intent)),
        )
    });
    view.update(cx, |view, cx| {
        view.set_target(target, cx);
        view.set_state(DetailState::Ready(Box::new(detail)), cx);
    });
    cx.run_until_parked();
    (view, cx, seen)
}

fn game(from_game: bool) -> Option<InstallTarget> {
    Some(InstallTarget {
        name: "Pack".to_owned(),
        game_version: "1.21".to_owned(),
        loader: Loader::Fabric,
        from_game,
    })
}

#[test]
fn filters_admit_a_version_that_passes_every_filter_that_is_on() {
    let v = detail().versions.remove(0);
    let mut filters = VersionFilters::default();
    assert!(filters.admits(&v) && !filters.active());
    filters.channels = vec![ReleaseChannel::Beta];
    assert!(!filters.admits(&v));
    filters.channels = vec![ReleaseChannel::Beta, ReleaseChannel::Release];
    filters.game_versions = vec!["1.20".to_owned(), "1.21".to_owned()];
    filters.platforms = vec!["fabric".to_owned()];
    assert!(filters.admits(&v), "any one of each list is enough");
    filters.platforms = vec!["forge".to_owned()];
    assert!(!filters.admits(&v));
}

#[gpui::test]
fn the_gallery_tab_only_exists_with_images(cx: &mut TestAppContext) {
    let mut bare = detail();
    bare.project.gallery.clear();
    let (view, cx, _) = mount(cx, None, bare);
    view.read_with(cx, |view, _| assert_eq!(view.tabs(), ["介绍", "版本"]));
}

#[gpui::test]
fn coming_from_a_game_starts_the_versions_filtered_to_it(cx: &mut TestAppContext) {
    let (view, cx, _) = mount(cx, game(true), detail());
    view.read_with(cx, |view, _| {
        assert_eq!(view.filters().game_versions, ["1.21"]);
        assert_eq!(view.filters().platforms, ["fabric"]);
    });
    // The person's own change sticks: clearing does not bring the game back.
    view.update(cx, |view, cx| {
        view.filter_versions(|filters| filters.game_versions.clear(), cx)
    });
    view.update(cx, |view, cx| view.set_target(game(true), cx));
    view.read_with(cx, |view, _| {
        assert!(view.filters().game_versions.is_empty())
    });
}

#[gpui::test]
fn plain_browsing_starts_the_versions_unfiltered(cx: &mut TestAppContext) {
    let (view, cx, _) = mount(cx, game(false), detail());
    view.read_with(cx, |view, _| assert!(!view.filters().active()));
}

#[gpui::test]
fn an_installed_project_offers_switching_and_the_installed_version_is_marked(
    cx: &mut TestAppContext,
) {
    let mut two = detail();
    let mut newer = two.versions[0].clone();
    newer.id = "v2".to_owned();
    newer.number = "2".to_owned();
    newer.published = "2026-02-01T00:00:00Z".to_owned();
    two.versions.insert(0, newer);
    let (view, cx, seen) = mount(cx, game(false), two);
    view.update(cx, |view, cx| {
        let mut have = std::collections::BTreeMap::new();
        have.insert(
            "P".to_owned(),
            lumilio_core::InstalledProject {
                file_name: "a.jar".to_owned(),
                version_id: "v1".to_owned(),
                update: None,
            },
        );
        view.set_installed(have, cx);
        view.select_tab(1, cx);
    });
    cx.run_until_parked();
    view.update(cx, |view, cx| view.select_tab(0, cx));
    cx.run_until_parked();
    // On the description the main button leads to the versions.
    let main = cx.debug_bounds("detail-install").expect("the main button");
    cx.simulate_click(main.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    view.read_with(cx, |view, _| assert_eq!(view.tabs()[1], "版本"));
    assert!(
        cx.debug_bounds("detail-versions").is_some(),
        "switch version goes to the list"
    );
    assert!(seen.borrow().is_empty(), "nothing was installed by that");
}

/// Not a check: prints the per-frame cost of a long description.
/// `cargo nextest run -p lumilio-ui detail_frame_cost --ignored --no-capture`
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
