//! Host-owned settings and failure isolation for synchronous plugin code.

mod access;
mod analysis;
mod content;
mod context;
mod network;
mod tabs;
pub use analysis::PluginFinding;
pub use content::PluginContentSource;
pub use tabs::{PluginEffect, PluginTab};
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use lumilio_plugin_api::{API_VERSION, HostContext, Manifest, Plugin, PluginError, PluginState};
use tokio::sync::OnceCell;

use context::Context;

const CALL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Failed { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginInfo {
    pub manifest: Manifest,
    pub state: PluginState,
    pub status: PluginStatus,
}

struct Entry {
    plugin: Arc<dyn Plugin>,
    manifest: Manifest,
    failure: Mutex<Option<String>>,
}

#[derive(Default)]
struct Registry {
    entries: BTreeMap<String, Arc<Entry>>,
    rejected: Vec<String>,
}

#[derive(Default)]
struct Preferences {
    states: BTreeMap<String, PluginState>,
    revisions: BTreeMap<String, u64>,
}

pub struct PluginHost {
    candidates: Vec<Arc<dyn Plugin>>,
    registry: OnceCell<Registry>,
    preferences: RwLock<Preferences>,
    /// UI state per (instance, plugin); owned by the host, not the plugins.
    tab_states: Mutex<BTreeMap<(String, String), lumilio_plugin_api::TabState>>,
    timeout: Duration,
    network: Option<network::Network>,
}

impl PluginHost {
    #[must_use]
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, states: BTreeMap<String, PluginState>) -> Self {
        Self {
            candidates: plugins,
            registry: OnceCell::new(),
            preferences: RwLock::new(Preferences {
                states,
                revisions: BTreeMap::new(),
            }),
            tab_states: Mutex::new(BTreeMap::new()),
            timeout: CALL_TIMEOUT,
            network: None,
        }
    }

    /// Supplies the launcher's transport and current mirror rules.
    #[must_use]
    pub fn with_network<T: crate::transfer::Transport>(
        self,
        transport: T,
        sources: crate::transfer::SourceChain,
    ) -> Self {
        self.with_network_config(transport, Some(sources))
    }

    pub(crate) fn with_network_config<T: crate::transfer::Transport>(
        mut self,
        transport: T,
        sources: Option<crate::transfer::SourceChain>,
    ) -> Self {
        self.network = Some(network::Network::new(Arc::new(transport), sources));
        self
    }

    /// Publish mirror changes only after their preferences have been saved.
    pub(crate) fn set_sources(&self, sources: crate::transfer::SourceChain) {
        if let Some(network) = &self.network {
            network.set_sources(sources);
        }
    }

    async fn registry(&self) -> &Registry {
        self.registry
            .get_or_init(|| async {
                let mut registry = Registry::default();
                for plugin in &self.candidates {
                    let candidate = plugin.clone();
                    match isolated(self.timeout, move || Ok(candidate.manifest())).await {
                        Ok(manifest) => {
                            if let Err(error) = validate_manifest(&manifest) {
                                registry.rejected.push(format!("{}: {error}", manifest.id));
                            } else if registry.entries.contains_key(&manifest.id) {
                                registry
                                    .rejected
                                    .push(format!("{}: duplicate plugin id", manifest.id));
                            } else {
                                registry.entries.insert(
                                    manifest.id.clone(),
                                    Arc::new(Entry {
                                        plugin: plugin.clone(),
                                        manifest,
                                        failure: Mutex::new(None),
                                    }),
                                );
                            }
                        }
                        Err(message) => registry.rejected.push(message),
                    }
                }
                registry
            })
            .await
    }

    pub async fn rejected(&self) -> Vec<String> {
        self.registry().await.rejected.clone()
    }

    pub async fn list(&self) -> Vec<PluginInfo> {
        let registry = self.registry().await;
        let preferences = self
            .preferences
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .entries
            .values()
            .map(|entry| {
                let state = preferences
                    .states
                    .get(&entry.manifest.id)
                    .cloned()
                    .unwrap_or_default();
                let failure = entry
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let status = match failure.as_ref() {
                    Some(message) => PluginStatus::Failed {
                        message: message.clone(),
                    },
                    None if enabled(&entry.manifest, &state) => PluginStatus::Enabled,
                    None => PluginStatus::Disabled,
                };
                PluginInfo {
                    manifest: entry.manifest.clone(),
                    state,
                    status,
                }
            })
            .collect()
    }

    /// Validates a complete preference group before the service persists it.
    pub async fn validate_state(&self, id: &str, state: &PluginState) -> Result<(), PluginError> {
        let entry = self
            .registry()
            .await
            .entries
            .get(id)
            .ok_or_else(|| PluginError::InvalidInput(format!("unknown plugin {id}")))?;
        for (key, value) in &state.values {
            let field = entry
                .manifest
                .settings
                .iter()
                .find(|field| field.key == *key);
            if !field.is_some_and(|field| field.kind.accepts(value)) {
                return Err(PluginError::InvalidInput(format!(
                    "invalid plugin setting {key}"
                )));
            }
        }
        Ok(())
    }

    /// Called only after persistence succeeds. Failures remain sticky until restart.
    pub fn set_state(&self, id: String, state: PluginState) {
        let mut preferences = self
            .preferences
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let revision = preferences.revisions.entry(id.clone()).or_default();
        *revision = revision.wrapping_add(1);
        preferences.states.insert(id, state);
    }

    /// All extension-point dispatch goes through this isolation boundary.
    /// A timed-out blocking task cannot be forcibly killed; its late result is
    /// discarded and no further calls are dispatched to that plugin this run.
    pub async fn call<R, F>(&self, id: &str, call: F) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&dyn Plugin, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        self.call_in(id, None, call).await
    }

    /// Like [`Self::call`], with read access to one game directory (within the
    /// plugin's `ReadGameFiles` grants).
    pub async fn call_in<R, F>(&self, id: &str, game_dir: Option<PathBuf>, call: F) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&dyn Plugin, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        self.call_in_with_timeout(id, game_dir, self.timeout, call)
            .await
    }

    async fn call_in_with_timeout<R, F>(
        &self,
        id: &str,
        game_dir: Option<PathBuf>,
        timeout: Duration,
        call: F,
    ) -> Option<R>
    where
        R: Send + 'static,
        F: FnOnce(&dyn Plugin, &dyn HostContext) -> Result<R, PluginError> + Send + 'static,
    {
        let entry = self.registry().await.entries.get(id)?.clone();
        let (state, revision) = {
            let preferences = self
                .preferences
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let state = preferences.states.get(id).cloned().unwrap_or_default();
            (state, preferences.revisions.get(id).copied().unwrap_or(0))
        };
        if !enabled(&entry.manifest, &state)
            || entry
                .failure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
        {
            return None;
        }
        let network = self
            .network
            .as_ref()
            .map(|network| network.context(timeout));
        let context = Context::new(&entry.manifest, &state, game_dir, network);
        let plugin = entry.plugin.clone();
        let result = isolated(timeout, move || call(plugin.as_ref(), &context)).await;
        match result {
            Ok(value) => {
                let preferences = self
                    .preferences
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let failed = entry
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some();
                (!failed && preferences.revisions.get(id).copied().unwrap_or(0) == revision)
                    .then_some(value)
            }
            Err(message) => {
                let mut failure = entry
                    .failure
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if failure.is_none() {
                    *failure = Some(message);
                }
                None
            }
        }
    }
}

