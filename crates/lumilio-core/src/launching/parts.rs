use std::future::Future;
use std::sync::Arc;

use lumilio_minecraft::LaunchDirectories;
use tokio::sync::{Mutex, mpsc};

use super::{JavaRequest, LaunchRequest, LaunchSignal, LaunchUpdate, Launcher, Launching};
use crate::accounts::Accounts;
use crate::activity::{CancellationToken, Meter, Tasks};
use crate::appearance::Appearance;
use crate::context::ContextHub;
use crate::error::ServiceError;
use crate::game_settings::GameSettings;
use crate::instances::{InstanceRecord, Instances};
use crate::java::{Java, JavaRuntime};
use crate::layout::Layout;
use crate::loader;
use crate::network::Network;
use crate::plugins::PluginHost;
use crate::settings::{SettingsStore, validate_memory};
use crate::transfer::{SourceChain, Transport};
use crate::tuning::QuickPlay;

/// Launching and installing, with everything they read and write: the
/// library and its leases, the session they publish, the accounts that sign
/// the game in, the Java it runs on and the game settings written around it.
pub(crate) struct LaunchParts<'a, T> {
    pub(crate) state: &'a Launching,
    pub(crate) instances: &'a Instances,
    pub(crate) context: &'a ContextHub,
    pub(crate) layout: &'a Layout,
    pub(crate) plugins: &'a Arc<PluginHost>,
    pub(crate) network: &'a Network<T>,
    pub(crate) settings: &'a Mutex<SettingsStore>,
    pub(crate) tasks: &'a Tasks,
    pub(crate) java: &'a Java<T>,
    pub(crate) accounts: &'a Accounts<T>,
    pub(crate) appearance: &'a Appearance<T>,
    pub(crate) game_settings: &'a GameSettings,
}

