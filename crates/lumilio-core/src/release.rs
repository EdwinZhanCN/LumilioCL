use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Deserializer, Serialize, de, ser::SerializeMap};
use serde_json::Value;

use crate::artifact::PackageCoordinate;
use crate::environment::{CompatibilityRule, HostProfile};

const DEFAULT_LIBRARY_REPOSITORY: &str = "https://libraries.minecraft.net/";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDescriptor {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
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

    fn with_fallbacks(&self, path: String, url: String) -> Self {
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
    id: String,
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha1: Option<String>,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    total_size: u64,
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
    component: String,
    major_version: u32,
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
    id: String,
    #[serde(flatten)]
    download: DownloadDescriptor,
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
    file: IdentifiedDownload,
    argument: String,
    #[serde(rename = "type")]
    format: String,
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
    exclude: Vec<String>,
}

impl ExtractionPolicy {
    #[must_use]
    pub fn exclusions(&self) -> &[String] {
        &self.exclude
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryDownloads {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact: Option<DownloadDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    classifiers: BTreeMap<String, DownloadDescriptor>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryDependency {
    #[serde(rename = "name")]
    coordinate: PackageCoordinate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    downloads: Option<LibraryDownloads>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    checksums: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extract: Option<ExtractionPolicy>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    natives: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rules: Vec<CompatibilityRule>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
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

    pub(crate) fn resolve_file(&self, host: &HostProfile) -> ResolvedLibraryFile {
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

    fn native_variant(&self, host: &HostProfile) -> Option<String> {
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

fn native_selector_candidates(host: &HostProfile) -> Vec<String> {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedLibraryFile {
    pub(crate) relative_path: String,
    pub(crate) download: DownloadDescriptor,
    /// Made by the loader's installer; never downloaded.
    pub(crate) generated: bool,
    pub(crate) native: bool,
    pub(crate) extraction: ExtractionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArgumentTemplate {
    Literal(String),
    Conditional {
        rules: Vec<CompatibilityRule>,
        values: Vec<String>,
    },
}

impl Serialize for ArgumentTemplate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Literal(value) => serializer.serialize_str(value),
            Self::Conditional { rules, values } => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("rules", rules)?;
                map.serialize_entry("value", values)?;
                map.end()
            }
        }
    }
}

impl ArgumentTemplate {
    pub(crate) fn render(
        &self,
        host: &HostProfile,
        variables: &BTreeMap<String, String>,
    ) -> Vec<String> {
        match self {
            Self::Literal(value) => vec![expand_variables(value, variables)],
            Self::Conditional { rules, values } if host.allows(rules) => values
                .iter()
                .map(|value| expand_variables(value, variables))
                .collect(),
            Self::Conditional { .. } => Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for ArgumentTemplate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(literal) => Ok(Self::Literal(literal)),
            Value::Object(mut object) => {
                let rules = object
                    .remove("rules")
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(de::Error::custom)?
                    .unwrap_or_default();
                let raw_values = object
                    .remove("value")
                    .or_else(|| object.remove("values"))
                    .ok_or_else(|| de::Error::missing_field("value"))?;
                let values = match raw_values {
                    Value::String(value) => vec![value],
                    Value::Array(values) => values
                        .into_iter()
                        .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                        .collect(),
                    other => {
                        return Err(de::Error::custom(format!(
                            "argument value must be a string or string array, got {other}"
                        )));
                    }
                };
                Ok(Self::Conditional { rules, values })
            }
            other => Err(de::Error::custom(format!(
                "argument must be a string or object, got {other}"
            ))),
        }
    }
}

pub(crate) fn expand_variables(template: &str, variables: &BTreeMap<String, String>) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;
    while let Some(start) = remaining.find("${") {
        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + 2..];
        let Some(end) = after_start.find('}') else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let name = &after_start[..end];
        if let Some(value) = variables.get(name) {
            output.push_str(value);
        } else {
            output.push_str("${");
            output.push_str(name);
            output.push('}');
        }
        remaining = &after_start[end + 1..];
    }
    output.push_str(remaining);
    output
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArgumentGroups {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    game: Option<Vec<ArgumentTemplate>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    jvm: Option<Vec<ArgumentTemplate>>,
}

impl ArgumentGroups {
    pub(crate) fn game(&self) -> Option<&[ArgumentTemplate]> {
        self.game.as_deref()
    }

    pub(crate) fn jvm(&self) -> Option<&[ArgumentTemplate]> {
        self.jvm.as_deref()
    }

    fn merge(parent: Option<&Self>, child: Option<&Self>) -> Option<Self> {
        if parent.is_none() && child.is_none() {
            return None;
        }
        Some(Self {
            game: merge_argument_list(
                parent.and_then(|value| value.game.as_ref()),
                child.and_then(|value| value.game.as_ref()),
            ),
            jvm: merge_argument_list(
                parent.and_then(|value| value.jvm.as_ref()),
                child.and_then(|value| value.jvm.as_ref()),
            ),
        })
    }
}

fn merge_argument_list(
    parent: Option<&Vec<ArgumentTemplate>>,
    child: Option<&Vec<ArgumentTemplate>>,
) -> Option<Vec<ArgumentTemplate>> {
    if parent.is_none() && child.is_none() {
        return None;
    }
    let mut merged = Vec::new();
    if let Some(parent) = parent {
        merged.extend(parent.iter().cloned());
    }
    if let Some(child) = child {
        merged.extend(child.iter().cloned());
    }
    Some(merged)
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseWire {
    id: Option<String>,
    #[serde(default)]
    minecraft_arguments: Option<String>,
    #[serde(default)]
    arguments: Option<ArgumentGroups>,
    #[serde(default)]
    main_class: Option<String>,
    #[serde(default)]
    inherits_from: Option<String>,
    #[serde(default)]
    jar: Option<String>,
    #[serde(default)]
    asset_index: Option<AssetCatalog>,
    #[serde(default)]
    assets: Option<String>,
    #[serde(default)]
    compliance_level: Option<u32>,
    #[serde(default)]
    java_version: Option<JavaRequirement>,
    #[serde(default)]
    libraries: Vec<LibraryDependency>,
    #[serde(default)]
    compatibility_rules: Vec<CompatibilityRule>,
    #[serde(default)]
    downloads: Option<BTreeMap<String, DownloadDescriptor>>,
    #[serde(default)]
    logging: Option<BTreeMap<String, LoggingConfiguration>>,
    #[serde(rename = "type", default)]
    release_type: Option<String>,
    #[serde(default)]
    time: Option<String>,
    #[serde(default)]
    release_time: Option<String>,
    #[serde(default)]
    minimum_launcher_version: Option<u32>,
    #[serde(default)]
    root: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    patches: Vec<Value>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct ReleaseManifest {
    id: String,
    legacy_arguments: Option<String>,
    arguments: Option<ArgumentGroups>,
    main_class: Option<String>,
    parent_id: Option<String>,
    jar_id: Option<String>,
    asset_index: Option<AssetCatalog>,
    assets: Option<String>,
    compliance_level: Option<u32>,
    java_requirement: Option<JavaRequirement>,
    libraries: Vec<LibraryDependency>,
    compatibility_rules: Vec<CompatibilityRule>,
    downloads: Option<BTreeMap<String, DownloadDescriptor>>,
    logging: Option<BTreeMap<String, LoggingConfiguration>>,
    release_type: Option<String>,
    time: Option<String>,
    release_time: Option<String>,
    minimum_launcher_revision: Option<u32>,
    root: Option<bool>,
    hidden: Option<bool>,
    patches: Vec<Value>,
    extra: BTreeMap<String, Value>,
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

    fn inherit_from(&self, parent: &Self) -> Self {
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

fn insert_optional<T: Serialize>(
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
    releases: BTreeMap<String, ReleaseManifest>,
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

    fn resolve_inner(
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

#[cfg(test)]
mod tests {

    #[test]
    fn an_explicitly_empty_address_marks_a_generated_file() {
        let host = crate::environment::HostProfile::current();
        let generated: LibraryDependency = serde_json::from_str(
            r#"{"name":"net.minecraftforge:forge:26.3-66.0.9:client",
                "downloads":{"artifact":{"path":"net/minecraftforge/forge/26.3-66.0.9/forge-26.3-66.0.9-client.jar",
                "url":"","sha1":"6254fd332277905b955a8502228e23a7eae396a8","size":1}}}"#,
        )
        .unwrap();
        let file = generated.resolve_file(&host);
        assert!(file.generated, "made by the installer, never downloaded");
        assert_eq!(file.download.url(), None);
        // Without any address the library repository is still the fallback.
        let plain: LibraryDependency = serde_json::from_str(r#"{"name":"a:b:1"}"#).unwrap();
        let file = plain.resolve_file(&host);
        assert!(!file.generated);
        assert_eq!(
            file.download.url(),
            Some("https://libraries.minecraft.net/a/b/1/b-1.jar")
        );
    }
    use super::{ArgumentTemplate, LibraryDependency, ReleaseManifest, ReleaseSet};

    #[test]
    fn unknown_members_do_not_prevent_decoding() {
        let release =
            ReleaseManifest::decode_json(r#"{"id":"release","custom":{"preserved":true}}"#)
                .unwrap();

        assert_eq!(release.id(), "release");
    }

    #[test]
    fn inheritance_cycles_are_rejected() {
        let first =
            ReleaseManifest::decode_json(r#"{"id":"first","inheritsFrom":"second"}"#).unwrap();
        let second =
            ReleaseManifest::decode_json(r#"{"id":"second","inheritsFrom":"first"}"#).unwrap();
        let mut releases = ReleaseSet::new();
        releases.insert(first);
        releases.insert(second);

        assert!(releases.resolve("first").is_err());
    }

    #[test]
    fn argument_string_decodes_as_a_literal() {
        let argument: ArgumentTemplate = serde_json::from_str(r#""--demo""#).unwrap();

        assert_eq!(argument, ArgumentTemplate::Literal("--demo".to_owned()));
    }
}
