//! Where installed content came from (IA `instance/content.md`, P-CONTENT-ITEM):
//! each file's SHA-1 is looked up on Modrinth to name its project, author,
//! icon and version, and to say whether a newer compatible version exists.
//!
//! Assembly is pure; the service does the hashing and the requests. Not knowing
//! a source never hides a file.

use std::collections::BTreeMap;

use crate::content::ContentItem;
use crate::discover::{ProjectSummary, Version};

/// The Modrinth project and version an installed file is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSource {
    pub project_id: String,
    pub slug: String,
    /// The project's name; empty when only the version could be identified.
    pub title: String,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    pub version_id: String,
    pub version_number: String,
}

/// One installed file and what is known about it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEntry {
    pub item: ContentItem,
    /// `None` for folder packs and unreadable files.
    pub sha1: Option<String>,
    /// `None` when Modrinth does not know the file (or was not reachable).
    pub source: Option<ContentSource>,
    /// The newest compatible version, when it differs from this file. Only
    /// for enabled files: a disabled file is the user's decision.
    pub update: Option<Version>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentList {
    pub entries: Vec<ContentEntry>,
    /// The content source could not be asked: missing sources are unknown,
    /// not absent.
    pub sources_unavailable: bool,
    /// Why it could not be asked (no source on, source stopped, no answer).
    pub source_note: Option<String>,
}

/// Puts files, identifications, project summaries, authors and newest
/// versions together, keeping the files' order.
#[must_use]
pub fn assemble(
    files: Vec<(ContentItem, Option<String>)>,
    identified: &BTreeMap<String, Version>,
    projects: &[ProjectSummary],
    latest: &BTreeMap<String, Version>,
) -> Vec<ContentEntry> {
    files
        .into_iter()
        .map(|(item, sha1)| {
            let version = sha1.as_ref().and_then(|hash| identified.get(hash));
            let source = version.map(|version| {
                let project = projects
                    .iter()
                    .find(|project| project.id == version.project_id);
                ContentSource {
                    project_id: version.project_id.clone(),
                    slug: project.map_or_else(|| version.project_id.clone(), |p| p.slug.clone()),
                    title: project.map(|p| p.title.clone()).unwrap_or_default(),
                    author: project.and_then(|p| p.author.clone()),
                    icon_url: project.and_then(|p| p.icon_url.clone()),
                    version_id: version.id.clone(),
                    version_number: version.number.clone(),
                }
            });
            let update = match (&sha1, item.enabled) {
                (Some(hash), true) if identified.contains_key(hash) => latest
                    .get(hash)
                    .filter(|newest| {
                        !newest
                            .files
                            .iter()
                            .any(|file| file.sha1.as_deref() == Some(hash.as_str()))
                    })
                    .cloned(),
                _ => None,
            };
            ContentEntry {
                item,
                sha1,
                source,
                update,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discover::{ReleaseChannel, VersionFile};

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
            }],
            dependencies: Vec::new(),
            downloads: 0,
            changelog: String::new(),
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
            ("h1".to_owned(), version("v1", "P", "3.0.10", "h1")),
            ("h3".to_owned(), version("v3", "Q", "1.0", "h3")),
        ]);
        let projects = vec![ProjectSummary {
            id: "P".into(),
            slug: "appleskin".into(),
            title: "AppleSkin".into(),
            kind: None,
            icon_url: Some("https://x/icon.png".into()),
            author: Some("squeek502".into()),
        }];
        let latest = BTreeMap::from([
            ("h1".to_owned(), version("v9", "P", "3.0.11", "new")),
            ("h3".to_owned(), version("v8", "Q", "2.0", "newer")),
        ]);
        let entries = assemble(files, &identified, &projects, &latest);
        assert_eq!(entries.len(), 4, "every file stays");
        let apple = entries[0].source.as_ref().unwrap();
        assert_eq!(apple.title, "AppleSkin");
        assert_eq!(apple.author.as_deref(), Some("squeek502"));
        assert_eq!(apple.version_number, "3.0.10");
        assert_eq!(entries[0].update.as_ref().unwrap().number, "3.0.11");
        assert!(entries[1].source.is_none(), "unknown to Modrinth");
        let off = entries[2].source.as_ref().unwrap();
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
    fn the_newest_version_listing_this_very_file_is_not_an_update() {
        let files = vec![(item("a.jar", true), Some("h1".to_owned()))];
        let identified = BTreeMap::from([("h1".to_owned(), version("v1", "P", "1", "h1"))]);
        let latest = BTreeMap::from([("h1".to_owned(), version("v1", "P", "1", "h1"))]);
        let entries = assemble(files, &identified, &[], &latest);
        assert!(entries[0].update.is_none());
    }
}
