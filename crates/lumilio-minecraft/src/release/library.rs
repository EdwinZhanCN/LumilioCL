use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{DEFAULT_LIBRARY_REPOSITORY, DownloadDescriptor, ExtractionPolicy};
use crate::artifact::PackageCoordinate;
use crate::environment::{CompatibilityRule, HostProfile};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) artifact: Option<DownloadDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) classifiers: BTreeMap<String, DownloadDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryDependency {
    #[serde(rename = "name")]
    pub(super) coordinate: PackageCoordinate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) downloads: Option<LibraryDownloads>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) checksums: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) extract: Option<ExtractionPolicy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) natives: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) rules: Vec<CompatibilityRule>,
    #[serde(flatten)]
    pub(super) extra: BTreeMap<String, Value>,
}

pub(super) fn native_selector_candidates(host: &HostProfile) -> Vec<String> {
    let os = host.platform().protocol_name();
    let architecture = host.architecture().protocol_name();
    let bits = host.architecture().bit_width().to_string();
    let mut candidates = Vec::with_capacity(9);
    for key in ["", architecture, bits.as_str()] {
        for prefix in ["", "native-", "natives-"] {
            let suffix = if key.is_empty() {
                String::new()
            } else {
                format!("-{key}")
            };
            candidates.push(format!("{prefix}{os}{suffix}"));
        }
    }
    candidates
}

/// The file a library resolves to on one host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLibraryFile {
    pub(crate) relative_path: String,
    pub(crate) download: DownloadDescriptor,
    /// Made by the loader's installer; never downloaded.
    pub(crate) generated: bool,
    pub(crate) native: bool,
    pub(crate) extraction: ExtractionPolicy,
}

impl ResolvedLibraryFile {
    /// The path under the libraries directory.
    #[must_use]
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    #[must_use]
    pub const fn download(&self) -> &DownloadDescriptor {
        &self.download
    }
}

impl LibraryDependency {
    #[must_use]
    pub const fn coordinate(&self) -> &PackageCoordinate {
        &self.coordinate
    }

    #[must_use]
    pub fn rules(&self) -> &[CompatibilityRule] {
        &self.rules
    }

    #[must_use]
    pub fn extraction_policy(&self) -> Option<&ExtractionPolicy> {
        self.extract.as_ref()
    }

    /// The native variants it names per system (`natives-osx`, …).
    pub fn native_classifiers(&self) -> impl Iterator<Item = &str> {
        self.natives.values().map(String::as_str)
    }

    #[must_use]
    pub fn resolve_file(&self, host: &HostProfile) -> ResolvedLibraryFile {
        let native_variant = self.native_variant(host);
        let raw = self.downloads.as_ref().and_then(|downloads| {
            native_variant
                .as_ref()
                .and_then(|variant| downloads.classifiers.get(variant))
                .or(downloads
                    .artifact
                    .as_ref()
                    .filter(|_| native_variant.is_none()))
        });
        let coordinate = self.coordinate.with_variant(
            native_variant
                .clone()
                .or_else(|| self.coordinate.variant().map(ToOwned::to_owned)),
        );
        let path = raw
            .and_then(DownloadDescriptor::path)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| coordinate.repository_path());
        let repository = self
            .url
            .as_deref()
            .filter(|url| !url.is_empty())
            .unwrap_or(DEFAULT_LIBRARY_REPOSITORY);
        let separator = if repository.ends_with('/') { "" } else { "/" };
        let fallback_url = format!("{repository}{separator}{path}");
        let descriptor = raw.cloned().unwrap_or(DownloadDescriptor {
            url: None,
            sha1: self.checksums.first().cloned(),
            size: 0,
            path: None,
        });

        let generated = descriptor.is_generated();
        ResolvedLibraryFile {
            relative_path: path.clone(),
            download: if generated {
                descriptor
            } else {
                descriptor.with_fallbacks(path, fallback_url)
            },
            generated,
            native: native_variant.is_some(),
            extraction: self.extract.clone().unwrap_or_default(),
        }
    }

    pub(super) fn native_variant(&self, host: &HostProfile) -> Option<String> {
        let candidates = native_selector_candidates(host);
        for candidate in &candidates {
            if let Some(value) = self.natives.get(candidate) {
                return Some(
                    value.replace("${arch}", &host.architecture().bit_width().to_string()),
                );
            }
        }

        let classifiers = self
            .downloads
            .as_ref()
            .map(|downloads| &downloads.classifiers)?;
        candidates
            .into_iter()
            .find(|candidate| classifiers.contains_key(candidate))
    }
}
