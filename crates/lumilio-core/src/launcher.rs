//! The launch service: from an instance record to a running game.
//!
//! It composes the catalog, release, install, repair-scan, Java, and process
//! pieces and reports everything as [`LaunchUpdate`]s, so a UI only has to
//! render a [`crate::LaunchSession`]. Behavior notes:
//! `docs/behavior/launcher-spine.md`.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use tokio::sync::{broadcast, mpsc};

use crate::account::AuthSession;
use crate::activity::CancellationToken;
use crate::environment::HostProfile;
use crate::fetch::{FetchError, fetch_document};
use crate::forge_install::{self, ForgeError};
use crate::install::{
    AssetIndex, InstallError, InstallEvent, InstallStage, InstallationPlan, Installer,
};
use crate::instance::{InstanceRecord, Loader};
use crate::java::{JavaRuntime, choose};
use crate::launch::{LaunchContext, LaunchDirectories, LaunchError, LaunchPlan};
use crate::launch_session::{LaunchFailure, LaunchPhase, LaunchSignal};
use crate::loader::{LAUNCHABLE_LOADERS, normalize_profile};
use crate::process::{self, GameEvent, GameExit, GameOptions, LogStream, ProcessError};
use crate::release::{ReleaseError, ReleaseManifest, ReleaseSet};
use crate::repair::InstallationVerifier;
use crate::transfer::{SourceChain, TransferEngine, TransferEvent, TransferRequest, Transport};
use crate::tuning::{LaunchTuning, QuickPlay};

/// Shown in the game's debug screen as the launcher brand.
/// Where the game may record how a direct start went, inside the game folder.
const QUICK_PLAY_LOG: &str = "quickPlay/lumilio.json";

const LAUNCHER_NAME: &str = "LumilioCL";

/// Everything a launch needs that is not part of the instance record.
#[derive(Clone, Debug)]
pub struct LaunchRequest {
    pub instance: InstanceRecord,
    pub directories: LaunchDirectories,
    /// Who plays: the identity values the game is started with.
    pub session: AuthSession,
    /// Where to fetch the release manifest when it is not on disk yet.
    pub manifest_url: Option<String>,
    /// Where to fetch the loader profile for a loader instance that is not
    /// installed yet (see `loader_profile_url`); for Forge and NeoForge, the
    /// address of their installer.
    pub loader_profile_url: Option<String>,
    pub runtimes: Vec<JavaRuntime>,
    /// Launcher-wide memory defaults; the instance's own settings win.
    pub default_max_memory_mb: Option<u32>,
    pub default_min_memory_mb: Option<u32>,
    /// How the game starts (launcher defaults; the instance's own JVM
    /// arguments are applied on top).
    pub tuning: LaunchTuning,
    /// Files downloaded at once; `None` leaves it to the source chain.
    pub download_concurrency: Option<u32>,
    /// Where to go once the game is up; `None` is the main menu.
    pub quick_play: Option<QuickPlay>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LaunchUpdate {
    Signal(LaunchSignal),
    Log { stream: LogStream, text: String },
}

#[derive(Debug)]
pub enum LaunchServiceError {
    /// This loader cannot be installed or started yet.
    LoaderUnsupported(Loader),
    /// A loader instance without a loader version cannot name its release.
    LoaderVersionMissing,
    InvalidMemory,
    /// The manifest is neither on disk nor fetchable.
    ManifestUnavailable(String),
    Release(ReleaseError),
    Launch(LaunchError),
    Install(InstallError),
    /// No installed Java satisfies the release; carries the required major.
    NoJava {
        required: Option<u32>,
    },
    Process(ProcessError),
    /// This game version cannot start in a world or on a server directly.
    QuickPlayUnsupported {
        /// `"singleplayer"` or `"multiplayer"`.
        mode: &'static str,
    },
    /// The world to start in is not among the saves.
    QuickPlayWorldMissing(String),
    /// A command the user set up did not succeed.
    Hook {
        /// `"before launch"` or `"after exit"`.
        when: &'static str,
        code: Option<i32>,
        stopped: bool,
    },
    /// Preparing Forge or NeoForge from its installer failed.
    Loader(ForgeError),
    Cancelled,
}

impl Display for LaunchServiceError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::LoaderUnsupported(loader) => {
                write!(f, "{loader:?} instances cannot be launched yet")
            }
            Self::LoaderVersionMissing => f.write_str("the instance has no loader version"),
            Self::InvalidMemory => f.write_str("effective memory values are out of range"),
            Self::ManifestUnavailable(reason) => {
                write!(f, "release manifest unavailable: {reason}")
            }
            Self::Release(error) => write!(f, "{error}"),
            Self::Launch(error) => write!(f, "{error}"),
            Self::Install(error) => write!(f, "{error}"),
            Self::NoJava {
                required: Some(major),
            } => write!(f, "Java {major} or newer is required but not installed"),
            Self::NoJava { required: None } => f.write_str("no Java installation found"),
            Self::Process(error) => write!(f, "{error}"),
            Self::QuickPlayUnsupported { mode } => write!(
                f,
                "this game version cannot start directly in {mode}; start it from the menu"
            ),
            Self::QuickPlayWorldMissing(name) => {
                write!(f, "there is no saved world {name:?} to start in")
            }
            Self::Hook {
                when,
                stopped: true,
                ..
            } => {
                write!(f, "the command {when} was stopped before it finished")
            }
            Self::Hook {
                when,
                code: Some(code),
                ..
            } => {
                write!(f, "the command {when} failed (exit code {code})")
            }
            Self::Hook { when, .. } => write!(f, "the command {when} failed"),
            Self::Loader(error) => write!(f, "{error}"),
            Self::Cancelled => f.write_str("launch cancelled"),
        }
    }
}

