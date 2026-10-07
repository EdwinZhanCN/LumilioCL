//! The official Minecraft version catalog.
//!
//! The catalog lists every downloadable game version with its kind, release
//! time, and the address of its release manifest.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::Deserialize;

use crate::fetch::{FetchError, fetch_decoded};
use crate::transfer::{SourceChain, Transport};

/// Where the official catalog lives.
pub const OFFICIAL_CATALOG_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VersionKind {
    Release,
    Snapshot,
    /// Historic alpha/beta builds.
    Old,
    /// Anything the catalog names that this build does not know.
    Other,
}

impl VersionKind {
    fn from_protocol(name: &str) -> Self {
        match name {
            "release" => Self::Release,
            "snapshot" => Self::Snapshot,
            "old_beta" | "old_alpha" => Self::Old,
            _ => Self::Other,
        }
    }
}

/// How a version is presented to someone choosing one: the catalog calls
/// pre-releases and release candidates "snapshots", but people tell them
/// apart. Display and filtering only; [`VersionKind`] stays authoritative.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum VersionChannel {
    Release,
    Snapshot,
    PreRelease,
    Candidate,
    /// Historic alpha/beta builds.
    Old,
}

impl VersionChannel {
    /// The channel a non-release version id names, by the conventions the
    /// game has used: `26.3-rc-1`, `1.16-rc1`, `1.14 Pre-Release 1`,
    /// `26.3-pre-2`, `26.4-snapshot-1`, `25w14a`.
    #[must_use]
    pub fn of_unreleased(id: &str) -> Self {
        let lower = id.to_ascii_lowercase();
        if lower.contains("-rc") || lower.contains("release candidate") {
            Self::Candidate
        } else if lower.contains("-pre") || lower.contains("pre-release") {
            Self::PreRelease
        } else {
            Self::Snapshot
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    id: String,
    kind: VersionKind,
    release_time: String,
    url: String,
    sha1: Option<String>,
}

impl CatalogEntry {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> VersionKind {
        self.kind
    }

    #[must_use]
    pub fn channel(&self) -> VersionChannel {
        match self.kind {
            VersionKind::Release => VersionChannel::Release,
            VersionKind::Old => VersionChannel::Old,
            VersionKind::Snapshot | VersionKind::Other => VersionChannel::of_unreleased(&self.id),
        }
    }

    /// RFC 3339 timestamp, as published.
    #[must_use]
    pub fn release_time(&self) -> &str {
        &self.release_time
    }

    /// Address of this version's release manifest.
    #[must_use]
    pub fn manifest_url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn manifest_sha1(&self) -> Option<&str> {
        self.sha1.as_deref()
    }
}

#[derive(Debug, Deserialize)]
struct RawCatalog {
    #[serde(default)]
    latest: RawLatest,
    versions: Option<Vec<RawEntry>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawLatest {
    release: Option<String>,
    snapshot: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    id: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(rename = "releaseTime")]
    release_time: Option<String>,
    time: Option<String>,
    url: Option<String>,
    sha1: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VersionCatalog {
    entries: Vec<CatalogEntry>,
    latest_release: Option<String>,
    latest_snapshot: Option<String>,
}

#[derive(Debug)]
pub enum CatalogError {
    Decode(String),
    Fetch(FetchError),
}

impl Display for CatalogError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(f, "invalid version catalog: {message}"),
            Self::Fetch(error) => write!(f, "version catalog unavailable: {error}"),
        }
    }
}

impl Error for CatalogError {}

impl From<FetchError> for CatalogError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

impl VersionCatalog {
    /// Decodes a catalog document.
    ///
    /// Entries without an id or manifest address are dropped rather than
    /// failing the whole catalog; a document with no `versions` list is
    /// rejected. Duplicate ids keep the first occurrence. Entries end up
    /// newest first by release time.
    pub fn decode_json(json: &str) -> Result<Self, CatalogError> {
        let raw: RawCatalog =
            serde_json::from_str(json).map_err(|error| CatalogError::Decode(error.to_string()))?;
        let versions = raw
            .versions
            .ok_or_else(|| CatalogError::Decode("missing versions list".to_owned()))?;
        let mut entries: Vec<CatalogEntry> = Vec::with_capacity(versions.len());
        for entry in versions {
            let (Some(id), Some(url)) = (
                entry.id.filter(|id| !id.trim().is_empty()),
                entry.url.filter(|url| !url.trim().is_empty()),
            ) else {
                continue;
            };
            if entries.iter().any(|known| known.id == id) {
                continue;
            }
            entries.push(CatalogEntry {
                id,
                kind: entry
                    .kind
                    .as_deref()
                    .map_or(VersionKind::Other, VersionKind::from_protocol),
                release_time: entry.release_time.or(entry.time).unwrap_or_default(),
                url,
                sha1: entry.sha1,
            });
        }
        // Stable, so entries published with equal times keep catalog order.
        entries.sort_by(|a, b| b.release_time.cmp(&a.release_time));
        Ok(Self {
            entries,
            latest_release: raw.latest.release,
            latest_snapshot: raw.latest.snapshot,
        })
    }

    /// Fetches the catalog from the first source that answers.
    pub async fn fetch<T: Transport + ?Sized>(
        transport: &T,
        chain: &SourceChain,
    ) -> Result<Self, CatalogError> {
        let sources = chain.candidates(OFFICIAL_CATALOG_URL);
        Ok(fetch_decoded(transport, &sources, |bytes| {
            let text = String::from_utf8(bytes)
                .map_err(|error| CatalogError::Decode(error.utf8_error().to_string()))?;
            Self::decode_json(&text)
        })
        .await?)
    }

