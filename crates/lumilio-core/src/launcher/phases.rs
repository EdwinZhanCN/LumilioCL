use super::progress::{ItemProgress, forward_game_event, handle_stage, uses_installer};
use super::types::{LaunchRequest, LaunchServiceError, LaunchUpdate};
use super::{LAUNCHER_NAME, Launcher, QUICK_PLAY_LOG};
use crate::activity::CancellationToken;
use crate::environment::HostProfile;
use crate::fetch::{FetchError, fetch_document};
use crate::forge_install::ForgeError;
use crate::install::{InstallError, InstallationPlan, Installer};
use crate::instance::Loader;
use crate::java::choose;
use crate::launch::{LaunchContext, LaunchPlan};
use crate::launch_session::{LaunchPhase, LaunchSignal};
use crate::loader::{LAUNCHABLE_LOADERS, normalize_profile};
use crate::process::{GameExit, GameOptions, LogStream};
use crate::release::{ReleaseManifest, ReleaseSet};
use crate::repair::InstallationVerifier;
use crate::transfer::{TransferEngine, TransferEvent, TransferRequest, Transport};
use crate::tuning::QuickPlay;
use crate::{forge_install, process};
use tokio::sync::{broadcast, mpsc};

impl<T> Launcher<T>
where
    T: Transport + Clone,
{
    /// Verifies the request, resolves the release and installs whatever is
    /// missing or damaged. Returns the plan the game would start with.
    pub(super) async fn prepare(
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
    pub(super) async fn finish_loader(
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
    pub(super) async fn ensure_installer(
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

    pub(super) async fn launch_inner(
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
            .with_agent_arguments(request.session.jvm_arguments())
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
    pub(super) async fn load_release(
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
    pub(super) async fn read_or_fetch(
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
    pub(super) async fn run_installer(
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
