//! Where installed content came from (IA `instance/content.md`, P-CONTENT-ITEM):
//! each file is looked up with the enabled content sources to name its
//! project, author, icon and version, and to say whether a newer compatible
//! version exists.
//!
//! Assembly is pure; the service does the hashing and the requests. Not knowing
//! a source never hides a file.

use std::collections::BTreeMap;

use crate::content::ContentItem;
use crate::content::discover::{ProjectSummary, Version};

/// The project and version an installed file is, in the source that knew it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentSource {
    /// The content source (plugin ID) that recognized the file.
    pub source: String,
    /// That source's name, for saying where the file is from.
    pub source_name: String,
    pub project_id: String,
    pub slug: String,
    /// The project's name; empty when only the version could be identified.
    pub title: String,
    pub author: Option<String>,
    pub icon_url: Option<String>,
    pub version_id: String,
    pub version_number: String,
    /// The project's page on the source's site; empty when unknown.
    pub page_url: String,
}

/// One installed file and what is known about it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentEntry {
    pub item: ContentItem,
    /// `None` for folder packs and unreadable files.
    pub sha1: Option<String>,
    /// `None` when no source knows the file (or none could be asked).
    pub source: Option<ContentSource>,
    /// The newest compatible version, when it differs from this file. Only
    /// for enabled files: a disabled file is the user's decision.
    pub update: Option<Version>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContentList {
    pub entries: Vec<ContentEntry>,
    /// A content source could not be asked: files it might know are
    /// unknown, not absent.
    pub sources_unavailable: bool,
    /// Why (no source on, a source stopped, no answer).
    pub source_note: Option<String>,
}

/// What one source said about an installed file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Recognized {
    pub source: String,
    pub source_name: String,
    pub version: Version,
}

/// Project summaries by source and project ID.
pub type Summaries = BTreeMap<(String, String), ProjectSummary>;

/// Puts files, identifications, project summaries, authors and newest
/// versions together, keeping the files' order.
#[must_use]
pub fn assemble(
    files: Vec<(ContentItem, Option<String>)>,
    identified: &BTreeMap<String, Recognized>,
    projects: &Summaries,
    latest: &BTreeMap<String, Version>,
) -> Vec<ContentEntry> {
    files
        .into_iter()
        .map(|(item, sha1)| {
            let known = sha1.as_ref().and_then(|hash| identified.get(hash));
            let source = known.map(|known| {
                let version = &known.version;
                let project = projects.get(&(known.source.clone(), version.project_id.clone()));
                ContentSource {
                    source: known.source.clone(),
                    source_name: known.source_name.clone(),
                    project_id: version.project_id.clone(),
                    slug: project.map_or_else(|| version.project_id.clone(), |p| p.slug.clone()),
                    title: project.map(|p| p.title.clone()).unwrap_or_default(),
                    author: project.and_then(|p| p.author.clone()),
                    icon_url: project.and_then(|p| p.icon_url.clone()),
                    version_id: version.id.clone(),
                    version_number: version.number.clone(),
                    page_url: project.map(|p| p.page_url.clone()).unwrap_or_default(),
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
mod tests;
