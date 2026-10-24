use std::path::Path;

use lumilio_plugin_api::{AnalysisInput, AnalysisSource, GameFacts, ModFact};

use std::sync::Arc;

use tokio::sync::Mutex;

use super::{CrashReport, FileEntry, Problem, ProblemKind};
use crate::activity::ActivityView;
use crate::blocking::blocking;
use crate::error::ServiceError;
use crate::history::HistoryLog;
use crate::inspect::inspect_instance;
use crate::instances::Instances;
use crate::java::JavaRuntime;
use crate::layout::Layout;
use crate::plugins::PluginHost;
use crate::settings::SettingsStore;
use crate::translation::SharedTranslation;

/// How much of the log's end `logs` returns.
pub(crate) const LOG_TAIL_BYTES: u64 = 64 * 1024;
/// How much of a log an export carries.
const EXPORT_LOG_BYTES: u64 = 8 * 1024 * 1024;
/// How much of a crash report `crash_report` returns.
const REPORT_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameLogs {
    pub latest: Option<String>,
    pub crashes: Vec<CrashReport>,
    pub files: Vec<FileEntry>,
}

/// An instance-local source; file names are validated before any disk access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameLogSource {
    Live,
    Latest,
    File(String),
    Crash(String),
}

/// One game's logs and problems, bug-report exports, and the disk the
/// launcher uses.
pub(crate) struct Diagnostics {
    pub(crate) settings: Arc<Mutex<SettingsStore>>,
    pub(crate) instances: Arc<Instances>,
    pub(crate) layout: Layout,
    pub(crate) plugins: Arc<PluginHost>,
    /// Emptied with the cache folder.
    pub(crate) translation: SharedTranslation,
}

impl Diagnostics {
    #[cfg(test)]
    pub(crate) fn replace_plugins(&mut self, plugins: Arc<PluginHost>) {
        self.plugins = plugins;
    }

    pub(crate) fn new(
        settings: Arc<Mutex<SettingsStore>>,
        instances: Arc<Instances>,
        layout: Layout,
        plugins: Arc<PluginHost>,
        translation: SharedTranslation,
    ) -> Self {
        Self {
            settings,
            instances,
            layout,
            plugins,
            translation,
        }
    }
}