impl Error for LaunchServiceError {}

pub struct Launcher<T> {
    transport: T,
    sources: SourceChain,
    concurrency: usize,
}

impl<T> Launcher<T>
where
    T: Transport + Clone,
{
    #[must_use]
    pub fn new(transport: T, sources: SourceChain) -> Self {
        let concurrency = sources.preferred_concurrency();
        Self {
            transport,
            sources,
            concurrency,
        }
    }

    /// Downloads this many files at once instead of the source chain's choice.
    #[must_use]
    pub fn with_concurrency(mut self, concurrency: Option<u32>) -> Self {
        if let Some(count) = concurrency.filter(|count| *count > 0) {
            self.concurrency = count as usize;
        }
        self
    }

    /// Prepares and starts the game, returning how the process ended.
    ///
    /// Every failure is also reported as a terminal signal, so a session that
    /// only listens to `updates` still ends. Cancelling stops downloads and
    /// kills a running game.
    pub async fn launch(
        &self,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<GameExit, LaunchServiceError> {
        let result = self.launch_inner(request, &updates, &cancel).await;
        match &result {
            Ok(_) => {}
            Err(LaunchServiceError::Cancelled) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Cancelled));
            }
            Err(error) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(
                    LaunchFailure::Step {
                        message: error.to_string(),
                    },
                )));
            }
        }
        result
    }

    /// Installs everything the instance needs and stops there: no Java is
    /// chosen and no process is created. Like [`Self::launch`], every failure
    /// also arrives as a terminal signal.
    pub async fn install(
        &self,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), LaunchServiceError> {
        let result = self.prepare(&request, &updates, &cancel).await.map(drop);
        match &result {
            Ok(()) => {}
            Err(LaunchServiceError::Cancelled) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Cancelled));
            }
            Err(error) => {
                let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Failed(
                    LaunchFailure::Step {
                        message: error.to_string(),
                    },
                )));
            }
        }
        result
    }

    /// Verifies the request, resolves the release and installs whatever is
    /// missing or damaged. Returns the plan the game would start with.
    async fn prepare(
        &self,
        request: &LaunchRequest,
        updates: &mpsc::UnboundedSender<LaunchUpdate>,
        cancel: &CancellationToken,
    ) -> Result<LaunchPlan, LaunchServiceError> {
        let signal = |signal: LaunchSignal| {
            let _ = updates.send(LaunchUpdate::Signal(signal));
        };
        signal(LaunchSignal::Phase(LaunchPhase::Verifying));

        if !LAUNCHABLE_LOADERS.contains(&request.instance.loader) {
            return Err(LaunchServiceError::LoaderUnsupported(
                request.instance.loader,
            ));
        }

        let settings = &request.instance.settings;
        let min_memory = settings.min_memory_mb.or(request.default_min_memory_mb);
        let max_memory = settings.max_memory_mb.or(request.default_max_memory_mb);
        crate::settings::validate_memory(min_memory, max_memory)
            .map_err(|_| LaunchServiceError::InvalidMemory)?;

        // Fetching the manifest can stall on a bad network; cancelling must
        // not wait for it.
        let release = tokio::select! {
            () = cancel.cancelled() => return Err(LaunchServiceError::Cancelled),
            release = self.load_release(request) => release?,
        };
        let mut context = request.session.apply(
            LaunchContext::new(HostProfile::current(), request.directories.clone())
                .with_value("version_type", LAUNCHER_NAME),
        );
        // The release's own rules decide whether (and how) it can start in a
        // world or on a server: only versions that declare the feature get
        // the arguments.
        match &request.quick_play {
            Some(QuickPlay::World(name)) => {
                let saved = request.directories.game().join("saves").join(name);
                if !saved.is_dir() {
                    return Err(LaunchServiceError::QuickPlayWorldMissing(name.clone()));
                }
                context = context
                    .with_feature("has_quick_plays_support", true)
                    .with_feature("is_quick_play_singleplayer", true)
                    .with_value("quickPlayPath", QUICK_PLAY_LOG)
                    .with_value("quickPlaySingleplayer", name);
            }
            Some(QuickPlay::Server(address)) => {
                context = context
                    .with_feature("has_quick_plays_support", true)
                    .with_feature("is_quick_play_multiplayer", true)
                    .with_value("quickPlayPath", QUICK_PLAY_LOG)
                    .with_value("quickPlayMultiplayer", address);
            }
            None => {}
        }
        let launch_plan = release
            .build_launch_plan(&context)
            .map_err(LaunchServiceError::Launch)?;
        if matches!(request.quick_play, Some(QuickPlay::World(_)))
            && !launch_plan
                .game_arguments()
                .iter()
                .any(|argument| argument == "--quickPlaySingleplayer")
        {
            return Err(LaunchServiceError::QuickPlayUnsupported {
                mode: "singleplayer",
            });
        }
        let install_plan = InstallationPlan::build(
            &release,
            &launch_plan,
            request.directories.clone(),
            self.sources.clone(),
            None,
        )
        .map_err(LaunchServiceError::Install)?;

        let up_to_date = install_plan.manifest_destination().is_file()
            && InstallationVerifier::scan(&install_plan, None)
                .await
                .is_ok_and(|repair| repair.is_empty() && !repair.republishes_natives());
        if !up_to_date {
            self.run_installer(&install_plan, updates, cancel).await?;
        }
        if uses_installer(request.instance.loader) {
            self.finish_loader(request, &launch_plan, updates, cancel)
                .await?;
        }
        if cancel.is_cancelled() {
            return Err(LaunchServiceError::Cancelled);
        }
        Ok(launch_plan)
    }

    /// Forge and NeoForge: the installer's processors turn the vanilla client
    /// into the patched one the loader starts from. Runs only while that
    /// output is missing, so a damaged install heals on the next launch.
    async fn finish_loader(
        &self,
        request: &LaunchRequest,
        launch_plan: &LaunchPlan,
        updates: &mpsc::UnboundedSender<LaunchUpdate>,
        cancel: &CancellationToken,
    ) -> Result<(), LaunchServiceError> {
        let record = &request.instance;
        let id = record
            .release_id()
            .ok_or(LaunchServiceError::LoaderVersionMissing)?;
        let versions = request.directories.versions();
        let libraries = request.directories.libraries();
        let installer = forge_install::installer_path(versions, &id);
        self.ensure_installer(&installer, request.loader_profile_url.as_deref())
            .await?;
        let read = forge_install::read_installer(&installer).map_err(LaunchServiceError::Loader)?;
        if forge_install::patched(&read.profile, libraries) {
            return Ok(());
        }
        let log = {
            let updates = updates.clone();
            move |text: String| {
                let _ = updates.send(LaunchUpdate::Log {
                    stream: LogStream::Stdout,
                    text,
                });
            }
        };
        log("preparing the loader from its installer".to_owned());

        // The tools and data the processors need.
        let host = HostProfile::current();
        let mut requests = Vec::new();
        for library in &read.profile.libraries {
            let file = library.resolve_file(&host);
            let destination = libraries.join(&file.relative_path);
            let intact = match file.download.sha1() {
                Some(digest) => crate::content::sha1_hex(&destination)
                    .is_ok_and(|actual| actual.eq_ignore_ascii_case(digest)),
                None => destination.is_file(),
            };
            if intact
                || forge_install::extract_embedded(&installer, &file.relative_path, &destination)
            {
                continue;
            }
            let url = file.download.url().ok_or_else(|| {
                LaunchServiceError::Loader(ForgeError::Profile(format!(
                    "{} has no address",
                    file.relative_path
                )))
            })?;
            let mut transfer = TransferRequest::new(
                file.relative_path.clone(),
                self.sources.candidates(url),
                destination,
            )
            .map_err(|error| LaunchServiceError::Install(InstallError::from(error)))?;
            if let Some(digest) = file.download.sha1() {
                transfer = transfer
                    .expect_sha1(digest)
                    .map_err(|error| LaunchServiceError::Install(InstallError::from(error)))?;
            }
            requests.push(transfer);
        }
        if !requests.is_empty() {
            let engine = TransferEngine::new(self.transport.clone(), self.concurrency)
                .map_err(|error| LaunchServiceError::Install(InstallError::from(error)))?;
            let report = engine.transfer_batch(requests, cancel.clone()).await;
            if cancel.is_cancelled() {
                return Err(LaunchServiceError::Cancelled);
            }
            if let Some((name, Err(error))) =
                report.results().iter().find(|(_, result)| result.is_err())
            {
                return Err(LaunchServiceError::Loader(ForgeError::Profile(format!(
                    "{name}: {error}"
                ))));
            }
        }

        let required = launch_plan.java_requirement().map(|java| java.major());
        let java = choose(
            &request.runtimes,
            required,
            record.settings.java_path.as_deref(),
        )
        .ok_or(LaunchServiceError::NoJava { required })?;
        let minecraft_jar = versions.join(&id).join(format!("{id}.jar"));
        let work = versions.join(&id).join("installer-work");
        let root = versions.parent().unwrap_or(versions);
        let paths = forge_install::Paths {
            installer: &installer,
            libraries,
            minecraft_jar: &minecraft_jar,
            root,
            work: &work,
        };
        let result = forge_install::run_processors(
            &read.profile,
            &record.game_version,
            &paths,
            java.executable(),
            &log,
            cancel,
        )
        .await;
        let _ = tokio::fs::remove_dir_all(&work).await;
        match result {
            Ok(()) if forge_install::patched(&read.profile, libraries) => Ok(()),
            Ok(()) => Err(LaunchServiceError::Loader(ForgeError::Output(
                "the patched client is missing after the processors ran".to_owned(),
            ))),
            Err(ForgeError::Cancelled) => Err(LaunchServiceError::Cancelled),
            Err(error) => Err(LaunchServiceError::Loader(error)),
        }
    }

    /// Downloads the installer next to the release unless it is already there.
    async fn ensure_installer(
        &self,
        installer: &std::path::Path,
        url: Option<&str>,
    ) -> Result<(), LaunchServiceError> {
        if installer.is_file() {
            return Ok(());
        }
        let url = url.ok_or_else(|| {
            LaunchServiceError::ManifestUnavailable("no installer address is known".to_owned())
        })?;
        let bytes = fetch_document(&self.transport, &self.sources.candidates(url))
            .await
            .map_err(|error: FetchError| {
                LaunchServiceError::ManifestUnavailable(error.to_string())
            })?;
        if let Some(parent) = installer.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| LaunchServiceError::Loader(ForgeError::Io(error)))?;
        }
        let partial = installer.with_extension("jar.part");
        tokio::fs::write(&partial, bytes)
            .await
            .map_err(|error| LaunchServiceError::Loader(ForgeError::Io(error)))?;
        tokio::fs::rename(&partial, installer)
            .await
            .map_err(|error| LaunchServiceError::Loader(ForgeError::Io(error)))?;
        Ok(())
    }

    async fn launch_inner(
        &self,
        request: LaunchRequest,
        updates: &mpsc::UnboundedSender<LaunchUpdate>,
        cancel: &CancellationToken,
    ) -> Result<GameExit, LaunchServiceError> {
        let signal = |signal: LaunchSignal| {
            let _ = updates.send(LaunchUpdate::Signal(signal));
        };
        let launch_plan = self.prepare(&request, updates, cancel).await?;
        let settings = &request.instance.settings;
        let min_memory = settings.min_memory_mb.or(request.default_min_memory_mb);
        let max_memory = settings.max_memory_mb.or(request.default_max_memory_mb);

        let required = launch_plan.java_requirement().map(|java| java.major());
        let java = choose(&request.runtimes, required, settings.java_path.as_deref())
            .ok_or(LaunchServiceError::NoJava { required })?;
        let tuning = request
            .tuning
            .for_instance(&settings.jvm_arguments, &settings.launch);
        let instance_environment = vec![
            ("INST_ID".to_owned(), request.instance.id.clone()),
            ("INST_NAME".to_owned(), request.instance.name.clone()),
            (
                "INST_DIR".to_owned(),
                request.directories.game().to_string_lossy().into_owned(),
            ),
            (
                "INST_JAVA".to_owned(),
                java.executable().to_string_lossy().into_owned(),
            ),
            (
                "INST_MC_VERSION".to_owned(),
                request.instance.game_version.clone(),
            ),
            (
                "INST_LOADER".to_owned(),
                format!("{:?}", request.instance.loader).to_lowercase(),
            ),
        ];
        let mut options = GameOptions::new(java.executable())
            .with_memory(min_memory, max_memory)
            .with_tuning(&tuning)
            .with_environment(instance_environment.clone());
        // Versions without direct quick play still join a server the old way.
        if let Some(QuickPlay::Server(address)) = &request.quick_play
            && !launch_plan
                .game_arguments()
                .iter()
                .any(|argument| argument == "--quickPlayMultiplayer")
        {
            let (host, port) = match address.rsplit_once(':') {
                Some((host, port)) => (host, port.parse::<u16>().ok()),
                None => (address.as_str(), None),
            };
            options = options.with_legacy_server(host, port);
        }
        // Commands see the same environment the game does.
        let environment = options.environment().to_vec();

        // A command the user wants run first can stop the launch.
        if let Some(command) = tuning.pre_launch.as_deref() {
            let outcome = process::run_hook(
                command,
                request.directories.game(),
                &environment,
                None,
                cancel,
            )
            .await
            .map_err(LaunchServiceError::Process)?;
            for (stream, text) in outcome.lines.iter().cloned() {
                let _ = updates.send(LaunchUpdate::Log { stream, text });
            }
            if cancel.is_cancelled() {
                return Err(LaunchServiceError::Cancelled);
            }
            if !outcome.succeeded() {
                return Err(LaunchServiceError::Hook {
                    when: "before launch",
                    code: outcome.code,
                    stopped: outcome.stopped,
                });
            }
        }

        signal(LaunchSignal::Phase(LaunchPhase::Starting));
        let line = process::command_line(&launch_plan, &options);
        let (events, mut game_events) = mpsc::unbounded_channel();
        let game = process::run_with_environment(
            &line,
            request.directories.game(),
            options.environment(),
            events,
            crate::process::DEFAULT_SETTLE,
            cancel,
        );
        tokio::pin!(game);
        let mut running = false;
        let exit = loop {
            tokio::select! {
                result = &mut game => break result.map_err(LaunchServiceError::Process)?,
                Some(event) = game_events.recv() => {
                    forward_game_event(&event, &mut running, updates);
                }
            }
        };
        // The process ended; deliver whatever it still had queued.
        while let Ok(event) = game_events.try_recv() {
            forward_game_event(&event, &mut running, updates);
        }

        // A command after the game runs on its own; its failure is only told.
        if let Some(command) = tuning.post_exit.as_deref() {
            let never = CancellationToken::new();
            match process::run_hook(
                command,
                request.directories.game(),
                &environment,
                Some(process::HOOK_TIME_LIMIT),
                &never,
            )
            .await
            {
                Ok(outcome) => {
                    for (stream, text) in outcome.lines.iter().cloned() {
                        let _ = updates.send(LaunchUpdate::Log { stream, text });
                    }
                    if !outcome.succeeded() {
                        let _ = updates.send(LaunchUpdate::Log {
                            stream: LogStream::Stderr,
                            text: LaunchServiceError::Hook {
                                when: "after exit",
                                code: outcome.code,
                                stopped: outcome.stopped,
                            }
                            .to_string(),
                        });
                    }
                }
                Err(error) => {
                    let _ = updates.send(LaunchUpdate::Log {
                        stream: LogStream::Stderr,
                        text: error.to_string(),
                    });
                }
            }
        }
        Ok(exit)
    }

    /// The release for the instance's version: the installed copy when there
    /// is one, otherwise the manifest fetched from the catalog's address.
    async fn load_release(
        &self,
        request: &LaunchRequest,
    ) -> Result<ReleaseManifest, LaunchServiceError> {
        let record = &request.instance;
        let id = record
            .release_id()
            .ok_or(LaunchServiceError::LoaderVersionMissing)?;
        let versions = request.directories.versions();

        // An installed release is stored fully resolved.
        if let Ok(text) =
            tokio::fs::read_to_string(versions.join(&id).join(format!("{id}.json"))).await
        {
            return ReleaseManifest::decode_json(&text).map_err(LaunchServiceError::Release);
        }

        let game = &record.game_version;
        let vanilla_text = self
            .read_or_fetch(
                &versions.join(game).join(format!("{game}.json")),
                request.manifest_url.as_deref(),
                game,
            )
            .await?;
        let vanilla =
            ReleaseManifest::decode_json(&vanilla_text).map_err(LaunchServiceError::Release)?;
        if record.loader == Loader::Vanilla {
            return Ok(vanilla);
        }

        let url = request.loader_profile_url.as_deref().ok_or_else(|| {
            LaunchServiceError::ManifestUnavailable(format!(
                "{id} is not installed and no loader address is known"
            ))
        })?;
        let bytes = if uses_installer(record.loader) {
            // Forge and NeoForge: the profile is the installer's `version.json`.
            let installer = forge_install::installer_path(versions, &id);
            self.ensure_installer(&installer, Some(url)).await?;
            forge_install::read_installer(&installer)
                .map_err(LaunchServiceError::Loader)?
                .version_json
        } else {
            let sources = self.sources.candidates(url);
            fetch_document(&self.transport, &sources)
                .await
                .map_err(|error: FetchError| {
                    LaunchServiceError::ManifestUnavailable(error.to_string())
                })?
        };
        let profile = normalize_profile(&bytes, &id, game)
            .map_err(|error| LaunchServiceError::ManifestUnavailable(error.to_string()))?;
        let mut set = ReleaseSet::default();
        set.insert(vanilla);
        set.insert(ReleaseManifest::decode_json(&profile).map_err(LaunchServiceError::Release)?);
        set.resolve(&id).map_err(LaunchServiceError::Release)
    }

    /// The text of a manifest: the local copy when present, else the download.
    async fn read_or_fetch(
        &self,
        local: &std::path::Path,
        url: Option<&str>,
        what: &str,
    ) -> Result<String, LaunchServiceError> {
        if let Ok(text) = tokio::fs::read_to_string(local).await {
            return Ok(text);
        }
        let url = url.ok_or_else(|| {
            LaunchServiceError::ManifestUnavailable(format!(
                "{what} is not installed and no download address is known"
            ))
        })?;
        let sources = self.sources.candidates(url);
        let bytes =
            fetch_document(&self.transport, &sources)
                .await
                .map_err(|error: FetchError| {
                    LaunchServiceError::ManifestUnavailable(error.to_string())
                })?;
        String::from_utf8(bytes)
            .map_err(|error| LaunchServiceError::ManifestUnavailable(error.to_string()))
    }

    /// Runs the installer, translating its progress into launch signals with
    /// honest item counts.
    async fn run_installer(
        &self,
        plan: &InstallationPlan,
        updates: &mpsc::UnboundedSender<LaunchUpdate>,
        cancel: &CancellationToken,
    ) -> Result<(), LaunchServiceError> {
        let engine = TransferEngine::new(self.transport.clone(), self.concurrency)
            .map_err(|error| LaunchServiceError::Install(InstallError::from(error)))?;
        let mut transfers = engine.subscribe();
        let installer = Installer::new(engine);
        let mut stages = installer.subscribe();
        let install = installer.execute(plan, cancel.clone());
        tokio::pin!(install);

        let mut progress = ItemProgress::default();
        let result = loop {
            tokio::select! {
                result = &mut install => break result,
                event = stages.recv() => {
                    if let Ok(event) = event {
                        handle_stage(&event, plan, &mut progress, updates);
                    }
                }
                event = transfers.recv() => {
                    if let Ok(TransferEvent::Completed { .. }) = event {
                        progress.completed(updates);
                    }
                }
            }
        };
        loop {
            match stages.try_recv() {
                Ok(event) => handle_stage(&event, plan, &mut progress, updates),
                Err(broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(_) => break,
            }
        }
        match result {
            Ok(_) => Ok(()),
            Err(InstallError::Cancelled) => Err(LaunchServiceError::Cancelled),
            Err(error) => Err(LaunchServiceError::Install(error)),
        }
    }
}

