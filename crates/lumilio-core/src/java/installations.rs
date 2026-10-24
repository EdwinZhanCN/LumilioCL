use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::activity::{CancellationToken, RetryAction, TaskAction, TaskCategory, TaskLabel, Tasks};
use crate::blocking::blocking;
use crate::context::ContextHub;
use crate::error::{ServiceError, read_unless_cancelled};
use crate::instances::Instances;
use crate::java::JavaLocator;
use crate::java_runtime::{JavaBuild, JavaSource};
use crate::layout::Layout;
use crate::network::Network;
use crate::settings::SettingsStore;
use crate::transfer::{TransferRequest, Transport};
use lumilio_minecraft::HostProfile;

/// The Java a download without a wanted major gets.
const RECOMMENDED_MAJOR: u32 = 21;

/// What an instance's next launch does about Java, for its settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceJava {
    /// Every Java found and not turned off, newest major first.
    pub runtimes: Vec<crate::java::JavaRuntime>,
    /// The one the next launch uses, with the rules it breaks.
    pub choice: Option<crate::java::JavaChoice>,
    /// What the automatic choice would be, whatever the settings say.
    pub automatic: Option<crate::java::JavaChoice>,
    /// When none fits: the major to download (`None` for the recommended one).
    pub needed: Option<u32>,
    /// Which build that download must be.
    pub build: JavaBuild,
    /// Whether the launch downloads it by itself.
    pub auto_install: bool,
}

/// The Java runtimes the launcher finds, installs and removes, and the
/// settings that choose among them.
pub(crate) struct Java<T> {
    settings: Arc<Mutex<SettingsStore>>,
    network: Network<T>,
    layout: Layout,
    tasks: Arc<Tasks>,
    context: ContextHub,
    instances: Arc<Instances>,
    /// Replaces the conventional Java search locations (tests, portable setups).
    runtime_roots: Option<Vec<PathBuf>>,
}

impl<T: Transport + Clone> Java<T> {
    pub(crate) fn new(
        settings: Arc<Mutex<SettingsStore>>,
        network: Network<T>,
        layout: Layout,
        tasks: Arc<Tasks>,
        context: ContextHub,
        instances: Arc<Instances>,
    ) -> Self {
        Self {
            settings,
            network,
            layout,
            tasks,
            context,
            instances,
            runtime_roots: None,
        }
    }

    /// Looks for Java only in these folders instead of the conventional
    /// system locations.
    pub(crate) fn set_runtime_roots(&mut self, roots: Vec<PathBuf>) {
        self.runtime_roots = Some(roots);
    }

    /// Downloads a Java for this system into the launcher's own `runtimes`
    /// folder (ADR 0014) and returns it, from the download source in the
    /// settings when it offers `required`, else from Mojang. `required` is the
    /// Java major a game needs; without it, the recommended one. An Intel
    /// `build` always comes from Mojang.
    pub(crate) async fn install_java(
        &self,
        required: Option<u32>,
        build: JavaBuild,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let source = match build {
            JavaBuild::Any => {
                let preferred = self.settings.lock().await.get().java_source;
                crate::java_runtime::source_for(preferred, &HostProfile::current(), required)
            }
            JavaBuild::Intel => JavaSource::Mojang,
        };
        self.install_java_from(source, required, build, cancel)
            .await
    }

    /// Downloads the Java instance `id` would need, as its settings page and
    /// problems offer: `required`, in the build its game needs.
    pub(crate) async fn install_instance_java(
        &self,
        id: &str,
        required: Option<u32>,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let build = self.instance_java(id).await?.build;
        self.install_java(required, build, cancel).await
    }

