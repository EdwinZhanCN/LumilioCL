use lumilio_core::{LaunchTuning, LauncherSettings, MirrorRule, Preferences, StorageUsage};
use std::path::{Path, PathBuf};

/// One Java installation of the Java tab.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JavaRow {
    pub home: PathBuf,
    /// `Java 21.0.1`.
    pub title: String,
    /// `Zulu · arm64 · /path/to/home`.
    pub detail: String,
    pub disabled: bool,
}

/// Everything the Settings page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsView {
    pub preferences: Preferences,
    pub launch: LaunchTuning,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub download_concurrency: Option<u32>,
    pub mirrors: Vec<MirrorRule>,
    pub prefer_mirrors: bool,
    pub java_roots: Vec<PathBuf>,
    pub java: Vec<JavaRow>,
    /// `None` while the disk is still being measured.
    pub storage: Option<StorageUsage>,
    pub data_dir: PathBuf,
    pub total_memory_mb: Option<u64>,
}

/// The Settings page's data, from the saved settings and what was found on
/// this machine.
pub fn settings_view(
    settings: &LauncherSettings,
    java: &[(lumilio_core::JavaRuntime, bool)],
    storage: Option<StorageUsage>,
    data_dir: &Path,
    total_memory_mb: Option<u64>,
) -> SettingsView {
    SettingsView {
        preferences: settings.preferences.clone(),
        launch: settings.launch.clone(),
        min_memory_mb: settings.default_min_memory_mb,
        max_memory_mb: settings.default_max_memory_mb,
        download_concurrency: settings.download_concurrency,
        mirrors: settings.mirrors.clone(),
        prefer_mirrors: settings.prefer_mirrors,
        java_roots: settings.extra_java_roots.clone(),
        java: java
            .iter()
            .map(|(runtime, disabled)| JavaRow {
                home: runtime.home().to_owned(),
                title: format!("Java {}", runtime.version()),
                detail: [
                    runtime.vendor().map(str::to_owned),
                    runtime.architecture().map(str::to_owned),
                    Some(runtime.home().display().to_string()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · "),
                disabled: *disabled,
            })
            .collect(),
        storage,
        data_dir: data_dir.to_owned(),
        total_memory_mb,
    }
}
