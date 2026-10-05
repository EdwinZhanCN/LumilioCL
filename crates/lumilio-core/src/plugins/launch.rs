use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use lumilio_plugin_api::{LaunchEvent, LaunchOutcome, Permission};
use tokio::sync::mpsc;

use super::PluginHost;

static NEXT_LAUNCH: AtomicU64 = AtomicU64::new(1);

impl PluginHost {
    /// Queue observations without waiting for plugin metadata or code. Each
    /// observer gets its own ordered worker so a slow observer cannot hold up
    /// the game or another observer. A session produces at most two events.
    pub(crate) fn launch_events(self: &Arc<Self>) -> mpsc::UnboundedSender<LaunchEvent> {
        let (sender, mut receiver) = mpsc::unbounded_channel::<LaunchEvent>();
        let host = self.clone();
        let launch_id = NEXT_LAUNCH.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            let registry = host.registry().await;
            let mut observers = Vec::new();
            for entry in registry.entries.values() {
                if !entry
                    .manifest
                    .permissions
                    .contains(&Permission::LaunchEvents)
                {
                    continue;
                }
                let (sender, mut events) = mpsc::unbounded_channel::<LaunchEvent>();
                observers.push(sender);
                let host = host.clone();
                let id = entry.manifest.id.clone();
                tokio::spawn(async move {
                    while let Some(event) = events.recv().await {
                        host.call_scoped(
                            &id,
                            None,
                            host.timeout,
                            Some(launch_id),
                            move |plugin, ctx| match plugin.launch_observer() {
                                Some(observer) => observer.observe(ctx, &event),
                                None => Ok(()),
                            },
                        )
                        .await;
                    }
                });
            }
            let mut active_since = None;
            while let Some(event) = receiver.recv().await {
                match &event {
                    LaunchEvent::Started { .. } => active_since = Some(std::time::Instant::now()),
                    LaunchEvent::Exited { .. } => active_since = None,
                }
                for observer in &observers {
                    let _ = observer.send(event.clone());
                }
            }
            // Dropping a launch future kills its child (kill_on_drop). Close
            // the observed session too, rather than leaking native presence.
            if let Some(started) = active_since {
                let event = LaunchEvent::Exited {
                    outcome: LaunchOutcome::Stopped,
                    played_seconds: started.elapsed().as_secs(),
                };
                for observer in &observers {
                    let _ = observer.send(event.clone());
                }
            }
        });
        sender
    }
}
