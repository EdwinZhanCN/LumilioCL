use super::*;
use crate::discover::{ReleaseChannel, TextFormat, VersionFile};

fn item(name: &str, enabled: bool) -> ContentItem {
    ContentItem {
        file_name: name.to_owned(),
        display_name: name.to_owned(),
        enabled,
        size: 1,
        modified: 0,
        is_directory: false,
    }
}

fn version(id: &str, project: &str, number: &str, sha1: &str) -> Version {
    Version {
        id: id.to_owned(),
        project_id: project.to_owned(),
        name: number.to_owned(),
        number: number.to_owned(),
        channel: ReleaseChannel::Release,
        game_versions: vec!["26.3".into()],
        loaders: vec!["fabric".into()],
        published: "2026-09-01T00:00:00Z".into(),
        files: vec![VersionFile {
            url: format!("https://cdn/{id}.jar"),
            filename: format!("{id}.jar"),
            primary: true,
            size: 1,
            sha1: Some(sha1.to_owned()),
            download_page: None,
        }],
        dependencies: Vec::new(),
        downloads: 0,
        changelog: String::new(),
        changelog_format: TextFormat::Markdown,
    }
}

fn recognized(source: &str, version: Version) -> Recognized {
    Recognized {
        source: source.to_owned(),
        source_name: source.to_uppercase(),
        version,
    }
}

#[test]
fn known_files_get_their_project_and_unknown_files_stay_listed() {
    let files = vec![
        (item("apple.jar", true), Some("h1".to_owned())),
        (item("mine.jar", true), Some("h2".to_owned())),
        (item("off.jar", false), Some("h3".to_owned())),
        (item("folder", true), None),
    ];
    let identified = BTreeMap::from([
        (
            "h1".to_owned(),
            recognized("mr", version("v1", "P", "3.0.10", "h1")),
        ),
        (
            "h3".to_owned(),
            recognized("cf", version("v3", "Q", "1.0", "h3")),
        ),
    ]);
    let projects = Summaries::from([(
        ("mr".to_owned(), "P".to_owned()),
        ProjectSummary {
            id: "P".into(),
            slug: "appleskin".into(),
            title: "AppleSkin".into(),
            kind: None,
            icon_url: Some("https://x/icon.png".into()),
            author: Some("squeek502".into()),
            page_url: "https://x/appleskin".into(),
        },
    )]);
    let latest = BTreeMap::from([
        ("h1".to_owned(), version("v9", "P", "3.0.11", "new")),
        ("h3".to_owned(), version("v8", "Q", "2.0", "newer")),
    ]);
    let entries = assemble(files, &identified, &projects, &latest);
    assert_eq!(entries.len(), 4, "every file stays");
    let apple = entries[0].source.as_ref().unwrap();
    assert_eq!(
        (apple.source.as_str(), apple.source_name.as_str()),
        ("mr", "MR")
    );
    assert_eq!(apple.title, "AppleSkin");
    assert_eq!(apple.author.as_deref(), Some("squeek502"));
    assert_eq!(apple.page_url, "https://x/appleskin");
    assert_eq!(apple.version_number, "3.0.10");
    assert_eq!(entries[0].update.as_ref().unwrap().number, "3.0.11");
    assert!(entries[1].source.is_none(), "unknown to every source");
    let off = entries[2].source.as_ref().unwrap();
    assert_eq!(off.source, "cf");
    assert_eq!(
        off.title, "",
        "no summary: the caller falls back to the file"
    );
    assert_eq!(off.slug, "Q");
    assert!(
        entries[2].update.is_none(),
        "disabled files are not offered updates"
    );
    assert!(entries[3].source.is_none() && entries[3].sha1.is_none());
}

#[test]
fn a_summary_from_another_source_is_not_this_project() {
    let files = vec![(item("a.jar", true), Some("h1".to_owned()))];
    let identified = BTreeMap::from([(
        "h1".to_owned(),
        recognized("cf", version("v1", "P", "1", "h1")),
    )]);
    let projects = Summaries::from([(
        ("mr".to_owned(), "P".to_owned()),
        ProjectSummary {
            id: "P".into(),
            slug: "elsewhere".into(),
            title: "Another".into(),
            kind: None,
            icon_url: None,
            author: None,
            page_url: String::new(),
        },
    )]);
    let entries = assemble(files, &identified, &projects, &BTreeMap::new());
    assert_eq!(entries[0].source.as_ref().unwrap().title, "");
}

#[test]
fn the_newest_version_listing_this_very_file_is_not_an_update() {
    let files = vec![(item("a.jar", true), Some("h1".to_owned()))];
    let identified = BTreeMap::from([(
        "h1".to_owned(),
        recognized("mr", version("v1", "P", "1", "h1")),
    )]);
    let latest = BTreeMap::from([("h1".to_owned(), version("v1", "P", "1", "h1"))]);
    let entries = assemble(files, &identified, &Summaries::new(), &latest);
    assert!(entries[0].update.is_none());
}