fn enabled(manifest: &Manifest, state: &PluginState) -> bool {
    state.enabled.unwrap_or(manifest.default_enabled)
}

fn validate_manifest(manifest: &Manifest) -> Result<(), &'static str> {
    if manifest.api != API_VERSION {
        return Err("incompatible plugin API");
    }
    if !manifest.id.contains('.')
        || manifest.id.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
    {
        return Err("invalid plugin id");
    }
    let mut keys = std::collections::BTreeSet::new();
    for field in &manifest.settings {
        if field.key.is_empty()
            || !keys.insert(&field.key)
            || !field.kind.accepts(&field.kind.default_value())
        {
            return Err("invalid setting declaration");
        }
    }
    Ok(())
}

async fn isolated<R, F>(timeout: Duration, call: F) -> Result<R, String>
where
    R: Send + 'static,
    F: FnOnce() -> Result<R, PluginError> + Send + 'static,
{
    let task = tokio::task::spawn_blocking(move || catch_unwind(AssertUnwindSafe(call)));
    match tokio::time::timeout(timeout, task).await {
        Ok(Ok(Ok(Ok(value)))) => Ok(value),
        Ok(Ok(Ok(Err(error)))) => Err(error.to_string()),
        Ok(Ok(Err(_))) => Err("plugin panicked".to_owned()),
        Ok(Err(error)) => Err(format!("plugin worker failed: {error}")),
        Err(_) => Err("plugin call timed out".to_owned()),
    }
}
