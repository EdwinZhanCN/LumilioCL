use lumilio_core::{
    DownloadSourcePreference, HostProfile, JavaRuntime, JavaSource, LaunchTuning, LauncherSettings,
    MirrorRule, Preferences, StorageUsage,
};
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
    /// The launcher installed it into its own folder, so it may delete it.
    pub removable: bool,
}

/// How a Java reads in a list or a dropdown: `Java 21 · Temurin · arm64`,
/// with a note when it runs translated on this machine.
#[must_use]
pub fn java_label(runtime: &JavaRuntime, host: &HostProfile) -> String {
    let mut parts = vec![format!("Java {}", runtime.major())];
    parts.extend(runtime.vendor().map(str::to_owned));
    parts.extend(runtime.architecture().map(str::to_owned));
    if !runtime.is_native(host) {
        parts.push(crate::tr!("java-translated").to_owned());
    }
    parts.join(" · ")
}

/// Everything the Settings page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsView {
    pub plugins: Vec<lumilio_core::PluginInfo>,
    pub preferences: Preferences,
    pub launch: LaunchTuning,
    pub min_memory_mb: Option<u32>,
    pub max_memory_mb: Option<u32>,
    pub download_concurrency: Option<u32>,
    pub mirrors: Vec<MirrorRule>,
    pub download_source: DownloadSourcePreference,
    pub auto_update_enabled: bool,
    pub java_roots: Vec<PathBuf>,
    pub java: Vec<JavaRow>,
    /// Where downloaded Java comes from.
    pub java_source: JavaSource,
    /// Whether a launch without a fitting Java downloads one first.
    pub auto_install_java: bool,
    /// The majors each download source offers this machine; empty until read.
    pub java_offers: Vec<(JavaSource, Vec<u32>)>,
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
        plugins: Vec::new(),
        preferences: settings.preferences.clone(),
        launch: settings.launch.clone(),
        min_memory_mb: settings.default_min_memory_mb,
        max_memory_mb: settings.default_max_memory_mb,
        download_concurrency: settings.download_concurrency,
        mirrors: settings.mirrors.clone(),
        download_source: settings.download_source_preference(),
        auto_update_enabled: settings.auto_update,
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
                removable: runtime.installed_under(&data_dir.join("runtimes")),
            })
            .collect(),
        java_source: settings.java_source,
        auto_install_java: settings.auto_install_java,
        java_offers: Vec::new(),
        storage,
        data_dir: data_dir.to_owned(),
        total_memory_mb,
    }
}