impl<T: Transport + Clone> LaunchParts<'_, T> {
    /// Everything a launch or an install needs about one instance, captured
    /// once: the record, effective settings and where to find the release.
    pub(crate) async fn request(
        &self,
        id: &str,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let (record, directories) = {
            let store = self.instances.lock().await;
            let record = store
                .get(id)
                .cloned()
                .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
            let directories = store.directories(&record);
            (record, directories)
        };
        self.request_for(record, directories, quick_play, cancel)
            .await
    }

    /// The request to start (or install) `record`, which need not be what the
    /// library stores yet: a version change prepares its new combination
    /// before committing it.
    pub(crate) async fn request_for(
        &self,
        record: InstanceRecord,
        directories: LaunchDirectories,
        quick_play: Option<Option<QuickPlay>>,
        cancel: &CancellationToken,
    ) -> Result<(LaunchRequest, SourceChain), ServiceError> {
        let own_quick_play = record.settings.launch.quick_play.clone();
        let chain = self.network.chain().await?;
        let (settings, runtimes) = self.java.environment().await;
        validate_memory(
            record
                .settings
                .min_memory_mb
                .or(settings.default_min_memory_mb),
            record
                .settings
                .max_memory_mb
                .or(settings.default_max_memory_mb),
        )?;

        // Releases already on disk launch without the network.
        let on_disk = |release: &str| {
            directories
                .versions()
                .join(release)
                .join(format!("{release}.json"))
                .is_file()
        };
        let vanilla_ready = on_disk(&record.game_version);
        let all_ready =
            vanilla_ready && record.release_id().is_some_and(|release| on_disk(&release));
        let manifest_url = if all_ready {
            None
        } else {
            let fetcher = self.network.fetcher();
            let fetched = tokio::select! {
                () = cancel.cancelled() => return Err(ServiceError::Cancelled),
                fetched = crate::minecraft::fetch_catalog(&fetcher, &chain) => fetched,
            };
            match fetched {
                Ok(catalog) => catalog
                    .get(&record.game_version)
                    .map(|entry| entry.manifest_url().to_owned()),
                Err(_) if vanilla_ready => None,
                Err(error) => return Err(ServiceError::Remote(error.to_string())),
            }
        };
        let loader_profile_url = record.loader_version.as_deref().and_then(|version| {
            loader::profile_url(record.loader, &record.game_version, version).or_else(|| {
                crate::forge_install::installer_url(record.loader, &record.game_version, version)
            })
        });
        let session = self
            .accounts
            .session_for(self.appearance, &settings)
            .await?;
        let request = LaunchRequest {
            instance: record,
            directories,
            session,
            manifest_url,
            loader_profile_url,
            runtimes,
            default_max_memory_mb: settings.default_max_memory_mb,
            default_min_memory_mb: settings.default_min_memory_mb,
            tuning: settings.launch.clone(),
            // `None` asks for the instance's own choice; `Some(None)` for the menu.
            quick_play: quick_play.unwrap_or(own_quick_play),
        };
        Ok((request, chain))
    }

    /// A launcher that downloads through `chain` on the shared connections
    /// and, when Java may be installed automatically, asks for it on the
    /// returned channel.
    pub(crate) async fn launcher(
        &self,
        chain: SourceChain,
    ) -> (Launcher<T>, mpsc::UnboundedReceiver<JavaRequest>) {
        let launcher = Launcher::new(self.network.transport().clone(), chain)
            .with_budget(self.network.connections().await)
            .keeping_partials(self.network.partials());
        let (sender, receiver) = mpsc::unbounded_channel();
        let launcher = if self.settings.lock().await.get().auto_install_java {
            launcher.with_java_installer(sender)
        } else {
            launcher
        };
        (launcher, receiver)
    }

    /// Runs `work` (a launch or an install) while installing the Java it asks
    /// for, each as its own Activity task cancelled with it.
    pub(crate) async fn serving_java<R>(
        &self,
        mut requests: mpsc::UnboundedReceiver<JavaRequest>,
        cancel: &CancellationToken,
        work: impl Future<Output = R>,
    ) -> R {
        tokio::pin!(work);
        loop {
            tokio::select! {
                result = &mut work => return result,
                Some(request) = requests.recv() => {
                    let installed: Result<JavaRuntime, String> = self
                        .java
                        .install_java(request.major, request.build, cancel.clone())
                        .await
                        .map_err(|error| error.to_string());
                    let _ = request.reply.send(installed);
                }
            }
        }
    }

    /// Installs `request` through a new launcher that may download the Java a
    /// Forge or NeoForge installer runs on, showing its files as task `task`.
    pub(crate) async fn install_serving_java(
        &self,
        task: u64,
        chain: SourceChain,
        request: LaunchRequest,
        updates: mpsc::UnboundedSender<LaunchUpdate>,
        cancel: CancellationToken,
    ) -> Result<(), ServiceError> {
        let (watcher, events) = tokio::sync::broadcast::channel(1024);
        let (launcher, java_requests) = self.launcher(chain).await;
        let launcher = launcher.watched_by(watcher);
        let watch = cancel.clone();
        let install = self.serving_java(
            java_requests,
            &watch,
            install_tracked(self.tasks, task, &launcher, request, updates, cancel),
        );
        self.tasks
            .watching(task, events, Meter::Files, install)
            .await
    }
}

/// Installs through `launcher`, passing its updates on while showing their
/// file counts as the task's progress.
async fn install_tracked<T: Transport + Clone>(
    tasks: &Tasks,
    task: u64,
    launcher: &Launcher<T>,
    request: LaunchRequest,
    updates: mpsc::UnboundedSender<LaunchUpdate>,
    cancel: CancellationToken,
) -> Result<(), ServiceError> {
    let (inner, mut seen) = mpsc::unbounded_channel();
    let forward = async {
        while let Some(update) = seen.recv().await {
            if let LaunchUpdate::Signal(LaunchSignal::Progress { done, total }) = &update {
                tasks.progress(task, *done, *total);
            }
            let _ = updates.send(update);
        }
    };
    let (installed, ()) = tokio::join!(launcher.install(request, inner, cancel), forward);
    installed.map_err(ServiceError::from)
}
