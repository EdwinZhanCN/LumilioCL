//! What the features' tests share: a real launcher in a temporary folder on
//! its own runtime, and a way to wait for its answers.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::VisualTestContext;
use lumilio_core::{DefaultTransport, LauncherService};
use lumilio_plugin_api::Plugin;

use crate::runtime::Worker;

/// A launcher in a temporary folder, and its runtime. Keep it until the end
/// of the test, after the views that ask it.
pub(crate) struct Launcher {
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) worker: Worker,
    _dir: tempfile::TempDir,
}

/// A launcher with these plugins.
pub(crate) fn launcher(plugins: Vec<Arc<dyn Plugin>>) -> Launcher {
    let dir = tempfile::tempdir().unwrap();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let service = {
        let _guard = runtime.enter();
        LauncherService::open(
            dir.path().join("root"),
            DefaultTransport::new().unwrap(),
            plugins,
        )
        .unwrap()
    };
    let worker = Worker::new(Arc::new(service), runtime.handle().clone());
    Launcher {
        runtime,
        worker,
        _dir: dir,
    }
}

/// Lets the runtime and GPUI work until `done` holds, for at most ten
/// seconds. The test must allow parking: the runtime's threads wake GPUI.
pub(crate) fn until(
    cx: &mut VisualTestContext,
    what: &str,
    done: impl Fn(&mut VisualTestContext) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // GPUI's test clock moves only when told: polls wait on it.
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The text of every toast posted so far.
pub(crate) fn toasts(cx: &mut VisualTestContext) -> Vec<String> {
    cx.update(|_, cx| {
        crate::toast::posted(cx)
            .iter()
            .map(|toast| toast.text.clone())
            .collect()
    })
}
