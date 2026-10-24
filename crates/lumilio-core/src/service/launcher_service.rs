use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::accounts::Accounts;
use crate::accounts::credentials::CredentialStore;
use crate::activity::{ActivityLog, Tasks};
use crate::appearance::Appearance;
use crate::content::Content;
use crate::diagnostics::Diagnostics;
use crate::error::ServiceError;
use crate::game_settings::GameSettings;
use crate::instances::{InstanceLibrary, InstanceStore, Instances};
use crate::java::Java;
use crate::launching::Launching;
use crate::layout::Layout;
use crate::network::Network;
use crate::plugins::PluginControls;
use crate::recovery;
use crate::recovery::RecoveryNote;
use crate::settings::{Settings, SettingsStore};
use crate::transfer::{ConnectionBudget, Transport};
use crate::worlds::Worlds;

/// A download stopped part-way is continued if started again within this
/// long; after it, its bytes are removed when the launcher opens.
pub(super) const PARTIAL_DOWNLOAD_AGE: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 3600);

pub struct LauncherService<T> {
    pub(crate) plugins: Arc<crate::plugins::PluginHost>,
    pub(super) layout: Layout,
    pub(super) network: Network<T>,
    pub(crate) instances: Arc<Instances>,
    pub(super) settings: Arc<Mutex<SettingsStore>>,
    /// Published by both stores on every save (plan global-context).
    pub(super) context: crate::context::ContextHub,
    pub(super) tasks: Arc<Tasks>,
    /// What the start-up recovery found and did.
    pub(super) startup: Vec<RecoveryNote>,
    pub(super) launching: Launching,
    pub(super) java: Java<T>,
    pub(crate) game_settings: GameSettings,
    pub(super) plugin_controls: PluginControls,
    pub(super) diagnostics: Diagnostics,
    pub(crate) worlds: Worlds<T>,
    pub(crate) content: Content<T>,
    pub(crate) accounts: Accounts<T>,
    pub(crate) library: InstanceLibrary<T>,
    pub(super) preferences: Settings<T>,
    pub(crate) appearance: Appearance<T>,
    // Declared last so the stores close before ownership is released.
    pub(super) _root_lock: crate::root_lock::RootLock,
}

impl<T: Transport + Clone> LauncherService<T> {
    /// Opens (creating on first use) everything under `root`.
    /// `plugins` are trusted, statically linked core plugins supplied by the
    /// application. Untrusted registration uses `PluginHost::new` instead.
    ///
    /// A relative `root` is made absolute against the current directory:
    /// installer processors and the game run in other working directories,
    /// where relative library paths would point nowhere.
    pub fn open(
        root: impl Into<PathBuf>,
        transport: T,
        plugins: Vec<Arc<dyn lumilio_plugin_api::Plugin>>,
    ) -> Result<Self, ServiceError> {
        let layout = Layout::new(std::path::absolute(root.into())?);
        let root_lock = crate::root_lock::RootLock::acquire(layout.root())?;
        let mut store = InstanceStore::open(layout.root())?;
        let mut settings = SettingsStore::open(layout.root())?;
        let log = ActivityLog::open(layout.root());
        let startup = recovery::startup_notes(&layout, &store, &settings, &log);
        crate::transfer::sweep_partials(&layout.partial_downloads(), PARTIAL_DOWNLOAD_AGE);
        // A saved current game that is gone is replaced before anything reads
        // it; a failed save leaves it to read as none.
        let _ = crate::settings::settle_current_in(store.instances(), &mut settings);
        let context = crate::context::ContextHub::new(settings.get());
        store.publish_to(context.clone());
        settings.publish_to(context.clone());
        let plugins = crate::plugins::PluginHost::new_core(plugins, settings.get().plugins.clone())
            // Invalid hand-edited mirrors must not make plugin initialization
            // prevent the launcher from opening. Network calls fail locally
            // until corrected; the rest of the service remains available.
            .with_network_config(transport.clone(), settings.source_chain().ok());
        let settings = Arc::new(Mutex::new(settings));
        let network = Network::new(transport, settings.clone(), layout.partial_downloads())?;
        let instances = Arc::new(Instances::new(store));
        let tasks = Arc::new(Tasks::new(log));
        let java = Java::new(
            settings.clone(),
            network.clone(),
            layout.clone(),
            tasks.clone(),
            context.clone(),
            instances.clone(),
        );
        let plugins = Arc::new(plugins);
        let translation = Arc::new(crate::translation::TranslationService::new(
            layout.root().to_owned(),
        ));
        Ok(Self {
            preferences: Settings::new(
                settings.clone(),
                context.clone(),
                instances.clone(),
                network.clone(),
                plugins.clone(),
                translation.clone(),
            ),
            library: InstanceLibrary::new(
                instances.clone(),
                layout.clone(),
                network.clone(),
                tasks.clone(),
                context.clone(),
                settings.clone(),
                plugins.clone(),
            ),
            accounts: Accounts::new(settings.clone(), network.clone(), layout.clone()),
            appearance: Appearance::new(
                settings.clone(),
                network.clone(),
                layout.clone(),
                instances.clone(),
            ),
            content: Content::new(
                instances.clone(),
                layout.clone(),
                network.clone(),
                plugins.clone(),
                tasks.clone(),
                settings.clone(),
                translation.clone(),
            ),
            worlds: Worlds::new(
                instances.clone(),
                layout.clone(),
                plugins.clone(),
                network.clone(),
                settings.clone(),
                tasks.clone(),
            ),
            diagnostics: Diagnostics::new(
                settings.clone(),
                instances.clone(),
                layout.clone(),
                plugins.clone(),
                translation.clone(),
            ),
            plugin_controls: PluginControls::new(
                plugins.clone(),
                settings.clone(),
                instances.clone(),
                layout.clone(),
            ),
            game_settings: GameSettings::new(instances.clone(), layout.clone()),
            java,
            network,
            plugins,
            layout,
            instances,
            settings,
            context,
            tasks,
            launching: Launching::default(),
            startup,
            _root_lock: root_lock,
        })
    }

    /// Keeps sign-in secrets in this store instead of the system one (tests).
    #[must_use]
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialStore>) -> Self {
        self.accounts.set_credentials(credentials);
        self
    }

    /// What start-up recovery found and did, for Diagnostics and Activity.
    #[must_use]
    pub fn startup_notes(&self) -> &[RecoveryNote] {
        &self.startup
    }

    /// Looks for Java only in these folders instead of the conventional
    /// system locations.
    #[must_use]
    pub fn with_runtime_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.java.set_runtime_roots(roots);
        self
    }

    /// Runs `plugins` instead of the ones the service opened with (tests).
    #[cfg(test)]
    pub(crate) fn set_plugin_host(&mut self, plugins: Arc<crate::plugins::PluginHost>) {
        self.plugin_controls.replace_plugins(plugins.clone());
        self.diagnostics.replace_plugins(plugins.clone());
        self.worlds.replace_plugins(plugins.clone());
        self.content.replace_plugins(plugins.clone());
        self.library.replace_plugins(plugins.clone());
        self.preferences.replace_plugins(plugins.clone());
        self.plugins = plugins;
    }

    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }

    /// The connections every download shares, so the launcher's own update
    /// downloads on them too (plan download-engine T4).
    #[must_use]
    pub fn connection_budget(&self) -> ConnectionBudget {
        self.network.budget()
    }
}
