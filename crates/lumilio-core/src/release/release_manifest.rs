use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::arguments::ArgumentGroups;
use super::library::LibraryDependency;
use crate::environment::CompatibilityRule;

pub(super) const DEFAULT_LIBRARY_REPOSITORY: &str = "https://libraries.minecraft.net/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDescriptor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) sha1: Option<String>,
    #[serde(default)]
    pub(super) size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) path: Option<String>,
}

impl DownloadDescriptor {
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref().filter(|url| !url.is_empty())
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&str> {
        self.sha1
            .as_deref()
            .filter(|digest| !digest.eq_ignore_ascii_case("invalid"))
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref().filter(|path| !path.is_empty())
    }

    /// A file the loader's installer makes on this machine rather than one to
    /// download: Forge lists its patched client with an explicitly empty
    /// address. Absent addresses still fall back to the library repository.
    #[must_use]
    pub fn is_generated(&self) -> bool {
        self.url.as_deref() == Some("")
    }

    pub(super) fn with_fallbacks(&self, path: String, url: String) -> Self {
        Self {
            url: Some(self.url().unwrap_or(&url).to_owned()),
            sha1: self.sha1.clone(),
            size: self.size,
            path: Some(self.path().unwrap_or(&path).to_owned()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetCatalog {
    pub(super) id: String,
    pub(super) url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) sha1: Option<String>,
    #[serde(default)]
    pub(super) size: u64,
    #[serde(default)]
    pub(super) total_size: u64,
}

impl AssetCatalog {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[must_use]
    pub fn sha1(&self) -> Option<&str> {
        self.sha1.as_deref()
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn total_size(&self) -> u64 {
        self.total_size
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaRequirement {
    pub(super) component: String,
    pub(super) major_version: u32,
}

impl JavaRequirement {
    #[must_use]
    pub fn component(&self) -> &str {
        &self.component
    }

    #[must_use]
    pub const fn major(&self) -> u32 {
        self.major_version
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifiedDownload {
    pub(super) id: String,
    #[serde(flatten)]
    pub(super) download: DownloadDescriptor,
}

impl IdentifiedDownload {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub const fn download(&self) -> &DownloadDescriptor {
        &self.download
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LoggingConfiguration {
    pub(super) file: IdentifiedDownload,
    pub(super) argument: String,
    #[serde(rename = "type")]
    pub(super) format: String,
}

impl LoggingConfiguration {
    #[must_use]
    pub const fn file(&self) -> &IdentifiedDownload {
        &self.file
    }

    #[must_use]
    pub fn argument(&self) -> &str {
        &self.argument
    }

    #[must_use]
    pub fn format(&self) -> &str {
        &self.format
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExtractionPolicy {
    #[serde(default)]
    pub(super) exclude: Vec<String>,
}

impl ExtractionPolicy {
    #[must_use]
    pub fn exclusions(&self) -> &[String] {
        &self.exclude
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReleaseWire {
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) minecraft_arguments: Option<String>,
    #[serde(default)]
    pub(super) arguments: Option<ArgumentGroups>,
    #[serde(default)]
    pub(super) main_class: Option<String>,
    #[serde(default)]
    pub(super) inherits_from: Option<String>,
    #[serde(default)]
    pub(super) jar: Option<String>,
    #[serde(default)]
    pub(super) asset_index: Option<AssetCatalog>,
    #[serde(default)]
    pub(super) assets: Option<String>,
    #[serde(default)]
    pub(super) compliance_level: Option<u32>,
    #[serde(default)]
    pub(super) java_version: Option<JavaRequirement>,
    #[serde(default)]
    pub(super) libraries: Vec<LibraryDependency>,
    #[serde(default)]
    pub(super) compatibility_rules: Vec<CompatibilityRule>,
    #[serde(default)]
    pub(super) downloads: Option<BTreeMap<String, DownloadDescriptor>>,
    #[serde(default)]
    pub(super) logging: Option<BTreeMap<String, LoggingConfiguration>>,
    #[serde(rename = "type", default)]
    pub(super) release_type: Option<String>,
    #[serde(default)]
    pub(super) time: Option<String>,
    #[serde(default)]
    pub(super) release_time: Option<String>,
    #[serde(default)]
    pub(super) minimum_launcher_version: Option<u32>,
    #[serde(default)]
    pub(super) root: Option<bool>,
    #[serde(default)]
    pub(super) hidden: Option<bool>,
    #[serde(default)]
    pub(super) patches: Vec<Value>,
    #[serde(flatten)]
    pub(super) extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct ReleaseManifest {
    pub(super) id: String,
    pub(super) legacy_arguments: Option<String>,
    pub(super) arguments: Option<ArgumentGroups>,
    pub(super) main_class: Option<String>,
    pub(super) parent_id: Option<String>,
    pub(super) jar_id: Option<String>,
    pub(super) asset_index: Option<AssetCatalog>,
    pub(super) assets: Option<String>,
    pub(super) compliance_level: Option<u32>,
    pub(super) java_requirement: Option<JavaRequirement>,
    pub(super) libraries: Vec<LibraryDependency>,
    pub(super) compatibility_rules: Vec<CompatibilityRule>,
    pub(super) downloads: Option<BTreeMap<String, DownloadDescriptor>>,
    pub(super) logging: Option<BTreeMap<String, LoggingConfiguration>>,
    pub(super) release_type: Option<String>,
    pub(super) time: Option<String>,
    pub(super) release_time: Option<String>,
    pub(super) minimum_launcher_revision: Option<u32>,
    pub(super) root: Option<bool>,
    pub(super) hidden: Option<bool>,
    pub(super) patches: Vec<Value>,
    pub(super) extra: BTreeMap<String, Value>,
}

impl ReleaseManifest {
    pub fn decode_json(json: &str) -> Result<Self, ReleaseError> {
        let wire: ReleaseWire = serde_json::from_str(json).map_err(ReleaseError::Json)?;
        let id = wire
            .id
            .filter(|id| !id.trim().is_empty())
            .ok_or(ReleaseError::MissingId)?;
        Ok(Self {
            id,
            legacy_arguments: wire.minecraft_arguments,
            arguments: wire.arguments,
            main_class: wire.main_class,
            parent_id: wire.inherits_from,
            jar_id: wire.jar,
            asset_index: wire.asset_index,
            assets: wire.assets,
            compliance_level: wire.compliance_level,
            java_requirement: wire.java_version,
            libraries: wire.libraries,
            compatibility_rules: wire.compatibility_rules,
            downloads: wire.downloads,
            logging: wire.logging,
            release_type: wire.release_type,
            time: wire.time,
            release_time: wire.release_time,
            minimum_launcher_revision: wire.minimum_launcher_version,
            root: wire.root,
            hidden: wire.hidden,
            patches: wire.patches,
            extra: wire.extra,
        })
    }

    pub fn encode_json(&self) -> Result<String, ReleaseError> {
        let mut object = serde_json::Map::from_iter(self.extra.clone());
        object.insert("id".to_owned(), Value::String(self.id.clone()));
        insert_optional(
            &mut object,
            "minecraftArguments",
            self.legacy_arguments.as_ref(),
        )?;
        insert_optional(&mut object, "arguments", self.arguments.as_ref())?;
        insert_optional(&mut object, "mainClass", self.main_class.as_ref())?;
        insert_optional(&mut object, "inheritsFrom", self.parent_id.as_ref())?;
        insert_optional(&mut object, "jar", self.jar_id.as_ref())?;
        insert_optional(&mut object, "assetIndex", self.asset_index.as_ref())?;
        insert_optional(&mut object, "assets", self.assets.as_ref())?;
        insert_optional(
            &mut object,
            "complianceLevel",
            self.compliance_level.as_ref(),
        )?;
        insert_optional(&mut object, "javaVersion", self.java_requirement.as_ref())?;
        if !self.libraries.is_empty() {
            object.insert(
                "libraries".to_owned(),
                serde_json::to_value(&self.libraries).map_err(ReleaseError::Json)?,
            );
        }
        if !self.compatibility_rules.is_empty() {
            object.insert(
                "compatibilityRules".to_owned(),
                serde_json::to_value(&self.compatibility_rules).map_err(ReleaseError::Json)?,
            );
        }
        insert_optional(&mut object, "downloads", self.downloads.as_ref())?;
        insert_optional(&mut object, "logging", self.logging.as_ref())?;
        insert_optional(&mut object, "type", self.release_type.as_ref())?;
        insert_optional(&mut object, "time", self.time.as_ref())?;
        insert_optional(&mut object, "releaseTime", self.release_time.as_ref())?;
        insert_optional(
            &mut object,
            "minimumLauncherVersion",
            self.minimum_launcher_revision.as_ref(),
        )?;
        insert_optional(&mut object, "root", self.root.as_ref())?;
        insert_optional(&mut object, "hidden", self.hidden.as_ref())?;
        if !self.patches.is_empty() {
            object.insert("patches".to_owned(), Value::Array(self.patches.clone()));
        }
        serde_json::to_string_pretty(&Value::Object(object)).map_err(ReleaseError::Json)
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&str> {
        self.parent_id.as_deref()
    }

    #[must_use]
    pub fn main_class(&self) -> Option<&str> {
        self.main_class.as_deref()
    }

    #[must_use]
    pub fn minimum_launcher_revision(&self) -> Option<u32> {
        self.minimum_launcher_revision
    }

    #[must_use]
    pub fn libraries(&self) -> &[LibraryDependency] {
        &self.libraries
    }

    #[must_use]
    pub fn java_requirement(&self) -> Option<&JavaRequirement> {
        self.java_requirement.as_ref()
    }

    #[must_use]
    pub fn asset_catalog(&self) -> Option<&AssetCatalog> {
        self.asset_index.as_ref()
    }

    #[must_use]
    pub fn assets_id(&self) -> Option<&str> {
        self.assets.as_deref()
    }

    #[must_use]
    pub const fn compliance_level(&self) -> Option<u32> {
        self.compliance_level
    }

    #[must_use]
    pub const fn is_root(&self) -> bool {
        matches!(self.root, Some(true))
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        matches!(self.hidden, Some(true))
    }

    #[must_use]
    pub fn logging(&self) -> Option<&BTreeMap<String, LoggingConfiguration>> {
        self.logging.as_ref()
    }

    pub(crate) fn legacy_arguments(&self) -> Option<&str> {
        self.legacy_arguments.as_deref()
    }

    pub(crate) fn arguments(&self) -> Option<&ArgumentGroups> {
        self.arguments.as_ref()
    }

    pub(crate) fn jar_id(&self) -> &str {
        self.jar_id.as_deref().unwrap_or(&self.id)
    }

    pub(crate) fn compatibility_rules(&self) -> &[CompatibilityRule] {
        &self.compatibility_rules
    }

    pub(crate) fn downloads(&self) -> Option<&BTreeMap<String, DownloadDescriptor>> {
        self.downloads.as_ref()
    }

    pub(crate) fn patches(&self) -> &[Value] {
        &self.patches
    }

    pub(super) fn inherit_from(&self, parent: &Self) -> Self {
        let mut libraries = self.libraries.clone();
        libraries.extend(parent.libraries.iter().cloned());
        let mut compatibility_rules = parent.compatibility_rules.clone();
        compatibility_rules.extend(self.compatibility_rules.iter().cloned());
        let mut patches = parent.patches.clone();
        patches.extend(self.patches.iter().cloned());
        let mut extra = parent.extra.clone();
        extra.extend(self.extra.clone());

        Self {
            id: self.id.clone(),
            legacy_arguments: self
                .legacy_arguments
                .clone()
                .or_else(|| parent.legacy_arguments.clone()),
            arguments: ArgumentGroups::merge(parent.arguments.as_ref(), self.arguments.as_ref()),
            main_class: self
                .main_class
                .clone()
                .or_else(|| parent.main_class.clone()),
            parent_id: None,
            jar_id: self.jar_id.clone().or_else(|| parent.jar_id.clone()),
            asset_index: self
                .asset_index
                .clone()
                .or_else(|| parent.asset_index.clone()),
            assets: self.assets.clone().or_else(|| parent.assets.clone()),
            compliance_level: self.compliance_level.or(parent.compliance_level),
            java_requirement: self
                .java_requirement
                .clone()
                .or_else(|| parent.java_requirement.clone()),
            libraries,
            compatibility_rules,
            downloads: self.downloads.clone().or_else(|| parent.downloads.clone()),
            logging: self.logging.clone().or_else(|| parent.logging.clone()),
            release_type: self
                .release_type
                .clone()
                .or_else(|| parent.release_type.clone()),
            time: self.time.clone().or_else(|| parent.time.clone()),
            release_time: self
                .release_time
                .clone()
                .or_else(|| parent.release_time.clone()),
            minimum_launcher_revision: match (
                self.minimum_launcher_revision,
                parent.minimum_launcher_revision,
            ) {
                (Some(child), Some(parent)) => Some(child.max(parent)),
                (child, parent) => child.or(parent),
            },
            root: Some(true),
            hidden: self.hidden.or(parent.hidden),
            patches,
            extra,
        }
    }
}

pub(super) fn insert_optional<T: Serialize>(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    value: Option<&T>,
) -> Result<(), ReleaseError> {
    if let Some(value) = value {
        object.insert(
            key.to_owned(),
            serde_json::to_value(value).map_err(ReleaseError::Json)?,
        );
    }
    Ok(())
}

#[derive(Clone, Debug, Default)]
pub struct ReleaseSet {
    pub(super) releases: BTreeMap<String, ReleaseManifest>,
}

impl ReleaseSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            releases: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, release: ReleaseManifest) -> Option<ReleaseManifest> {
        self.releases.insert(release.id.clone(), release)
    }

    pub fn resolve(&self, id: &str) -> Result<ReleaseManifest, ReleaseError> {
        self.resolve_inner(id, &mut BTreeSet::new())
    }

    pub(super) fn resolve_inner(
        &self,
        id: &str,
        resolving: &mut BTreeSet<String>,
    ) -> Result<ReleaseManifest, ReleaseError> {
        if !resolving.insert(id.to_owned()) {
            return Err(ReleaseError::InheritanceCycle(id.to_owned()));
        }
        let release = self
            .releases
            .get(id)
            .ok_or_else(|| ReleaseError::MissingRelease(id.to_owned()))?;
        let resolved = if let Some(parent_id) = release.parent_id() {
            let parent = self.resolve_inner(parent_id, resolving)?;
            release.inherit_from(&parent)
        } else {
            release.clone()
        };
        resolving.remove(id);
        Ok(resolved)
    }
}

#[derive(Debug)]
pub enum ReleaseError {
    Json(serde_json::Error),
    MissingId,
    MissingRelease(String),
    InheritanceCycle(String),
}

impl Display for ReleaseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "release JSON error: {error}"),
            Self::MissingId => formatter.write_str("release JSON is missing a non-empty id"),
            Self::MissingRelease(id) => write!(formatter, "release {id} was not found"),
            Self::InheritanceCycle(id) => {
                write!(formatter, "release inheritance contains a cycle at {id}")
            }
        }
    }
}

impl Error for ReleaseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::MissingId | Self::MissingRelease(_) | Self::InheritanceCycle(_) => None,
        }
    }
}
