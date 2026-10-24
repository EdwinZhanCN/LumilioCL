use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};

use tokio::sync::Mutex;

use super::{appearance, preferences, third_party};
use crate::activity::{ActivityLog, Tasks};
use crate::content::Content;
use crate::credentials::{CredentialStore, SystemCredentials};
use crate::diagnostics::Diagnostics;
use crate::error::ServiceError;
use crate::game_settings::GameSettings;
use crate::instances::{InstanceStore, Instances, Loader};
use crate::java::Java;
use crate::launching::Launching;
use crate::layout::Layout;
use crate::network::Network;
use crate::plugins::PluginControls;
use crate::recovery;
use crate::recovery::RecoveryNote;
use crate::settings::SettingsStore;
use crate::transfer::{ConnectionBudget, Transport};
use crate::worlds::Worlds;

/// `Fabric 0.16.0` / `Vanilla`, for a history line.
pub(super) fn loader_text(loader: Loader, version: Option<&str>) -> String {
    match version {
        Some(version) => format!("{loader:?} {version}"),
        None => format!("{loader:?}"),
    }
}

/// Instances diagnosed for Home: verifying files is not free, so only the
/// ones a person is likely to care about right now.
pub(super) const DIAGNOSED_LIMIT: usize = 8;
/// A download stopped part-way is continued if started again within this
/// long; after it, its bytes are removed when the launcher opens.
pub(super) const PARTIAL_DOWNLOAD_AGE: std::time::Duration =
    std::time::Duration::from_secs(7 * 24 * 3600);
/// A Minecraft token with less than this left is refreshed before a launch.
pub(super) const TOKEN_MARGIN_SECONDS: u64 = 300;
/// A key that is never an account, used to see whether the store works.
pub(super) const CREDENTIAL_PROBE: &str = "lumilio-store-check";

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
    /// Translated summaries and bodies, cached in their own SQLite file.
    pub(super) translation: crate::translation::SharedTranslation,
    /// Where sign-in secrets live; never a file.
    pub(super) credentials: Arc<dyn CredentialStore>,
    /// One refresh at a time: refresh tokens rotate, so two at once would
    /// invalidate each other.
    pub(super) auth_lock: Mutex<()>,
    /// Library index writes and per-profile appearance changes serialize,
    /// independently of credential refresh.
    pub(super) wardrobe_lock: Mutex<()>,
    pub(super) profiles: appearance::Profiles,
    pub(super) textures: crate::skin::cache::TextureCache,
    /// Third-party sign-ins waiting for the person to choose a character:
    /// the session stays in memory only, until chosen or abandoned.
    pub(super) pending_sign_ins: StdMutex<BTreeMap<u64, third_party::PendingSignIn>>,
    pub(super) next_pending: std::sync::atomic::AtomicU64,
    /// A client token to use instead of a random one (tests only: a scripted
    /// server cannot echo what it was sent).
    pub(super) client_token_override: Option<String>,
    /// The agent was looked up for a newer build in this run already.
    pub(super) injector_checked: std::sync::atomic::AtomicBool,
    /// The key the local skin server signs with, made on first use.
    pub(super) skin_signer: tokio::sync::OnceCell<Arc<crate::skin::Signer>>,
    /// Size of that key (smaller in tests, where generating one is slow).
    pub(super) skin_key_bits: usize,
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
        let _ = preferences::settle_current(store.instances(), &mut settings);
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
            translation,
            textures: crate::skin::cache::TextureCache::new(
                layout.root().join("cache/skin-textures"),
            ),
            profiles: appearance::Profiles::default(),
            layout,
            instances,
            settings,
            context,
            tasks,
            launching: Launching::default(),
            startup,
            credentials: Arc::new(SystemCredentials),
            auth_lock: Mutex::new(()),
            wardrobe_lock: Mutex::new(()),
            pending_sign_ins: StdMutex::default(),
            next_pending: std::sync::atomic::AtomicU64::new(1),
            client_token_override: None,
            injector_checked: std::sync::atomic::AtomicBool::new(false),
            skin_signer: tokio::sync::OnceCell::new(),
            skin_key_bits: crate::skin::KEY_BITS,
            _root_lock: root_lock,
        })
    }

    /// Keeps sign-in secrets in this store instead of the system one (tests).
    #[must_use]
    pub fn with_credentials(mut self, credentials: Arc<dyn CredentialStore>) -> Self {
        self.credentials = credentials;
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
