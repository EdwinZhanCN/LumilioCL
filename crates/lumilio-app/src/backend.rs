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
            LauncherService::open(root, transport).map_err(|error| error.to_string())?
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
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn an_explicit_home_wins() {
        let root = data_root(env(&[("LUMILIO_HOME", "/data/lumilio"), ("HOME", "/h")]));
        assert_eq!(root, Some(PathBuf::from("/data/lumilio")));
    }

    #[test]
    fn an_empty_override_is_ignored() {
        let root = data_root(env(&[
            ("LUMILIO_HOME", ""),
            ("HOME", "/h"),
            ("APPDATA", "/a"),
        ]));
        assert!(root.is_some_and(|path| path.ends_with("LumilioCL") || path.ends_with("lumilio")));
    }

    #[test]
    fn nothing_to_go_on_means_no_root() {
        assert_eq!(data_root(env(&[])), None);
    }

    #[test]
    fn the_backend_opens_and_runs_work_off_the_calling_thread() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend::open(dir.path().join("root")).unwrap();
        let service = backend.service.clone();
        let handle = backend.spawn(async move { service.library().await.instances.len() });
        let count = futures_block(handle);
        assert_eq!(count, 0);
    }

    /// Polls a join handle without a runtime of our own, the way GPUI awaits it.
    fn futures_block<T>(handle: tokio::task::JoinHandle<T>) -> T {
        use std::sync::mpsc;
        use std::task::{Context, Poll, Wake, Waker};
        struct Ping(mpsc::Sender<()>);
        impl Wake for Ping {
            fn wake(self: Arc<Self>) {
                let _ = self.0.send(());
            }
        }
        let (tx, rx) = mpsc::channel();
        let waker = Waker::from(Arc::new(Ping(tx)));
        let mut cx = Context::from_waker(&waker);
        let mut handle = Box::pin(handle);
        loop {
            if let Poll::Ready(value) = handle.as_mut().poll(&mut cx) {
                return value.unwrap();
            }
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        }
    }
}
