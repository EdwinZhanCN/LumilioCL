use super::LauncherService;
use super::error::{ServiceError, read_unless_cancelled};
use super::support::blocking;
use crate::activity::CancellationToken;
use crate::activity_log::{RetryAction, TaskCategory};
use crate::environment::HostProfile;
use crate::java::JavaLocator;
use crate::transfer::{TransferEngine, TransferRequest, Transport};
use std::path::{Path, PathBuf};

impl<T: Transport + Clone> LauncherService<T> {
    /// Downloads Mojang's Java runtime for this system into the launcher's own
    /// `runtimes` folder (ADR 0014) and returns it. `required` is the Java
    /// major a game needs; without it, the recommended one. Nothing is touched
    /// until every file has arrived and checked out; an installed runtime is
    /// returned as it is. It is an Activity task and can be cancelled.
    pub async fn install_java(
        &self,
        required: Option<u32>,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        let task = self
            .begin(
                TaskCategory::Install,
                match required {
                    Some(major) => format!("安装 Java {major}"),
                    None => "安装 Java".to_owned(),
                },
                None,
                Some(&cancel),
            )
            .retrying(RetryAction::InstallJava { major: required });
        let result = self
            .install_java_inner(task.id, required, Self::java_manifest_allowed(), cancel)
            .await;
        self.end(task, &result);
        result
    }

    /// Which addresses a runtime file may come from; tests widen it.
    pub(super) fn java_manifest_allowed() -> fn(&str) -> bool {
        #[cfg(test)]
        {
            |_| true
        }
        #[cfg(not(test))]
        {
            crate::java_runtime::is_trusted_source
        }
    }

    pub(super) async fn install_java_inner(
        &self,
        task: u64,
        required: Option<u32>,
        allow: fn(&str) -> bool,
        cancel: CancellationToken,
    ) -> Result<crate::java::JavaRuntime, ServiceError> {
        use crate::java_runtime as runtime;
        let fail = |why: String| ServiceError::Install(why);
        let host = crate::environment::HostProfile::current();
        let platform = runtime::platform_key(&host)
            .ok_or_else(|| fail(runtime::JavaRuntimeError::UnsupportedPlatform.to_string()))?;
        let chain = read_unless_cancelled(&cancel, self.chain()).await?;
        let fetch = |address: String| {
            let candidates = chain.candidates(&address);
            let transport = self.transport.clone();
            async move {
                crate::fetch::fetch_document(&transport, &candidates)
                    .await
                    .map_err(|error| ServiceError::Remote(error.to_string()))
            }
        };
        let index = read_unless_cancelled(&cancel, fetch(runtime::INDEX_URL.to_owned())).await?;
        let index = String::from_utf8_lossy(&index).into_owned();
        let choice = runtime::choose_component(&index, platform, required)
            .map_err(|error| fail(error.to_string()))?;

        let runtimes = self.layout.runtimes();
        let folder = runtimes.join(&choice.component);
        if let Some(found) = Self::runtime_in(&folder) {
            return Ok(found);
        }
        let manifest = read_unless_cancelled(&cancel, fetch(choice.manifest.url.clone())).await?;
        let entries = runtime::parse_manifest(&String::from_utf8_lossy(&manifest), allow)
            .map_err(|error| fail(error.to_string()))?;

        let staging = runtimes.join(format!(".{}.installing", choice.component));
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
                    format!("java:{}:{path}", choice.component),
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
        let engine = TransferEngine::new(self.transport.clone(), self.transfer_concurrency().await)
            .map_err(|error| fail(error.to_string()))?;
        let report = self
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
    pub(super) fn runtime_in(folder: &Path) -> Option<crate::java::JavaRuntime> {
        [folder.to_path_buf(), folder.join("Contents/Home")]
            .iter()
            .find_map(|home| crate::java::JavaRuntime::inspect(home))
    }

    pub(super) async fn environment(
        &self,
    ) -> (
        crate::settings::LauncherSettings,
        Vec<crate::java::JavaRuntime>,
    ) {
        let settings = self.settings.lock().await.get().clone();
        let found = self.discover_java(&settings).await;
        // A Java the user turned off is never chosen, here or at launch.
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

    pub(super) async fn discover_java(
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
    pub async fn java_installations(&self) -> Vec<(crate::java::JavaRuntime, bool)> {
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
}