impl Diagnostics {
    /// The instance's history, oldest first. Reading needs no lease; a log
    /// that cannot be read is an error, not an empty history.
    pub(crate) async fn history(
        &self,
        id: &str,
    ) -> Result<crate::history::HistoryRead, ServiceError> {
        self.instances.record(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || HistoryLog::for_instance(&root, &id).read())
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// The complete `logs/latest.log` and the crash
    /// reports, newest first. Reading needs no lease; a missing log is `None`.
    pub(crate) async fn logs(&self, id: &str) -> Result<GameLogs, ServiceError> {
        self.instances.record(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || {
            Ok::<_, std::io::Error>(GameLogs {
                latest: match crate::diagnostics::read_game_log(&game_dir, "latest.log") {
                    Ok(text) => Some(text),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error),
                },
                crashes: crate::diagnostics::list_crash_reports(&game_dir)?,
                files: crate::diagnostics::list_game_logs(&game_dir)?,
            })
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }

    /// One complete crash report's text with the likely causes
    /// recognised in it. Names that are not plain file names are refused.
    pub(crate) async fn crash_report(
        &self,
        id: &str,
        file_name: &str,
        runtimes: Vec<JavaRuntime>,
    ) -> Result<(String, Vec<crate::plugins::PluginFinding>), ServiceError> {
        let record = self.instances.record(id).await?;
        let layout = self.layout.clone();
        let (game_dir, name) = (self.layout.game(id), file_name.to_owned());
        let (text, game) = tokio::task::spawn_blocking(move || {
            let text = crate::diagnostics::read_complete_crash(&game_dir, &name)?;
            Ok::<_, std::io::Error>((text, game_facts(&layout, &record, &runtimes)))
        })
        .await
        .map_err(std::io::Error::other)??;
        let findings = self
            .plugins
            .analyze(AnalysisInput {
                text: text.clone(),
                source: AnalysisSource::CrashReport,
                game,
            })
            .await;
        Ok((text, findings))
    }

    /// Reads an entire selected file. Large files fail explicitly instead of silently truncating.
    pub(crate) async fn game_log(
        &self,
        id: &str,
        source: GameLogSource,
    ) -> Result<String, ServiceError> {
        self.instances.record(id).await?;
        let game_dir = self.layout.game(id);
        Ok(blocking(move || match source {
            GameLogSource::Latest => crate::diagnostics::read_game_log(&game_dir, "latest.log"),
            GameLogSource::File(name) => crate::diagnostics::read_game_log(&game_dir, &name),
            GameLogSource::Crash(name) => crate::diagnostics::read_complete_crash(&game_dir, &name),
            GameLogSource::Live => Err(std::io::Error::other(
                "live output is supplied by the running process",
            )),
        })
        .await?)
    }

    /// Analyzes a snapshot and returns only redacted text and findings for sharing.
    pub(crate) async fn analyze_game_log(
        &self,
        id: &str,
        text: String,
        crash: bool,
        runtimes: Vec<JavaRuntime>,
    ) -> Result<(String, Vec<crate::plugins::PluginFinding>), ServiceError> {
        let record = self.instances.record(id).await?;
        let settings = self.settings.lock().await.get().clone();
        let mut redactor = self.redactor(&settings);
        redactor.hide_path(&self.layout.game(id), "<game>");
        let text = redactor.apply(&text);
        let layout = self.layout.clone();
        let game = blocking(move || Ok(game_facts(&layout, &record, &runtimes))).await?;
        let mut findings = self
            .plugins
            .analyze(AnalysisInput {
                text: text.clone(),
                source: if crash {
                    AnalysisSource::CrashReport
                } else {
                    AnalysisSource::LatestLog
                },
                game,
            })
            .await;
        for result in &mut findings {
            result.finding.title = redactor.apply(&result.finding.title);
            result.finding.advice = redactor.apply(&result.finding.advice);
            result.finding.evidence = result
                .finding
                .evidence
                .as_ref()
                .map(|text| redactor.apply(text));
        }
        Ok((text, findings))
    }

    /// Exports the complete selected source, independent of reading filters.
    pub(crate) async fn export_game_log(
        &self,
        id: &str,
        source: GameLogSource,
        live: String,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        let text = if source == GameLogSource::Live {
            self.instances.record(id).await?;
            live
        } else {
            self.game_log(id, source).await?
        };
        let settings = self.settings.lock().await.get().clone();
        let mut redactor = self.redactor(&settings);
        redactor.hide_path(&self.layout.game(id), "<game>");
        let destination = destination.to_owned();
        blocking(move || {
            let part = destination.with_extension("log.part");
            let result = std::fs::write(&part, redactor.apply(&text))
                .and_then(|()| std::fs::rename(&part, &destination));
            if result.is_err() {
                let _ = std::fs::remove_file(part);
            }
            result
        })
        .await?;
        Ok(())
    }

    /// How much disk the game's own folder takes (mods, saves, options…),
    /// not counting game files shared with other games. Walks the disk.
    pub(crate) async fn instance_size(&self, id: &str) -> Result<u64, ServiceError> {
        self.instances.record(id).await?;
        let game_dir = self.layout.game(id);
        Ok(
            tokio::task::spawn_blocking(move || crate::storage::directory_size(&game_dir))
                .await
                .map_err(std::io::Error::other)?,
        )
    }

    /// A folder of the game directory (`relative` empty is the directory itself),
    /// folders first. Only plain names inside the game directory are accepted.
    pub(crate) async fn list_files(
        &self,
        id: &str,
        relative: &str,
    ) -> Result<Vec<crate::diagnostics::FileEntry>, ServiceError> {
        self.instances.record(id).await?;
        let (game_dir, relative) = (self.layout.game(id), relative.to_owned());
        Ok(blocking(move || crate::diagnostics::list_dir(&game_dir, &relative)).await?)
    }

    /// What is wrong with this instance right now, most severe first.
    pub(crate) async fn problems(
        &self,
        id: &str,
        runtimes: Vec<JavaRuntime>,
    ) -> Result<Vec<Problem>, ServiceError> {
        let record = self.instances.record(id).await?;
        let settings = self.settings.lock().await.get().clone();
        Ok(self
            .inspect_with_plugins(&settings, &runtimes, &record)
            .await)
    }

    /// Shared by the instance problem list and Home, preserving built-in
    /// checks even when every analyzer is disabled or broken.
    pub(crate) async fn inspect_with_plugins(
        &self,
        settings: &crate::settings::LauncherSettings,
        runtimes: &[crate::java::JavaRuntime],
        record: &crate::instances::InstanceRecord,
    ) -> Vec<Problem> {
        self.inspect_with_analysis(settings, runtimes, record, true)
            .await
    }

    pub(crate) async fn inspect_with_analysis(
        &self,
        settings: &crate::settings::LauncherSettings,
        runtimes: &[crate::java::JavaRuntime],
        record: &crate::instances::InstanceRecord,
        verify_files: bool,
    ) -> Vec<Problem> {
        let mut problems = if verify_files {
            inspect_instance(&self.layout, settings, runtimes, record).await
        } else {
            crate::inspect::inspect_home_instance(&self.layout, settings, runtimes, record).await
        };
        if problems
            .iter()
            .any(|problem| matches!(problem.kind, ProblemKind::LastSessionFailed(_)))
        {
            let (layout, record, runtimes) =
                (self.layout.clone(), record.clone(), runtimes.to_vec());
            let input = blocking(move || {
                let game_dir = layout.launch_directories(&record).game().to_owned();
                let reports = crate::diagnostics::list_crash_reports(&game_dir)?;
                let (text, source) = if let Some(report) = reports.first() {
                    (
                        Some(crate::diagnostics::read_crash_report(
                            &game_dir,
                            &report.file_name,
                            REPORT_BYTES,
                        )?),
                        AnalysisSource::CrashReport,
                    )
                } else {
                    (
                        crate::diagnostics::read_latest_log(&game_dir, LOG_TAIL_BYTES)?,
                        AnalysisSource::LatestLog,
                    )
                };
                Ok(text.map(|text| AnalysisInput {
                    text,
                    source,
                    game: game_facts(&layout, &record, &runtimes),
                }))
            })
            .await;
            if let Ok(Some(input)) = input {
                problems.extend(
                    self.plugins
                        .analyze(input)
                        .await
                        .into_iter()
                        .map(|finding| Problem {
                            severity: finding.finding.severity,
                            kind: ProblemKind::Finding(finding),
                        }),
                );
                problems.sort_by_key(|problem| std::cmp::Reverse(problem.severity));
            }
        }
        problems
    }

    /// Hides what identifies the person: account names and ids, the launcher's
    /// folder and the home folder.
    pub(crate) fn redactor(
        &self,
        settings: &crate::settings::LauncherSettings,
    ) -> crate::bundle::Redactor {
        let mut redactor = crate::bundle::Redactor::new();
        for entry in &settings.accounts {
            redactor.hide(&entry.name, "<player>");
            if let Ok(profile) = entry.profile() {
                redactor.hide(&profile.id().to_string(), "<uuid>");
                redactor.hide(&profile.id().compact(), "<uuid>");
            }
        }
        redactor.hide_path(self.layout.root(), "<launcher>");
        for variable in ["HOME", "USERPROFILE"] {
            if let Some(home) = std::env::var_os(variable) {
                redactor.hide_path(Path::new(&home), "~");
            }
        }
        redactor
    }

    /// Saves the game's latest log (or one crash report, by file name) as text
    /// at `destination` with player names, ids and folders replaced, so it can
    /// be sent to someone. Written through a `.part` file.
    pub(crate) async fn export_log(
        &self,
        id: &str,
        crash_report: Option<&str>,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        self.instances.record(id).await?;
        let settings = self.settings.lock().await.get().clone();
        let redactor = self.redactor(&settings);
        let (game_dir, destination) = (self.layout.game(id), destination.to_owned());
        let crash = crash_report.map(str::to_owned);
        blocking(move || {
            let text = match &crash {
                Some(name) => {
                    crate::diagnostics::read_crash_report(&game_dir, name, EXPORT_LOG_BYTES)?
                }
                None => crate::diagnostics::read_latest_log(&game_dir, EXPORT_LOG_BYTES)?
                    .ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::NotFound, "there is no log yet")
                    })?,
            };
            // Paths of this very game, however spelled, are no use to a reader.
            let mut redactor = redactor;
            redactor.hide_path(&game_dir, "<game>");
            let part = destination.with_extension("txt.part");
            let written = std::fs::write(&part, redactor.apply(&text))
                .and_then(|()| std::fs::rename(&part, &destination));
            if written.is_err() {
                let _ = std::fs::remove_file(&part);
            }
            written
        })
        .await?;
        Ok(())
    }

    /// Writes a zip for a bug report to `destination`: versions, settings
    /// summary, the Java `installations`, recent tasks from `activity` and each
    /// game's latest log, with player names, profile ids and folders replaced.
    pub(crate) async fn export_diagnostics(
        &self,
        destination: &Path,
        installations: Vec<(JavaRuntime, bool)>,
        activity: ActivityView,
    ) -> Result<(), ServiceError> {
        use std::fmt::Write as _;
        let settings = self.settings.lock().await.get().clone();
        let instances = self.instances.lock().await.instances().to_vec();

        let redactor = self.redactor(&settings);

        let mut about = String::new();
        let _ = writeln!(about, "LumilioCL {}", env!("CARGO_PKG_VERSION"));
        let _ = writeln!(
            about,
            "system: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        let _ = writeln!(about, "instances: {}", instances.len());
        let _ = writeln!(about, "accounts: {}", settings.accounts.len());

        let mut summary = String::new();
        let launch = &settings.launch;
        let _ = writeln!(
            summary,
            "memory: min {:?} MB, max {:?} MB",
            settings.default_min_memory_mb, settings.default_max_memory_mb
        );
        let _ = writeln!(summary, "preferences: {:?}", settings.preferences);
        let _ = writeln!(
            summary,
            "download concurrency: {:?}",
            settings.download_concurrency
        );
        let _ = writeln!(
            summary,
            "mirrors: {} (source preference: {:?})",
            settings.mirrors.len(),
            settings.download_source_preference()
        );
        let _ = writeln!(
            summary,
            "launch defaults: window {:?}x{:?}, fullscreen {:?}, {} JVM arguments, {} game arguments, {} environment variables, pre-launch {}, wrapper {}, post-exit {}",
            launch.window_width,
            launch.window_height,
            launch.fullscreen,
            launch.jvm_arguments.len(),
            launch.game_arguments.len(),
            launch.environment.len(),
            launch.pre_launch.is_some(),
            launch.wrapper.is_some(),
            launch.post_exit.is_some(),
        );

        let mut java = String::new();
        for (runtime, disabled) in &installations {
            let _ = writeln!(
                java,
                "Java {} {} {}{} — {}",
                runtime.version(),
                runtime.vendor().unwrap_or("unknown vendor"),
                runtime.architecture().unwrap_or("unknown arch"),
                if *disabled { " (turned off)" } else { "" },
                runtime.home().display()
            );
        }

        let mut list = String::new();
        for record in &instances {
            let _ = writeln!(
                list,
                "{} · {} · {:?} {} · installed: {} · played {} s",
                record.id,
                record.game_version,
                record.loader,
                record.loader_version.as_deref().unwrap_or("-"),
                record.installed,
                record.play_seconds,
            );
        }

        let mut tasks = String::new();
        for task in &activity.finished {
            let _ = writeln!(
                tasks,
                "{} · {:?} · {:?}",
                task.label.text(),
                task.category,
                task.outcome
            );
        }

        let mut files = vec![
            ("about.txt".to_owned(), about),
            ("settings.txt".to_owned(), summary),
            ("java.txt".to_owned(), java),
            ("instances.txt".to_owned(), list),
            ("activity.txt".to_owned(), tasks),
        ];
        for record in &instances {
            if let Ok(logs) = self.logs(&record.id).await
                && let Some(latest) = logs.latest
            {
                files.push((format!("logs/{}-latest.log", record.id), latest));
            }
            // What the game printed in its last launch, also when it quit
            // before writing a log of its own.
            let output = GameLogSource::File(crate::GAME_OUTPUT_LOG.to_owned());
            if let Ok(text) = self.game_log(&record.id, output).await {
                files.push((
                    format!("logs/{}-{}", record.id, crate::GAME_OUTPUT_LOG),
                    text,
                ));
            }
        }
        // The launcher's own log, and the one before it once that filled up.
        for path in [
            self.layout.launcher_log(),
            self.layout.earlier_launcher_log(),
        ] {
            if let Ok(bytes) = tokio::fs::read(&path).await
                && let Some(name) = path.file_name()
            {
                files.push((
                    format!("launcher/{}", name.to_string_lossy()),
                    String::from_utf8_lossy(&bytes).into_owned(),
                ));
            }
        }
        for (_, text) in &mut files {
            *text = redactor.apply(text);
        }
        let destination = destination.to_owned();
        tokio::task::spawn_blocking(move || crate::bundle::write_bundle(&destination, &files))
            .await
            .map_err(std::io::Error::other)??;
        Ok(())
    }
}

