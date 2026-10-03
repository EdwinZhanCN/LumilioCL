use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path, PathBuf};

use crate::environment::{CompatibilityRule, HostProfile, RuleDecision};
use crate::release::{
    ArgumentTemplate, AssetCatalog, DownloadDescriptor, ExtractionPolicy, JavaRequirement,
    LoggingConfiguration, ReleaseManifest, expand_variables,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchDirectories {
    libraries: PathBuf,
    versions: PathBuf,
    assets: PathBuf,
    natives: PathBuf,
    game: PathBuf,
}

impl LaunchDirectories {
    #[must_use]
    pub fn under(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref();
        Self {
            libraries: root.join("libraries"),
            versions: root.join("versions"),
            assets: root.join("assets"),
            natives: root.join("natives"),
            game: root.join("game"),
        }
    }

    #[must_use]
    pub fn new(
        libraries: impl Into<PathBuf>,
        versions: impl Into<PathBuf>,
        assets: impl Into<PathBuf>,
        natives: impl Into<PathBuf>,
        game: impl Into<PathBuf>,
    ) -> Self {
        Self {
            libraries: libraries.into(),
            versions: versions.into(),
            assets: assets.into(),
            natives: natives.into(),
            game: game.into(),
        }
    }

    #[must_use]
    pub fn libraries(&self) -> &Path {
        &self.libraries
    }

    #[must_use]
    pub fn versions(&self) -> &Path {
        &self.versions
    }

    #[must_use]
    pub fn assets(&self) -> &Path {
        &self.assets
    }

    #[must_use]
    pub fn natives(&self) -> &Path {
        &self.natives
    }

    #[must_use]
    pub fn game(&self) -> &Path {
        &self.game
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchContext {
    host: HostProfile,
    directories: LaunchDirectories,
    values: BTreeMap<String, String>,
}

impl LaunchContext {
    #[must_use]
    pub fn new(host: HostProfile, directories: LaunchDirectories) -> Self {
        let mut values = BTreeMap::new();
        values.insert(
            "library_directory".to_owned(),
            directories.libraries.to_string_lossy().into_owned(),
        );
        values.insert(
            "natives_directory".to_owned(),
            directories.natives.to_string_lossy().into_owned(),
        );
        values.insert(
            "assets_root".to_owned(),
            directories.assets.to_string_lossy().into_owned(),
        );
        values.insert(
            "game_directory".to_owned(),
            directories.game.to_string_lossy().into_owned(),
        );
        values.insert("launcher_name".to_owned(), "LumilioCL".to_owned());
        values.insert(
            "launcher_version".to_owned(),
            env!("CARGO_PKG_VERSION").to_owned(),
        );
        Self {
            host,
            directories,
            values,
        }
    }

    #[must_use]
    pub fn with_value(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(name.into(), value.into());
        self
    }

    #[must_use]
    pub fn with_feature(mut self, name: impl Into<String>, enabled: bool) -> Self {
        self.host = self.host.clone().with_feature(name, enabled);
        self
    }

    #[must_use]
    pub fn host(&self) -> &HostProfile {
        &self.host
    }

    #[must_use]
    pub fn directories(&self) -> &LaunchDirectories {
        &self.directories
    }

    #[must_use]
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeArchive {
    path: PathBuf,
    download: DownloadDescriptor,
    extraction: ExtractionPolicy,
}

impl NativeArchive {
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    #[must_use]
    pub const fn download(&self) -> &DownloadDescriptor {
        &self.download
    }

    #[must_use]
    pub const fn extraction_policy(&self) -> &ExtractionPolicy {
        &self.extraction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequiredDownload {
    destination: PathBuf,
    source: DownloadDescriptor,
}

impl RequiredDownload {
    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    #[must_use]
    pub const fn source(&self) -> &DownloadDescriptor {
        &self.source
    }
}

#[derive(Clone, Debug)]
pub struct LaunchPlan {
    release_id: String,
    main_class: String,
    java_requirement: Option<JavaRequirement>,
    jvm_arguments: Vec<String>,
    game_arguments: Vec<String>,
    classpath: Vec<PathBuf>,
    native_archives: Vec<NativeArchive>,
    required_downloads: Vec<RequiredDownload>,
    asset_catalog: Option<AssetCatalog>,
    logging: Option<BTreeMap<String, LoggingConfiguration>>,
}

impl LaunchPlan {
    #[must_use]
    pub fn release_id(&self) -> &str {
        &self.release_id
    }

    #[must_use]
    pub fn main_class(&self) -> &str {
        &self.main_class
    }

    #[must_use]
    pub fn java_requirement(&self) -> Option<&JavaRequirement> {
        self.java_requirement.as_ref()
    }

    #[must_use]
    pub fn jvm_arguments(&self) -> &[String] {
        &self.jvm_arguments
    }

    #[must_use]
    pub fn game_arguments(&self) -> &[String] {
        &self.game_arguments
    }

    #[must_use]
    pub fn classpath(&self) -> &[PathBuf] {
        &self.classpath
    }

    #[must_use]
    pub fn native_archives(&self) -> &[NativeArchive] {
        &self.native_archives
    }

    #[must_use]
    pub fn required_downloads(&self) -> &[RequiredDownload] {
        &self.required_downloads
    }

    #[must_use]
    pub fn asset_catalog(&self) -> Option<&AssetCatalog> {
        self.asset_catalog.as_ref()
    }

    #[must_use]
    pub fn logging(&self) -> Option<&BTreeMap<String, LoggingConfiguration>> {
        self.logging.as_ref()
    }
}

impl ReleaseManifest {
    pub fn build_launch_plan(&self, context: &LaunchContext) -> Result<LaunchPlan, LaunchError> {
        if self.parent_id().is_some() {
            return Err(LaunchError::UnresolvedInheritance(self.id().to_owned()));
        }
        if !self.patches().is_empty() {
            return Err(LaunchError::PendingPatches(self.id().to_owned()));
        }
        if !context.host.allows(self.compatibility_rules()) {
            return Err(LaunchError::ReleaseBlocked(self.id().to_owned()));
        }
        let main_class = self
            .main_class()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| LaunchError::MissingMainClass(self.id().to_owned()))?
            .to_owned();

        let mut classpath = Vec::new();
        let mut seen_classpath = HashSet::new();
        let mut seen_downloads = HashSet::new();
        let mut native_archives = Vec::new();
        let mut required_downloads = Vec::new();

        for library in self.libraries() {
            if !context.host.allows(library.rules()) {
                continue;
            }
            let resolved = library.resolve_file(&context.host);
            let destination =
                join_relative(context.directories.libraries(), &resolved.relative_path)?;
            // Files the loader's installer makes are on the classpath but
            // never downloaded; the installer step produces them.
            let newly_required = !resolved.generated && seen_downloads.insert(destination.clone());
            if newly_required {
                required_downloads.push(RequiredDownload {
                    destination: destination.clone(),
                    source: resolved.download.clone(),
                });
            }
            if resolved.native {
                if newly_required {
                    native_archives.push(NativeArchive {
                        path: destination,
                        download: resolved.download,
                        extraction: resolved.extraction,
                    });
                }
            } else if seen_classpath.insert(destination.clone()) {
                classpath.push(destination);
            }
        }

        let jar_id = self.jar_id();
        let client_path = context
            .directories
            .versions()
            .join(jar_id)
            .join(format!("{jar_id}.jar"));
        if seen_classpath.insert(client_path.clone()) {
            classpath.push(client_path.clone());
        }
        if let Some(client) = self
            .downloads()
            .and_then(|downloads| downloads.get("client"))
            && seen_downloads.insert(client_path.clone())
        {
            required_downloads.push(RequiredDownload {
                destination: client_path,
                source: client.clone(),
            });
        }

        let separator = if context.host.platform().protocol_name() == "windows" {
            ";"
        } else {
            ":"
        };
        let classpath_value = classpath
            .iter()
            .map(|path| path.to_string_lossy())
            .collect::<Vec<_>>()
            .join(separator);
        let mut variables = context.values.clone();
        variables.insert("classpath".to_owned(), classpath_value);
        variables.insert("classpath_separator".to_owned(), separator.to_owned());
        variables.insert("version_name".to_owned(), self.id().to_owned());
        variables.insert("assets_index_name".to_owned(), asset_id(self).to_owned());
        variables.insert("primary_jar_name".to_owned(), format!("{jar_id}.jar"));

        let jvm_arguments = match self.arguments().and_then(|groups| groups.jvm()) {
            Some(arguments) => render_templates(arguments, &context.host, &variables)?,
            None => render_templates(&default_jvm_arguments(), &context.host, &variables)?,
        };
        let game_arguments = if let Some(legacy) = self.legacy_arguments() {
            let tokens = tokenize_legacy_arguments(legacy)?;
            render_legacy_tokens(&tokens, &variables)?
        } else {
            match self.arguments().and_then(|groups| groups.game()) {
                Some(arguments) => render_templates(arguments, &context.host, &variables)?,
                None => render_templates(&default_game_arguments(), &context.host, &variables)?,
            }
        };

        Ok(LaunchPlan {
            release_id: self.id().to_owned(),
            main_class,
            java_requirement: self.java_requirement().cloned(),
            jvm_arguments,
            game_arguments,
            classpath,
            native_archives,
            required_downloads,
            asset_catalog: self.asset_catalog().cloned(),
            logging: self.logging().cloned(),
        })
    }
}

fn asset_id(release: &ReleaseManifest) -> &str {
    release
        .asset_catalog()
        .map(AssetCatalog::id)
        .unwrap_or("legacy")
}

fn default_jvm_arguments() -> Vec<ArgumentTemplate> {
    vec![
        ArgumentTemplate::Literal("-Djava.library.path=${natives_directory}".to_owned()),
        ArgumentTemplate::Literal("-Dminecraft.launcher.brand=${launcher_name}".to_owned()),
        ArgumentTemplate::Literal("-Dminecraft.launcher.version=${launcher_version}".to_owned()),
        ArgumentTemplate::Literal("-cp".to_owned()),
        ArgumentTemplate::Literal("${classpath}".to_owned()),
    ]
}

fn default_game_arguments() -> Vec<ArgumentTemplate> {
    vec![ArgumentTemplate::Conditional {
        rules: vec![
            CompatibilityRule::new(RuleDecision::Permit)
                .requiring_feature("has_custom_resolution", true),
        ],
        values: vec![
            "--width".to_owned(),
            "${resolution_width}".to_owned(),
            "--height".to_owned(),
            "${resolution_height}".to_owned(),
        ],
    }]
}

fn render_templates(
    templates: &[ArgumentTemplate],
    host: &HostProfile,
    variables: &BTreeMap<String, String>,
) -> Result<Vec<String>, LaunchError> {
    let mut rendered = Vec::new();
    for template in templates {
        for value in template.render(host, variables) {
            reject_unresolved_variable(&value)?;
            rendered.push(value);
        }
    }
    Ok(rendered)
}

fn render_legacy_tokens(
    tokens: &[String],
    variables: &BTreeMap<String, String>,
) -> Result<Vec<String>, LaunchError> {
    tokens
        .iter()
        .map(|token| {
            let expanded = expand_variables(token, variables);
            reject_unresolved_variable(&expanded)?;
            Ok(expanded)
        })
        .collect()
}

fn reject_unresolved_variable(value: &str) -> Result<(), LaunchError> {
    if let Some(start) = value.find("${")
        && let Some(end) = value[start + 2..].find('}')
    {
        return Err(LaunchError::MissingVariable(
            value[start + 2..start + 2 + end].to_owned(),
        ));
    }
    Ok(())
}

fn join_relative(root: &Path, relative: &str) -> Result<PathBuf, LaunchError> {
    let path = Path::new(relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(LaunchError::UnsafeLibraryPath(relative.to_owned()));
    }
    Ok(root.join(path))
}

fn tokenize_legacy_arguments(value: &str) -> Result<Vec<String>, LaunchError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;

    for character in value.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            character if character.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }
    if escaped {
        current.push('\\');
    }
    if quote.is_some() {
        return Err(LaunchError::UnterminatedQuote);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchError {
    UnresolvedInheritance(String),
    PendingPatches(String),
    ReleaseBlocked(String),
    MissingMainClass(String),
    MissingVariable(String),
    UnsafeLibraryPath(String),
    UnterminatedQuote,
}

impl Display for LaunchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedInheritance(id) => {
                write!(formatter, "release {id} still has an unresolved parent")
            }
            Self::PendingPatches(id) => {
                write!(formatter, "release {id} still has pending patch operations")
            }
            Self::ReleaseBlocked(id) => {
                write!(formatter, "release {id} is blocked on this host")
            }
            Self::MissingMainClass(id) => write!(formatter, "release {id} has no main class"),
            Self::MissingVariable(name) => write!(formatter, "launch variable {name} is missing"),
            Self::UnsafeLibraryPath(path) => {
                write!(formatter, "library path escapes the library root: {path}")
            }
            Self::UnterminatedQuote => {
                formatter.write_str("legacy argument string has an open quote")
            }
        }
    }
}

impl Error for LaunchError {}

#[cfg(test)]
mod tests {
    use super::tokenize_legacy_arguments;

    #[test]
    fn legacy_tokenizer_supports_both_quote_styles_and_escapes() {
        let tokens =
            tokenize_legacy_arguments("--first 'two words' --third escaped\\ value").unwrap();

        assert_eq!(tokens, ["--first", "two words", "--third", "escaped value"]);
    }
}
