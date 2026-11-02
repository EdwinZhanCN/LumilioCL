use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use super::DiscoverFilters;
use super::recognition_cache::RecognitionCache;
use crate::activity::Tasks;
use crate::content::translation::SharedTranslation;
use crate::instances::Instances;
use crate::network::Network;
use crate::plugins::PluginHost;
use crate::settings::SettingsStore;
use crate::storage::Layout;

/// Mods, packs and shaders in a game: finding them in the content sources,
/// installing and switching their versions, recognizing what is installed and
/// translating what the sources say about it.
pub(crate) struct Content<T> {
    pub(super) instances: Arc<Instances>,
    pub(super) layout: Layout,
    pub(super) network: Network<T>,
    pub(super) plugins: Arc<PluginHost>,
    pub(super) tasks: Arc<Tasks>,
    pub(super) settings: Arc<Mutex<SettingsStore>>,
    /// Filter choices per content source, for the life of the service.
    pub(super) filters: Mutex<BTreeMap<String, DiscoverFilters>>,
    /// Translated summaries and bodies, cached in their own SQLite file.
    pub(super) translation: SharedTranslation,
    /// What the sources said about installed files, and the files' hashes.
    pub(super) recognition: RecognitionCache,
}

impl<T> Content<T> {
    pub(crate) fn new(
        instances: Arc<Instances>,
        layout: Layout,
        network: Network<T>,
        plugins: Arc<PluginHost>,
        tasks: Arc<Tasks>,
        settings: Arc<Mutex<SettingsStore>>,
        translation: SharedTranslation,
    ) -> Self {
        Self {
            recognition: RecognitionCache::new(layout.root().join("cache/content.sqlite")),
            instances,
            layout,
            network,
            plugins,
            tasks,
            settings,
            filters: Mutex::default(),
            translation,
        }
    }

    #[cfg(test)]
    pub(crate) fn replace_plugins(&mut self, plugins: Arc<PluginHost>) {
        self.plugins = plugins;
    }
}