/// Loaders installed by running their official installer's processors.
const fn uses_installer(loader: Loader) -> bool {
    matches!(loader, Loader::Forge | Loader::NeoForge)
}

/// Counts finished transfers inside the current download stage.
#[derive(Default)]
struct ItemProgress {
    active: bool,
    done: u64,
    total: u64,
}

impl ItemProgress {
    fn begin(&mut self, total: u64, updates: &mpsc::UnboundedSender<LaunchUpdate>) {
        self.active = total > 0;
        self.done = 0;
        self.total = total;
        if self.active {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Progress {
                done: 0,
                total,
            }));
        }
    }

    fn completed(&mut self, updates: &mpsc::UnboundedSender<LaunchUpdate>) {
        if !self.active {
            return;
        }
        self.done = (self.done + 1).min(self.total);
        let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Progress {
            done: self.done,
            total: self.total,
        }));
    }
}

fn handle_stage(
    event: &InstallEvent,
    plan: &InstallationPlan,
    progress: &mut ItemProgress,
    updates: &mpsc::UnboundedSender<LaunchUpdate>,
) {
    match event {
        InstallEvent::StageStarted(InstallStage::InitialTransfers) => {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Phase(
                LaunchPhase::Libraries,
            )));
            progress.begin(plan.initial_artifacts().len() as u64, updates);
            return;
        }
        InstallEvent::StageStarted(InstallStage::AssetTransfers) => {
            let _ = updates.send(LaunchUpdate::Signal(LaunchSignal::Phase(
                LaunchPhase::Assets,
            )));
            progress.begin(asset_object_count(plan), updates);
            return;
        }
        InstallEvent::StageStarted(_) => progress.active = false,
        _ => {}
    }
    if let Some(signal) = LaunchSignal::from_install(event) {
        let _ = updates.send(LaunchUpdate::Signal(signal));
    }
}