    /// Downloads a Java from `source`. Nothing is touched until every file
    /// has arrived and checked out; an installed runtime is returned as it
    /// is. It is an Activity task and can be cancelled. `build` applies to
    /// Mojang's runtimes.
    pub(crate) async fn install_java_from(
        &self,
        source: JavaSource,
        required: Option<u32>,
        build: JavaBuild,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let mut subject = match (source, required) {
            (JavaSource::Mojang, None) => String::new(),
            (JavaSource::Mojang, Some(major)) => major.to_string(),
            (other, None) => other.name().to_owned(),
            (other, Some(major)) => format!("{major} · {}", other.name()),
        };
        if build == JavaBuild::Intel {
            subject = [subject.as_str(), "Intel"]
                .into_iter()
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
        }
        let task = self
            .tasks
            .begin(
                TaskCategory::Install,
                TaskLabel::Typed {
                    action: TaskAction::InstallJava,
                    subject,
                },
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallJava {
                major: required,
                source,
                build,
            });
        let result = match source {
            JavaSource::Mojang => {
                self.install_java_inner(
                    task.id,
                    required,
                    build,
                    Self::java_manifest_allowed(),
                    cancel,
                )
                .await
            }
            JavaSource::OracleGraalVm => {
                self.install_graalvm(task.id, required, Self::graalvm_allowed(), cancel)
                    .await
            }
        };
        self.tasks.end(task, &result);
        if result.is_ok() {
            self.context.bump(crate::context::Change::Java);
        }
        result
    }

    /// The Java an instance would start with now, and the ones it could use.
    /// The requirement comes from its installed release; before the first
    /// install only the game version's rules apply.
    pub(crate) async fn instance_java(&self, id: &str) -> Result<InstanceJava, ServiceError> {
        let (record, directories) = {
            let store = self.instances.lock().await;
            let record = store
                .get(id)
                .cloned()
                .ok_or_else(|| ServiceError::NoSuchInstance(id.to_owned()))?;
            let directories = store.directories(&record);
            (record, directories)
        };
        let (settings, runtimes) = self.environment().await;
        let auto_install = settings.auto_install_java;
        crate::blocking::blocking_value(move || {
            let needs = record
                .release_id()
                .and_then(|release| {
                    std::fs::read_to_string(
                        directories
                            .versions()
                            .join(&release)
                            .join(format!("{release}.json")),
                    )
                    .ok()
                })
                .and_then(|text| lumilio_minecraft::ReleaseManifest::decode_json(&text).ok())
                .map_or_else(
                    || crate::java::JavaNeeds::new(&record.game_version, record.loader, None),
                    |release| crate::java::JavaNeeds::of_release(&record, &release),
                );
            let host = HostProfile::current();
            let choice = crate::java::choose(
                &runtimes,
                &needs,
                record.settings.java_path.as_deref(),
                &host,
            );
            let needed = crate::java::install_major(&needs, &host);
            let build = crate::java::install_build(&needs, &host);
            let automatic = crate::java::choose(&runtimes, &needs, None, &host);
            InstanceJava {
                runtimes,
                choice,
                automatic,
                needed,
                build,
                auto_install,
            }
        })
        .await
    }

    /// Deletes a Java the launcher installed: the folder below `runtimes`
    /// that `home` belongs to. Any other Java is refused; it can only be
    /// turned off.
    pub(crate) async fn remove_java(&self, home: &Path) -> Result<(), ServiceError> {
        let runtimes = self.layout.runtimes();
        let folder = home
            .strip_prefix(&runtimes)
            .ok()
            .and_then(|relative| relative.components().next())
            .and_then(|first| match first {
                std::path::Component::Normal(name) => Some(runtimes.join(name)),
                _ => None,
            })
            .filter(|folder| Self::runtime_in(folder).is_some())
            .ok_or_else(|| ServiceError::NoJavaAt(home.to_owned()))?;
        blocking(move || std::fs::remove_dir_all(&folder)).await?;
        self.context.bump(crate::context::Change::Java);
        let mut settings = self.settings.lock().await;
        let mut disabled = settings.get().disabled_java.clone();
        if disabled.iter().any(|saved| saved == home) {
            disabled.retain(|saved| saved != home);
            settings.set_disabled_java(disabled)?;
        }
        Ok(())
    }

    /// The Java majors each source offers this system, for choosing one to
    /// download. Mojang's come from its index; a source that cannot be read
    /// offers nothing.
    pub(crate) async fn java_offers(&self) -> Vec<(JavaSource, Vec<u32>)> {
        let host = HostProfile::current();
        let mojang = match self.network.chain().await {
            Ok(chain) => crate::fetch::fetch_decoded(
                &self.network.fetcher(),
                &chain.candidates(crate::java_runtime::INDEX_URL),
                |bytes| {
                    let index = String::from_utf8(bytes).map_err(|error| error.to_string())?;
                    crate::java_runtime::offered_majors(&index, &host)
                        .map_err(|error| error.to_string())
                },
            )
            .await
            .unwrap_or_default(),
            Err(_) => Vec::new(),
        };
        vec![
            (JavaSource::Mojang, mojang),
            (
                JavaSource::OracleGraalVm,
                crate::java_runtime::graalvm::offered_majors(&host),
            ),
        ]
    }

