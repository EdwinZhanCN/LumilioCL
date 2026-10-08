use super::discover::DiscoverQuery;
use gpui::{App, Window};
use lumilio_core::{
    DownloadSourcePreference, LaunchTuning, MirrorPreset, MirrorRule, Preferences, ProjectKind,
};
use std::path::PathBuf;
use std::rc::Rc;

/// What a live page asks the application to do.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveIntent {
    /// Create an instance with the newest release.
    NewInstance,
    /// Choose a local `.mrpack` and import it as a new instance.
    ImportPack,
    ToggleFavorite(String),
    /// Open the registered instance without launching it.
    OpenInstance(String),
    /// Launch this instance from Home's launch moment.
    Play(String),
    /// Launch this instance straight into one of its worlds or servers.
    PlayPlace(String, super::PlaceTarget),
    /// Run this Discover search.
    Search(DiscoverQuery),
    /// Fetch the category and game-version choices.
    LoadFilters,
    /// Open a project's public page in its own window.
    OpenProject {
        kind: ProjectKind,
        slug: String,
    },
    /// Install a project: into the chosen instance, or as a new one for a modpack.
    Install {
        kind: ProjectKind,
        slug: String,
        title: String,
        /// One specific version, for mods, packs and shaders.
        version: Option<String>,
    },
    /// Stop this running task (by the service's task id).
    CancelTask(u64),
    /// Forget every finished task on the Activity page.
    ClearFinished,
    /// Run a failed or cancelled task again from its original input.
    RetryTask(lumilio_core::RetryAction),
    /// Ask for a name and make a collection.
    NewCollection,
    /// Ask for another name for this collection.
    RenameCollection(String),
    /// Remove this collection; its games stay in the library.
    DeleteCollection(String),
    /// Ask which collections this game (by id) belongs to.
    EditCollections(String),
    /// Open this game and ask how to export it as a modpack.
    ExportPackOf(String),
    /// Open this game and ask for a name to copy it under.
    CopyGameOf(String),
    /// Open this game and ask whether to delete it.
    DeleteGameOf(String),
    /// Show this game's folder in the file manager.
    RevealGame(String),
    /// Download one version's file where the person chooses (the application
    /// asks where).
    SaveVersionAs {
        project: String,
        version_id: String,
        file_name: String,
        title: String,
    },
    /// Remember how the Library is ordered and filtered (see
    /// `Preferences::library_sort`).
    RememberLibraryView {
        sort: u8,
        loader: u8,
    },
    /// Remember what Discover keeps between visits (see
    /// `Preferences::discover`).
    RememberDiscover(lumilio_core::DiscoverPreferences),
    /// Measure the shared game files no game uses and offer to remove them.
    CheckReclaimable,
    /// Remove them (after the person confirmed).
    Reclaim,
    /// Choose a folder made by another launcher and bring its game over.
    ImportGame,
    /// Choose a backup file and make a new game of it.
    RestoreBackup,
    /// Show the folder the launcher keeps its games in.
    OpenGamesFolder,
    /// Download Java into the launcher (the major version, if wanted).
    InstallJava(Option<u32>),
    /// Replace an installed file with its newer version (from Discover).
    UpdateInstalled {
        kind: ProjectKind,
        project: String,
        title: String,
        file_name: String,
        version_id: String,
    },
    /// Open this game and do what a problem's button says (from Home).
    Resolve(String, crate::instance_detail::ProblemAction),
    /// Files dropped on the Library page.
    DropFiles(Vec<PathBuf>),
    /// Choose which instance Discover installs into (the current instance).
    InstallTarget(String),
    /// Open the add-offline-account dialog.
    NewAccount,
    /// Open the Microsoft sign-in dialog.
    MicrosoftSignIn,
    /// Open the sign-in dialog for authentication servers (LittleSkin and others).
    ThirdPartySignIn,
    /// Open the list of authentication servers.
    ManageAuthServers,
    OpenSkinSite(String),
    /// Open the skin dialog of this offline account (by key).
    EditSkin(String),
    /// Load what this account (by key) looks like, for the detail's preview;
    /// the answer goes to `LauncherShell::account_look`.
    LoadAccountLook(String),
    /// An appearance operation bound to one retained account detail.
    Wardrobe {
        key: String,
        revision: u64,
        action: crate::wardrobe::WardrobeAction,
    },
    SaveInlineSkin {
        revision: u64,
        intent: crate::skin_dialog::SkinIntent,
    },
    /// Refresh this Microsoft account's sign-in now.
    RefreshAccount(String),
    /// Use this account (by key) for later launches.
    SelectAccount(String),
    /// Forget this account (by key); games and worlds stay.
    RemoveAccount(String),
    /// Put text on the clipboard and say so in a toast.
    CopyText {
        text: String,
        notice: String,
    },
    /// Read the settings, Java list and disk use again.
    LoadSettings,
    SetPluginEnabled {
        id: String,
        enabled: bool,
    },
    SetPluginValue {
        id: String,
        key: String,
        value: lumilio_plugin_api::SettingValue,
    },
    ResetPlugin(String),
    EditPluginSetting {
        id: String,
        key: String,
    },
    SetPreferences(Preferences),
    SetLaunchDefaults(LaunchTuning),
    SetMemory {
        min_mb: Option<u32>,
        max_mb: Option<u32>,
    },
    SetConcurrency(Option<u32>),
    AddMirrorPreset(MirrorPreset),
    SetMirrors(Vec<MirrorRule>),
    SetDownloadSource(DownloadSourcePreference),
    SetJavaRoots(Vec<PathBuf>),
    /// Turn an installation (by its home folder) off or on.
    SetJavaDisabled {
        home: PathBuf,
        disabled: bool,
    },
    /// Choose a Java (executable or home folder) to add.
    AddJava,
    /// Show a file or folder in the system file manager.
    Reveal(PathBuf),
    ClearCache,
    /// Choose where to save the diagnostics bundle, then write it.
    ExportDiagnostics,
    Refresh,
}

pub type LiveHandler = Rc<dyn Fn(LiveIntent, &mut Window, &mut App)>;
