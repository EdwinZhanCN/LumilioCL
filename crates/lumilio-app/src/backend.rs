//! The asynchronous side of the application: a tokio runtime and the one
//! [`LauncherService`] the interface talks to (plan 0007).
//!
//! GPUI's executor does not drive network or process work, so every piece of
//! domain work is spawned here and its result is handed back to the interface
//! thread.

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use lumilio_core::{DefaultTransport, LauncherService};
use tokio::runtime::{Builder, Runtime};

pub type Service = LauncherService<DefaultTransport>;

pub struct Backend {
    runtime: Runtime,
    pub service: Arc<Service>,
}

impl Backend {
    pub fn open(root: PathBuf) -> Result<Self, String> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("lumilio-worker")
            .enable_all()
            .build()
            .map_err(|error| format!("could not start the worker runtime: {error}"))?;
        let transport = DefaultTransport::new().map_err(|error| error.to_string())?;
        let service = {
            // The transport builds its HTTP client lazily on this runtime.
            let _guard = runtime.enter();
            LauncherService::open(
                root,
                transport,
                vec![
                    Arc::new(lumilio_plugin_crash_analyzer::CrashAnalyzer),
                    Arc::new(lumilio_plugin_litematica::Litematica),
                    Arc::new(lumilio_plugin_modrinth::Modrinth),
                ],
            )
            .map_err(|error| error.to_string())?
        };
        Ok(Self {
            runtime,
            service: Arc::new(service),
        })
    }

    /// Runs `work` on the worker runtime. The handle can be awaited from any
    /// executor, including GPUI's.
    pub fn spawn<F>(&self, work: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime.spawn(work)
    }
}

/// Where the launcher keeps its data. `LUMILIO_HOME` overrides the platform
/// convention (development, portable installs).
pub fn data_root(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    if let Some(custom) = env("LUMILIO_HOME").filter(|path| !path.is_empty()) {
        return Some(PathBuf::from(custom));
    }
    if cfg!(target_os = "macos") {
        let home = env("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/LumilioCL"))
    } else if cfg!(target_os = "windows") {
        Some(PathBuf::from(env("APPDATA")?).join("LumilioCL"))
    } else {
        match env("XDG_DATA_HOME").filter(|path| !path.is_empty()) {
            Some(data) => Some(PathBuf::from(data).join("lumilio")),
            None => Some(PathBuf::from(env("HOME")?).join(".local/share/lumilio")),
        }
    }
}

#[cfg(test)]
mod tests;