    #[must_use]
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    /// Entries of the given kinds, newest first.
    pub fn of_kinds<'a>(
        &'a self,
        kinds: &'a [VersionKind],
    ) -> impl Iterator<Item = &'a CatalogEntry> + 'a {
        self.entries
            .iter()
            .filter(move |entry| kinds.contains(&entry.kind))
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    #[must_use]
    pub fn latest_release(&self) -> Option<&CatalogEntry> {
        let id = self.latest_release.as_deref()?;
        self.get(id)
    }

    #[must_use]
    pub fn latest_snapshot(&self) -> Option<&CatalogEntry> {
        let id = self.latest_snapshot.as_deref()?;
        self.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"{
        "latest": {"release": "1.21.1", "snapshot": "24w40a"},
        "versions": [
            {"id": "1.20.4", "type": "release", "url": "https://x/1.20.4.json",
             "releaseTime": "2023-12-07T12:56:20+00:00", "sha1": "aa"},
            {"id": "24w40a", "type": "snapshot", "url": "https://x/24w40a.json",
             "releaseTime": "2024-10-02T13:00:00+00:00"},
            {"id": "1.21.1", "type": "release", "url": "https://x/1.21.1.json",
             "releaseTime": "2024-08-08T12:24:45+00:00"},
            {"id": "b1.7.3", "type": "old_beta", "url": "https://x/b1.7.3.json",
             "releaseTime": "2011-07-08T00:00:00+00:00"},
            {"id": "", "type": "release", "url": "https://x/blank.json"},
            {"id": "no-url", "type": "release"},
            {"id": "1.21.1", "type": "release", "url": "https://x/dup.json",
             "releaseTime": "2000-01-01T00:00:00+00:00"},
            {"id": "weird", "type": "future_kind", "url": "https://x/w.json",
             "time": "2025-01-01T00:00:00+00:00"}
        ]
    }"#;

    #[test]
    fn orders_newest_first_and_drops_unusable_entries() {
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        let ids: Vec<_> = catalog.entries().iter().map(CatalogEntry::id).collect();
        assert_eq!(ids, ["weird", "24w40a", "1.21.1", "1.20.4", "b1.7.3"]);
    }

    #[test]
    fn duplicate_ids_keep_the_first_occurrence() {
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        assert_eq!(
            catalog.get("1.21.1").unwrap().manifest_url(),
            "https://x/1.21.1.json"
        );
    }

    #[test]
    fn classifies_kinds_and_falls_back_to_time() {
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        assert_eq!(catalog.get("b1.7.3").unwrap().kind(), VersionKind::Old);
        let weird = catalog.get("weird").unwrap();
        assert_eq!(weird.kind(), VersionKind::Other);
        assert_eq!(weird.release_time(), "2025-01-01T00:00:00+00:00");
    }

    #[test]
    fn channels_tell_snapshots_pre_releases_and_candidates_apart() {
        use VersionChannel::*;
        for (id, channel) in [
            ("26.4-snapshot-2", Snapshot),
            ("25w14a", Snapshot),
            ("26.3-pre-3", PreRelease),
            ("1.14 Pre-Release 1", PreRelease),
            ("26.3-rc-1", Candidate),
            ("1.16-rc1", Candidate),
            ("1.17 Release Candidate 2", Candidate),
        ] {
            assert_eq!(VersionChannel::of_unreleased(id), channel, "{id}");
        }
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        assert_eq!(catalog.get("1.21.1").unwrap().channel(), Release);
        assert_eq!(catalog.get("24w40a").unwrap().channel(), Snapshot);
        assert_eq!(catalog.get("b1.7.3").unwrap().channel(), Old);
    }

    #[test]
    fn filters_by_kind() {
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        let releases: Vec<_> = catalog
            .of_kinds(&[VersionKind::Release])
            .map(CatalogEntry::id)
            .collect();
        assert_eq!(releases, ["1.21.1", "1.20.4"]);
    }

    #[test]
    fn resolves_latest_pointers() {
        let catalog = VersionCatalog::decode_json(DOC).unwrap();
        assert_eq!(catalog.latest_release().unwrap().id(), "1.21.1");
        assert_eq!(catalog.latest_snapshot().unwrap().id(), "24w40a");
    }

    #[test]
    fn rejects_a_document_without_versions() {
        assert!(matches!(
            VersionCatalog::decode_json(r#"{"latest":{}}"#),
            Err(CatalogError::Decode(_))
        ));
        assert!(matches!(
            VersionCatalog::decode_json("not json"),
            Err(CatalogError::Decode(_))
        ));
    }

    #[tokio::test]
    async fn fetches_through_a_local_source() {
        use crate::transfer::{FileTransport, OfficialSource};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.json");
        std::fs::write(&path, DOC).unwrap();
        // A mirror that rewrites the official address onto the local file.
        struct Local(String);
        impl crate::transfer::SourceProvider for Local {
            fn candidates(&self, _: &str) -> Vec<String> {
                vec![self.0.clone()]
            }
        }
        let url = url::Url::from_file_path(&path).unwrap().to_string();
        let chain = SourceChain::new([
            std::sync::Arc::new(Local(url)) as std::sync::Arc<dyn crate::transfer::SourceProvider>,
            std::sync::Arc::new(OfficialSource),
        ])
        .unwrap();
        let catalog = VersionCatalog::fetch(&FileTransport, &chain).await.unwrap();
        assert_eq!(catalog.entries().len(), 5);
    }
}