/// Runs on a blocking worker. Facts contain no host paths or UI types.
pub(crate) fn game_facts(
    layout: &crate::layout::Layout,
    record: &crate::instances::InstanceRecord,
    runtimes: &[crate::java::JavaRuntime],
) -> GameFacts {
    use crate::instances::Loader;
    let directories = layout.launch_directories(record);
    let needs = record
        .release_id()
        .and_then(|id| {
            std::fs::read_to_string(directories.versions().join(&id).join(format!("{id}.json")))
                .ok()
        })
        .and_then(|text| lumilio_minecraft::ReleaseManifest::decode_json(&text).ok())
        .map_or_else(
            || crate::java::JavaNeeds::new(&record.game_version, record.loader, None),
            |release| crate::java::JavaNeeds::of_release(record, &release),
        );
    let java_major = crate::java::choose(
        runtimes,
        &needs,
        record.settings.java_path.as_deref(),
        &lumilio_minecraft::HostProfile::current(),
    )
    .map(|choice| choice.runtime.major());
    let mods = crate::content::scan(directories.game(), crate::discover::ProjectKind::Mod)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.enabled && !item.is_directory)
        .filter_map(|item| {
            let metadata = crate::content::read_mod_metadata(
                &directories.game().join("mods").join(&item.file_name),
            )?;
            Some(ModFact {
                id: metadata.id,
                version: metadata.version,
                file: item.file_name,
            })
        })
        .collect();
    GameFacts {
        game_version: record.game_version.clone(),
        loader: match record.loader {
            Loader::Vanilla => "vanilla",
            Loader::Fabric => "fabric",
            Loader::Forge => "forge",
            Loader::NeoForge => "neoforge",
            Loader::Quilt => "quilt",
        }
        .into(),
        loader_version: record.loader_version.clone(),
        java_major,
        mods,
    }
}