    /// Which addresses a runtime file may come from; tests widen it.
    fn java_manifest_allowed() -> fn(&str) -> bool {
        #[cfg(test)]
        {
            |_| true
        }
        #[cfg(not(test))]
        {
            crate::java_runtime::is_trusted_source
        }
    }

    /// Which addresses a GraalVM archive may come from; tests widen it.
    fn graalvm_allowed() -> fn(&str) -> bool {
        #[cfg(test)]
        {
            |_| true
        }
        #[cfg(not(test))]
        {
            crate::java_runtime::graalvm::is_trusted_source
        }
    }

    /// Oracle GraalVM: one archive, checked against Oracle's SHA-256 and
    /// unpacked into `runtimes/graalvm-<major>`.
    async fn install_graalvm(
        &self,
        task: u64,
        required: Option<u32>,
        allow: fn(&str) -> bool,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        use crate::java_runtime::{archive, graalvm};
        let fail = |why: String| ServiceError::Install(why);
        let major = required.unwrap_or(RECOMMENDED_MAJOR);
        let wanted = graalvm::archive(&HostProfile::current(), major)
            .map_err(|error| fail(error.to_string()))?;
        for url in [&wanted.url, &wanted.checksum_url] {
            if !allow(url) {
                return Err(fail(
                    crate::java_runtime::JavaRuntimeError::Unsafe(url.clone()).to_string(),
                ));
            }
        }
        let name = JavaSource::OracleGraalVm
            .folder_name(major)
            .expect("GraalVM names its folders");
        let runtimes = self.layout.runtimes();
        let folder = runtimes.join(&name);
        if let Some(found) = Self::runtime_in(&folder) {
            return Ok(found);
        }
        let chain = read_unless_cancelled(&cancel, self.network.chain()).await?;
        let checksum = read_unless_cancelled(&cancel, async {
            crate::fetch::fetch_decoded(
                &self.network.fetcher(),
                &chain.candidates(&wanted.checksum_url),
                |bytes| {
                    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
                    graalvm::parse_checksum(&text).map_err(|error| error.to_string())
                },
            )
            .await
            .map_err(|error| fail(error.to_string()))
        })
        .await?;

        let download = runtimes.join(format!(".{name}.{}", wanted.kind.extension()));
        let request = TransferRequest::new(
            format!("java:{name}"),
            chain.candidates(&wanted.url),
            &download,
        )
        .and_then(|request| request.expect_sha256(&checksum))
        .map_err(|error| fail(error.to_string()))?;
        let engine = self.network.transfer_engine().await;
        let report = self
            .tasks
            .metered(
                task,
                engine.subscribe(),
                engine.transfer_batch(vec![request], cancel.clone()),
            )
            .await;
        if report.failed() > 0 {
            let _ = tokio::fs::remove_file(&download).await;
            return Err(if cancel.is_cancelled() {
                ServiceError::Cancelled
            } else {
                fail(format!("Oracle GraalVM {major} could not be fetched"))
            });
        }
        if cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }

