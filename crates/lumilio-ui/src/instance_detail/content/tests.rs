use super::model::ContentFilter;
use super::model::title_of;
use super::model::visible;
use lumilio_core::{ContentEntry, ReleaseChannel, Version};

use lumilio_core::{ContentItem, ContentSource};

fn entry(name: &str, enabled: bool, title: Option<&str>, update: bool) -> ContentEntry {
    ContentEntry {
        item: ContentItem {
            file_name: name.to_owned(),
            display_name: name.trim_end_matches(".disabled").to_owned(),
            enabled,
            size: 1,
            modified: 0,
            is_directory: false,
        },
        sha1: Some("h".into()),
        source: title.map(|title| ContentSource {
            project_id: "P".into(),
            slug: "p".into(),
            title: title.to_owned(),
            author: Some("squeek502".into()),
            icon_url: None,
            version_id: "v1".into(),
            version_number: "1".into(),
        }),
        update: update.then(|| Version {
            id: "v2".into(),
            project_id: "P".into(),
            name: "2".into(),
            number: "2".into(),
            channel: ReleaseChannel::Release,
            game_versions: vec![],
            loaders: vec![],
            published: String::new(),
            files: vec![],
            dependencies: vec![],
            downloads: 0,
            changelog: String::new(),
        }),
    }
}

#[test]
fn filters_and_search_narrow_the_list() {
    let entries = [
        entry("appleskin.jar", true, Some("AppleSkin"), true),
        entry("mine.jar", true, None, false),
        entry("off.jar.disabled", false, Some("Off"), false),
    ];
    let titles = |shown: Vec<&ContentEntry>| {
        shown
            .iter()
            .map(|e| title_of(e).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(visible(&entries, "", ContentFilter::All).len(), 3);
    assert_eq!(
        titles(visible(&entries, "", ContentFilter::Updates)),
        ["AppleSkin"]
    );
    assert_eq!(
        titles(visible(&entries, "", ContentFilter::Disabled)),
        ["Off"]
    );
    assert_eq!(
        titles(visible(&entries, "", ContentFilter::Unknown)),
        ["mine.jar"]
    );
    assert_eq!(
        titles(visible(&entries, "apple", ContentFilter::All)),
        ["AppleSkin"]
    );
    assert_eq!(
        visible(&entries, "squeek", ContentFilter::All).len(),
        2,
        "the author matches too"
    );
    assert_eq!(
        titles(visible(&entries, "MINE", ContentFilter::All)),
        ["mine.jar"]
    );
}
