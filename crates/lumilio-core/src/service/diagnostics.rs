use super::LauncherService;
use super::error::ServiceError;
use super::support::blocking;
use super::types::{EXPORT_LOG_BYTES, GameLogs, LOG_TAIL_BYTES, REPORT_BYTES};
use crate::diagnostics::{Problem, ProblemKind};
use crate::history::HistoryLog;
use crate::inspect::inspect_instance;
use crate::transfer::Transport;
use lumilio_plugin_api::{AnalysisInput, AnalysisSource, GameFacts, ModFact};
use std::path::Path;

impl<T: Transport + Clone> LauncherService<T> {
    /// The instance's history, oldest first. Reading needs no lease; a log
    /// that cannot be read is an error, not an empty history.
    pub async fn history(&self, id: &str) -> Result<crate::history::HistoryRead, ServiceError> {
        self.instance(id).await?;
        let (root, id) = (self.layout.root().to_path_buf(), id.to_owned());
        tokio::task::spawn_blocking(move || HistoryLog::for_instance(&root, &id).read())
            .await
            .map_err(std::io::Error::other)?
            .map_err(ServiceError::from)
    }

    /// The end of `logs/latest.log` (at most [`LOG_TAIL_BYTES`]) and the crash
    /// reports, newest first. Reading needs no lease; a missing log is `None`.
    pub async fn logs(&self, id: &str) -> Result<GameLogs, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        tokio::task::spawn_blocking(move || {
            Ok::<_, std::io::Error>(GameLogs {
                latest: crate::diagnostics::read_latest_log(&game_dir, LOG_TAIL_BYTES)?,
                crashes: crate::diagnostics::list_crash_reports(&game_dir)?,
            })
        })
        .await
        .map_err(std::io::Error::other)?
        .map_err(ServiceError::from)
    }

    /// One crash report's text (cut at [`REPORT_BYTES`]) with the likely causes
    /// recognised in it. Names that are not plain file names are refused.
    pub async fn crash_report(
        &self,
        id: &str,
        file_name: &str,
    ) -> Result<(String, Vec<crate::plugins::PluginFinding>), ServiceError> {
        let record = self.instance(id).await?;
        let (_, runtimes) = self.environment().await;
        let layout = self.layout.clone();
        let (game_dir, name) = (self.layout.game(id), file_name.to_owned());
        let (text, game) = tokio::task::spawn_blocking(move || {
            let text = crate::diagnostics::read_crash_report(&game_dir, &name, REPORT_BYTES)?;
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

    /// How much disk the game's own folder takes (mods, saves, options…),
    /// not counting game files shared with other games. Walks the disk.
    pub async fn instance_size(&self, id: &str) -> Result<u64, ServiceError> {
        self.instance(id).await?;
        let game_dir = self.layout.game(id);
        Ok(
            tokio::task::spawn_blocking(move || crate::storage::directory_size(&game_dir))
                .await
                .map_err(std::io::Error::other)?,
        )
    }

    /// A folder of the game directory (`relative` empty is the directory itself),
    /// folders first. Only plain names inside the game directory are accepted.
    pub async fn list_files(
        &self,
        id: &str,
        relative: &str,
    ) -> Result<Vec<crate::diagnostics::FileEntry>, ServiceError> {
        self.instance(id).await?;
        let (game_dir, relative) = (self.layout.game(id), relative.to_owned());
        Ok(blocking(move || crate::diagnostics::list_dir(&game_dir, &relative)).await?)
    }

    /// What is wrong with this instance right now, most severe first.
    pub async fn problems(&self, id: &str) -> Result<Vec<Problem>, ServiceError> {
        let record = self.instance(id).await?;
        let (settings, runtimes) = self.environment().await;
        Ok(self
            .inspect_with_plugins(&settings, &runtimes, &record)
            .await)
    }

    /// Shared by the instance problem list and Home, preserving built-in
    /// checks even when every analyzer is disabled or broken.
    pub(super) async fn inspect_with_plugins(
        &self,
        settings: &crate::settings::LauncherSettings,
        runtimes: &[crate::java::JavaRuntime],
        record: &crate::instance::InstanceRecord,
    ) -> Vec<Problem> {
        let mut problems = inspect_instance(&self.layout, settings, runtimes, record).await;
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

    /// Writes a zip for a bug report to `destination`: versions, settings
    /// summary, Java list, recent tasks and each game's latest log, with
    /// player names, profile ids and folders replaced.
    /// Hides what identifies the person: account names and ids, the launcher's
    /// folder and the home folder.
    pub(super) fn redactor(
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
    pub async fn export_log(
        &self,
        id: &str,
        crash_report: Option<&str>,
        destination: &Path,
    ) -> Result<(), ServiceError> {
        self.instance(id).await?;
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

    pub async fn export_diagnostics(&self, destination: &Path) -> Result<(), ServiceError> {
        use std::fmt::Write as _;
        let (settings, _) = self.environment().await;
        let installations = self.java_installations().await;
        let instances = self.library().await.instances;
        let activity = self.activity(50);

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
            "mirrors: {} (preferred: {})",
            settings.mirrors.len(),
            settings.prefer_mirrors
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
                task.label, task.category, task.outcome
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
fn game_facts(
    layout: &crate::layout::Layout,
    record: &crate::instance::InstanceRecord,
    runtimes: &[crate::java::JavaRuntime],
) -> GameFacts {
    use crate::instance::Loader;
    let directories = layout.launch_directories(record);
    let required = record.release_id().and_then(|id| {
        std::fs::read_to_string(directories.versions().join(&id).join(format!("{id}.json")))
            .ok()
            .and_then(|text| crate::release::ReleaseManifest::decode_json(&text).ok())
            .and_then(|release| release.java_requirement().map(|java| java.major()))
    });
    let java_major = crate::java::choose(runtimes, required, record.settings.java_path.as_deref())
        .map(|java| java.major());
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