        let staging = runtimes.join(format!(".{name}.installing"));
        let kind = wanted.kind;
        let unpacked = {
            let (staging, folder, download) = (staging.clone(), folder.clone(), download.clone());
            blocking(move || {
                let _ = std::fs::remove_dir_all(&staging);
                archive::unpack(&download, kind, &staging)?;
                let _ = std::fs::remove_dir_all(&folder);
                std::fs::rename(&staging, &folder)?;
                let _ = std::fs::remove_file(&download);
                Ok(())
            })
            .await
        };
        if let Err(error) = unpacked {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            let _ = tokio::fs::remove_file(&download).await;
            return Err(fail(error.to_string()));
        }
        Self::runtime_in(&folder)
            .ok_or_else(|| fail("the downloaded Java does not start".to_owned()))
    }

    async fn install_java_inner(
        &self,
        task: u64,
        required: Option<u32>,
        build: JavaBuild,
        allow: fn(&str) -> bool,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        use crate::java_runtime as runtime;
        let fail = |why: String| ServiceError::Install(why);
        let host = lumilio_minecraft::HostProfile::current();
        if runtime::platform_keys(&host).is_empty() {
            return Err(fail(
                runtime::JavaRuntimeError::UnsupportedPlatform.to_string(),
            ));
        }
        let chain = read_unless_cancelled(&cancel, self.network.chain()).await?;
        let choice = read_unless_cancelled(&cancel, async {
            crate::fetch::fetch_decoded(
                &self.network.fetcher(),
                &chain.candidates(runtime::INDEX_URL),
                |bytes| {
                    let index =
                        String::from_utf8(bytes).map_err(|error| fail(error.to_string()))?;
                    runtime::choose_for_host(&index, &host, required, build)
                        .map_err(|error| fail(error.to_string()))
                },
            )
            .await
            .map_err(|error| fail(error.to_string()))
        })
        .await?;

        let runtimes = self.layout.runtimes();
        let name = choice.folder(&host);
        let folder = runtimes.join(&name);
        if let Some(found) = Self::runtime_in(&folder) {
            return Ok(found);
        }
        let entries = read_unless_cancelled(&cancel, async {
            crate::fetch::fetch_decoded(
                &self.network.fetcher(),
                &chain.candidates(&choice.manifest.url),
                |bytes| {
                    let manifest =
                        String::from_utf8(bytes).map_err(|error| fail(error.to_string()))?;
                    runtime::parse_manifest(&manifest, allow)
                        .map_err(|error| fail(error.to_string()))
                },
            )
            .await
            .map_err(|error| fail(error.to_string()))
        })
        .await?;

        let staging = runtimes.join(format!(".{name}.installing"));
        let prepared = {
            let staging = staging.clone();
            let layout: Vec<(String, bool)> = entries
                .iter()
                .filter_map(|(path, entry)| match entry {
                    runtime::Entry::Directory => Some((path.clone(), true)),
                    _ => None,
                })
                .collect();
            blocking(move || {
                let _ = std::fs::remove_dir_all(&staging);
                std::fs::create_dir_all(&staging)?;
                for (path, _) in layout {
                    std::fs::create_dir_all(runtime::place(&staging, &path))?;
                }
                Ok(())
            })
            .await
        };
        prepared?;

        let mut requests = Vec::new();
        for (path, entry) in &entries {
            if let runtime::Entry::File { download, .. } = entry {
                let mut request = TransferRequest::new(
                    format!("java:{name}:{path}"),
                    chain.candidates(&download.url),
                    runtime::place(&staging, path),
                )
                .map_err(|error| fail(error.to_string()))?;
                if download.size > 0 {
                    request = request.expect_size(download.size);
                }
                request = request
                    .expect_sha1(&download.sha1)
                    .map_err(|error| fail(error.to_string()))?;
                requests.push(request);
            }
        }
        let engine = self.network.transfer_engine().await;
        let report = self
            .tasks
            .metered(
                task,
                engine.subscribe(),
                engine.transfer_batch(requests, cancel.clone()),
            )
            .await;
        if report.failed() > 0 {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(if cancel.is_cancelled() {
                ServiceError::Cancelled
            } else {
                fail(format!(
                    "{} Java files could not be fetched",
                    report.failed()
                ))
            });
        }

        let publish = {
            let (staging, folder) = (staging.clone(), folder.clone());
            blocking(move || {
                for (path, entry) in &entries {
                    let target = runtime::place(&staging, path);
                    match entry {
                        runtime::Entry::File {
                            executable: true, ..
                        } => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                std::fs::set_permissions(
                                    &target,
                                    std::fs::Permissions::from_mode(0o755),
                                )?;
                            }
                        }
                        runtime::Entry::Link { target: to } => {
                            #[cfg(unix)]
                            {
                                if let Some(parent) = target.parent() {
                                    std::fs::create_dir_all(parent)?;
                                }
                                std::os::unix::fs::symlink(to, &target)?;
                            }
                            #[cfg(not(unix))]
                            let _ = to;
                        }
                        _ => {}
                    }
                }
                let _ = std::fs::remove_dir_all(&folder);
                std::fs::rename(&staging, &folder)
            })
            .await
        };
        if let Err(error) = publish {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error.into());
        }
        Self::runtime_in(&folder)
            .ok_or_else(|| fail("the downloaded Java does not start".to_owned()))
    }

    /// The runtime installed in `folder` (directly, or as a macOS bundle).
    fn runtime_in(folder: &Path) -> Option<crate::java::JavaRuntime> {
        [folder.to_path_buf(), folder.join("Contents/Home")]
            .iter()
            .find_map(|home| crate::java::JavaRuntime::inspect(home))
    }

    pub(crate) async fn environment(
        &self,
    ) -> (
        crate::settings::LauncherSettings,
        Vec<crate::java::JavaRuntime>,
    ) {
        let settings = self.settings.lock().await.get().clone();
        let found = self.discover_java(&settings).await;
        // A Java the user turned off is never picked automatically, here or
        // at launch; a game whose settings name it still uses it.
        let runtimes = found
            .into_iter()
            .filter(|runtime| {
                !settings
                    .disabled_java
                    .iter()
                    .any(|home| home == runtime.home())
            })
            .collect();
        (settings, runtimes)
    }

    pub(crate) async fn discover_java(
        &self,
        settings: &crate::settings::LauncherSettings,
    ) -> Vec<crate::java::JavaRuntime> {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        let locator = match &self.runtime_roots {
            Some(roots) => JavaLocator::new(roots.clone()),
            None => {
                JavaLocator::standard(&HostProfile::current(), home.as_deref(), self.layout.root())
            }
        }
        .with_roots(settings.extra_java_roots.clone());
        tokio::task::spawn_blocking(move || locator.discover())
            .await
            .unwrap_or_default()
    }

    /// Every Java found on this machine, with whether the user turned it off.
    pub(crate) async fn java_installations(&self) -> Vec<(crate::java::JavaRuntime, bool)> {
        let settings = self.settings.lock().await.get().clone();
        self.discover_java(&settings)
            .await
            .into_iter()
            .map(|runtime| {
                let disabled = settings
                    .disabled_java
                    .iter()
                    .any(|home| home == runtime.home());
                (runtime, disabled)
            })
            .collect()
    }

    pub(crate) async fn set_java_source(
        &self,
        source: crate::java_runtime::JavaSource,
    ) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_java_source(source)?)
    }

    pub(crate) async fn set_auto_install_java(&self, enabled: bool) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_auto_install_java(enabled)?)
    }

    /// Turns a Java installation off (or back on) by its home folder.
    pub(crate) async fn set_java_disabled(
        &self,
        home: &Path,
        disabled: bool,
    ) -> Result<(), ServiceError> {
        let mut settings = self.settings.lock().await;
        let mut homes = settings.get().disabled_java.clone();
        homes.retain(|saved| saved != home);
        if disabled {
            homes.push(home.to_owned());
        }
        Ok(settings.set_disabled_java(homes)?)
    }

    /// Adds the Java the person chose: its `java` executable, its home
    /// folder, or a folder of installations. The folder joins the extra
    /// search folders so it is found again later.
    pub(crate) async fn add_java(
        &self,
        chosen: &Path,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let mut candidates = vec![chosen.to_owned()];
        if chosen.is_file()
            && let Some(home) = chosen.parent().and_then(Path::parent)
        {
            candidates.insert(0, home.to_owned());
        }
        for candidate in candidates {
            let probe = candidate.clone();
            let found = tokio::task::spawn_blocking(move || JavaLocator::new([probe]).discover())
                .await
                .unwrap_or_default();
            let Some(runtime) = found.into_iter().next() else {
                continue;
            };
            let mut settings = self.settings.lock().await;
            let mut roots = settings.get().extra_java_roots.clone();
            if !roots.contains(&candidate) {
                roots.push(candidate);
                settings.set_java_roots(roots)?;
            }
            return Ok(runtime);
        }
        Err(ServiceError::NoJavaAt(chosen.to_owned()))
    }

    pub(crate) async fn set_java_roots(&self, roots: Vec<PathBuf>) -> Result<(), ServiceError> {
        Ok(self.settings.lock().await.set_java_roots(roots)?)
    }
}