/// Unique asset objects listed by the already-downloaded asset catalog.
fn asset_object_count(plan: &InstallationPlan) -> u64 {
    plan.asset_catalog_artifact()
        .and_then(|artifact| std::fs::read_to_string(artifact.request().destination()).ok())
        .and_then(|json| AssetIndex::decode_json(&json).ok())
        .map_or(0, |index| index.unique_objects().len() as u64)
}

fn forward_game_event(
    event: &GameEvent,
    running: &mut bool,
    updates: &mpsc::UnboundedSender<LaunchUpdate>,
) {
    if matches!(event, GameEvent::Running) {
        *running = true;
    }
    if let GameEvent::Line { stream, text } = event {
        let _ = updates.send(LaunchUpdate::Log {
            stream: *stream,
            text: text.clone(),
        });
    }
    if let Some(signal) = event.signal(*running) {
        let _ = updates.send(LaunchUpdate::Signal(signal));
    }
}

/// Helper for callers that only need the path a runtime list should start
/// from; kept here so the app and tests share one definition of the layout.
#[must_use]
pub fn runtimes_root(launcher_root: &std::path::Path) -> PathBuf {
    crate::layout::Layout::new(launcher_root).runtimes()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::instance::{InstanceStore, NewInstance};
    use crate::java::JavaLocator;
    use crate::launch_session::{LaunchSession, LaunchStatus};
    use crate::transfer::{FileTransport, OfficialSource};
    use sha1::{Digest, Sha1};
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    struct World {
        _dir: tempfile::TempDir,
        root: PathBuf,
        request: LaunchRequest,
    }

    fn sha1_hex(bytes: &[u8]) -> String {
        Sha1::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// A launcher root with a fake Java, a local "server" holding a one-file
    /// release, and an instance pointing at it.
    fn world(java_script: &str) -> World {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("launcher");
        let server = dir.path().join("server");
        std::fs::create_dir_all(&server).unwrap();

        let client = b"pretend client jar";
        std::fs::write(server.join("client.jar"), client).unwrap();
        let client_url = url::Url::from_file_path(server.join("client.jar")).unwrap();
        let manifest = format!(
            r#"{{
                "id": "1.0",
                "mainClass": "net.example.Main",
                "minecraftArguments": "--username ${{auth_player_name}} --version ${{version_name}}",
                "javaVersion": {{"component": "java-runtime-gamma", "majorVersion": 21}},
                "downloads": {{"client": {{"url": "{client_url}", "sha1": "{}", "size": {}}}}},
                "libraries": []
            }}"#,
            sha1_hex(client),
            client.len()
        );
        std::fs::write(server.join("1.0.json"), &manifest).unwrap();
        let manifest_url = url::Url::from_file_path(server.join("1.0.json")).unwrap();

        // A fake JDK 21 whose "java" is a script.
        let jdk = root.join("runtimes/jdk-21");
        std::fs::create_dir_all(jdk.join("bin")).unwrap();
        std::fs::write(jdk.join("release"), "JAVA_VERSION=\"21.0.1\"\n").unwrap();
        let java = jdk.join("bin/java");
        std::fs::write(&java, format!("#!/bin/sh\n{java_script}\n")).unwrap();
        std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut store = InstanceStore::open(&root).unwrap();
        let instance = store
            .create(
                NewInstance {
                    name: "Test".to_owned(),
                    game_version: "1.0".to_owned(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                1,
            )
            .unwrap()
            .clone();
        let directories = store.directories(&instance);
        let runtimes = JavaLocator::new([runtimes_root(&root)]).discover();
        assert_eq!(runtimes.len(), 1);
        World {
            request: LaunchRequest {
                instance,
                directories,
                session: crate::account::OfflineProfile::new("Steve")
                    .unwrap()
                    .session(),
                manifest_url: Some(manifest_url.to_string()),
                loader_profile_url: None,
                runtimes,
                default_max_memory_mb: Some(1024),
                default_min_memory_mb: None,
                tuning: LaunchTuning::default(),
                download_concurrency: None,
                quick_play: None,
            },
            root,
            _dir: dir,
        }
    }

    fn launcher() -> Launcher<FileTransport> {
        let chain =
            SourceChain::new(
                [Arc::new(OfficialSource) as Arc<dyn crate::transfer::SourceProvider>],
            )
            .unwrap();
        Launcher::new(FileTransport, chain)
    }

    async fn run_to_end(
        launcher: &Launcher<FileTransport>,
        request: LaunchRequest,
    ) -> (Result<GameExit, LaunchServiceError>, Vec<LaunchUpdate>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let result = launcher.launch(request, tx, CancellationToken::new()).await;
        let mut updates = Vec::new();
        while let Ok(update) = rx.try_recv() {
            updates.push(update);
        }
        (result, updates)
    }

    #[tokio::test]
    async fn invalid_effective_memory_fails_before_installing_or_starting() {
        let mut world = world("echo 'Setting user: Steve'");
        world.request.instance.settings.min_memory_mb = Some(4096);
        let (result, updates) = run_to_end(&launcher(), world.request).await;
        assert!(matches!(result, Err(LaunchServiceError::InvalidMemory)));
        assert!(
            updates
                .iter()
                .any(|update| matches!(update, LaunchUpdate::Signal(LaunchSignal::Failed(_))))
        );
        assert!(!world.root.join("meta/versions").exists());
        assert!(!world.root.join("profiles/test/game").exists());
    }

    fn session_of(updates: &[LaunchUpdate]) -> LaunchSession {
        let mut session = LaunchSession::new();
        for update in updates {
            if let LaunchUpdate::Signal(signal) = update {
                session.apply(signal.clone());
            }
        }
        session
    }

    #[tokio::test]
    async fn installs_then_runs_a_release_end_to_end() {
        let world = world("echo \"Setting user: $*\"\nexit 0");
        let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
        let exit = result.unwrap();
        assert_eq!(exit.code, Some(0));

        // The client and manifest were published.
        assert!(world.root.join("meta/versions/1.0/1.0.jar").is_file());
        assert!(world.root.join("meta/versions/1.0/1.0.json").is_file());

        // The session walked the phases and ended cleanly.
        let phases: Vec<_> = updates
            .iter()
            .filter_map(|u| match u {
                LaunchUpdate::Signal(LaunchSignal::Phase(phase)) => Some(*phase),
                _ => None,
            })
            .collect();
        assert_eq!(phases.first(), Some(&LaunchPhase::Verifying));
        assert!(phases.contains(&LaunchPhase::Libraries));
        assert_eq!(phases.last(), Some(&LaunchPhase::Starting));
        assert_eq!(
            session_of(&updates).status(),
            &LaunchStatus::Exited { code: Some(0) }
        );

        // The fake Java saw the substituted arguments and printed its marker.
        let logged: Vec<_> = updates
            .iter()
            .filter_map(|u| match u {
                LaunchUpdate::Log { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            logged
                .iter()
                .any(|line| line.contains("--username Steve --version 1.0")),
            "game arguments reach the process: {logged:?}"
        );
    }

    #[tokio::test]
    async fn a_second_launch_skips_the_install() {
        let world = world("echo \"Setting user: x\"");
        let launcher = launcher();
        run_to_end(&launcher, world.request.clone())
            .await
            .0
            .unwrap();
        let (result, updates) = run_to_end(&launcher, world.request.clone()).await;
        result.unwrap();
        assert!(!updates.iter().any(|u| matches!(
            u,
            LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
        )));
    }

    #[tokio::test]
    async fn a_second_instance_of_the_same_version_installs_nothing() {
        let world = world("echo \"Setting user: x\"");
        let launcher = launcher();
        run_to_end(&launcher, world.request.clone())
            .await
            .0
            .unwrap();

        let mut store = InstanceStore::open(&world.root).unwrap();
        let other = store
            .create(
                NewInstance {
                    name: "Other".to_owned(),
                    game_version: "1.0".to_owned(),
                    loader: Loader::Vanilla,
                    loader_version: None,
                },
                2,
            )
            .unwrap()
            .clone();
        let mut request = world.request.clone();
        request.directories = store.directories(&other);
        request.instance = other;
        assert_ne!(request.directories.game(), world.request.directories.game());

        let (result, updates) = run_to_end(&launcher, request).await;
        result.unwrap();
        assert!(!updates.iter().any(|u| matches!(
            u,
            LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
        )));
        assert!(world.root.join("profiles/other/game").is_dir());
    }

    #[tokio::test]
    async fn a_damaged_client_is_repaired_on_the_next_launch() {
        let world = world("echo \"Setting user: x\"");
        let launcher = launcher();
        run_to_end(&launcher, world.request.clone())
            .await
            .0
            .unwrap();
        let jar = world.root.join("meta/versions/1.0/1.0.jar");
        std::fs::write(&jar, "corrupt").unwrap();
        run_to_end(&launcher, world.request.clone())
            .await
            .0
            .unwrap();
        assert_eq!(std::fs::read(&jar).unwrap(), b"pretend client jar");
    }

    #[tokio::test]
    async fn progress_reports_item_counts_for_the_download_phase() {
        let world = world("echo \"Setting user: x\"");
        let (_, updates) = run_to_end(&launcher(), world.request.clone()).await;
        let counts: Vec<_> = updates
            .iter()
            .filter_map(|u| match u {
                LaunchUpdate::Signal(LaunchSignal::Progress { done, total }) => {
                    Some((*done, *total))
                }
                _ => None,
            })
            .collect();
        assert!(
            counts.contains(&(0, 1)) && counts.contains(&(1, 1)),
            "{counts:?}"
        );
    }

    #[tokio::test]
    async fn a_missing_java_fails_with_a_clear_reason_and_a_terminal_signal() {
        let mut world = world("exit 0");
        world.request.runtimes.clear();
        let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
        assert!(matches!(
            result,
            Err(LaunchServiceError::NoJava { required: Some(21) })
        ));
        assert!(matches!(
            session_of(&updates).status(),
            LaunchStatus::Failed { failure: LaunchFailure::Step { message }, .. }
                if message.contains("Java 21")
        ));
    }

    #[tokio::test]
    async fn a_game_that_dies_at_once_fails_the_session() {
        let world = world("echo crash >&2\nexit 1");
        let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
        assert_eq!(result.unwrap().code, Some(1));
        assert!(matches!(
            session_of(&updates).status(),
            LaunchStatus::Failed {
                failure: LaunchFailure::ExitedEarly { code: Some(1) },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn a_forge_instance_without_a_build_is_refused_before_any_work() {
        let mut world = world("exit 0");
        world.request.instance.loader = Loader::Forge;
        world.request.instance.loader_version = None;
        let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
        assert!(matches!(
            result,
            Err(LaunchServiceError::LoaderVersionMissing)
        ));
        assert!(session_of(&updates).is_finished());
        assert!(!world.root.join("meta/versions").exists());
    }

    #[tokio::test]
    async fn an_unknown_version_without_an_address_is_reported() {
        let mut world = world("exit 0");
        world.request.manifest_url = None;
        let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
        assert!(matches!(
            result,
            Err(LaunchServiceError::ManifestUnavailable(_))
        ));
    }

    /// Turns the world's instance into a Fabric one served from the local
    /// "server": a profile that inherits the vanilla release and adds a
    /// library from a maven-style folder.
    fn make_fabric(world: &mut World) {
        let server = world._dir.path().join("server");
        let lib_dir = server.join("maven/net/example/loader-lib/1.0");
        std::fs::create_dir_all(&lib_dir).unwrap();
        std::fs::write(lib_dir.join("loader-lib-1.0.jar"), "loader library").unwrap();
        let base = url::Url::from_directory_path(server.join("maven")).unwrap();
        let profile = format!(
            r#"{{"id":"whatever-upstream-calls-it","inheritsFrom":"1.0",
                "mainClass":"net.example.LoaderMain",
                "libraries":[{{"name":"net.example:loader-lib:1.0","url":"{base}"}}]}}"#
        );
        std::fs::write(server.join("profile.json"), profile).unwrap();
        world.request.instance.loader = Loader::Fabric;
        world.request.instance.loader_version = Some("0.16.0".to_owned());
        world.request.loader_profile_url = Some(
            url::Url::from_file_path(server.join("profile.json"))
                .unwrap()
                .to_string(),
        );
    }

    #[tokio::test]
    async fn a_fabric_instance_installs_its_profile_and_starts_the_loader() {
        let mut world = world("echo \"Setting user: $*\"\nexit 0");
        make_fabric(&mut world);
        assert_eq!(
            world.request.instance.release_id().as_deref(),
            Some("fabric-loader-0.16.0-1.0")
        );
        let (result, updates) = run_to_end(&launcher(), world.request.clone()).await;
        assert_eq!(result.unwrap().code, Some(0));
        let logged: String = updates
            .iter()
            .filter_map(|u| match u {
                LaunchUpdate::Log { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(logged.contains("net.example.LoaderMain"), "{logged}");
        assert!(logged.contains("loader-lib-1.0.jar"), "{logged}");
        assert!(
            logged.contains("fabric-loader-0.16.0-1.0.jar"),
            "the client jar is on the classpath: {logged}"
        );
        assert!(
            world
                .root
                .join("meta/libraries/net/example/loader-lib/1.0/loader-lib-1.0.jar")
                .is_file()
        );
        assert!(
            world
                .root
                .join("meta/versions/fabric-loader-0.16.0-1.0/fabric-loader-0.16.0-1.0.json")
                .is_file()
        );
        assert!(
            world
                .root
                .join("meta/versions/fabric-loader-0.16.0-1.0/fabric-loader-0.16.0-1.0.jar")
                .is_file()
        );
        assert_eq!(
            session_of(&updates).status(),
            &LaunchStatus::Exited { code: Some(0) }
        );
    }

    #[tokio::test]
    async fn an_installed_loader_instance_launches_without_any_network_address() {
        let mut world = world("echo \"Setting user: $*\"");
        make_fabric(&mut world);
        let launcher = launcher();
        run_to_end(&launcher, world.request.clone())
            .await
            .0
            .unwrap();
        world.request.manifest_url = None;
        world.request.loader_profile_url = None;
        let (result, updates) = run_to_end(&launcher, world.request.clone()).await;
        result.unwrap();
        assert!(!updates.iter().any(|u| matches!(
            u,
            LaunchUpdate::Signal(LaunchSignal::Phase(LaunchPhase::Libraries))
        )));
    }

    #[tokio::test]
    async fn a_loader_instance_without_a_version_or_address_reports_why() {
        let mut world = world("exit 0");
        make_fabric(&mut world);
        world.request.instance.loader_version = None;
        let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
        assert!(matches!(
            result,
            Err(LaunchServiceError::LoaderVersionMissing)
        ));

        make_fabric(&mut world);
        world.request.loader_profile_url = None;
        let (result, _) = run_to_end(&launcher(), world.request.clone()).await;
        assert!(matches!(
            result,
            Err(LaunchServiceError::ManifestUnavailable(_))
        ));
    }
}
