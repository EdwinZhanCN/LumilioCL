use super::panels::ProblemAction;
use lumilio_core::{InstanceSettings, ProjectKind, ServerEntry};

/// A piece of data the sections load on demand.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Section {
    Content(ProjectKind),
    Worlds,
    Servers,
    Screenshots,
    Snapshots,
    History,
    Problems,
    Logs,
    Files,
    Size,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstanceIntent {
    Play,
    Rename(String),
    SaveMemory(InstanceSettings),
    /// Save every instance setting after one dialog changed its part.
    SaveSettings(InstanceSettings),
    Reload,
    /// Fetch this data and answer with [`InstanceDetailView::arrived`].
    Load(Section),
    SetContent {
        kind: ProjectKind,
        files: Vec<String>,
        enabled: bool,
    },
    DeleteContent {
        kind: ProjectKind,
        files: Vec<String>,
    },
    CopyWorld(String),
    /// Back up this world alone (a snapshot of just that world).
    BackupWorld(String),
    /// Ask where to save this world as a .zip, then export it.
    ExportWorld(String),
    /// The place chosen after [`InstanceIntent::ExportWorld`]; the
    /// application sends this to itself.
    ExportWorldTo {
        folder: String,
        path: std::path::PathBuf,
    },
    /// Ask for a world .zip and add it to the saves.
    ImportWorld,
    /// Open the export dialog; the application lists the game's files for it.
    ExportPack,
    /// The pack chosen in that dialog; the application asks where to save it.
    ExportPackAs(lumilio_core::ExportSpec),
    /// Write the pack here; the application sends this to itself.
    ExportPackTo {
        spec: lumilio_core::ExportSpec,
        path: std::path::PathBuf,
    },
    /// The archive chosen after [`InstanceIntent::ImportWorld`]; the
    /// application sends this to itself.
    AddWorld(std::path::PathBuf),
    /// Start the game and go straight into this world (by folder name).
    PlayWorld(String),
    DeleteWorld(String),
    /// Add a server (`index` is `None`) or replace the one at `index`, which
    /// must still read `expected`.
    SaveServer {
        index: Option<usize>,
        expected: Option<ServerEntry>,
        entry: ServerEntry,
    },
    DeleteServer {
        index: usize,
        expected: ServerEntry,
    },
    MoveServer {
        index: usize,
        expected: ServerEntry,
        to: usize,
    },
    /// Ask this server how it is; answer with
    /// [`InstanceDetailView::server_status_arrived`].
    PingServer(String),
    /// Start the game and go straight onto this server.
    PlayServer(String),
    /// Make this screenshot's thumbnail; answer with
    /// [`InstanceDetailView::thumbnail_arrived`].
    Thumbnail(String),
    /// Put this screenshot on the clipboard as a picture.
    CopyScreenshot(String),
    DeleteScreenshot(String),
    /// Which plugin tabs show for this game; answer with
    /// [`InstanceDetailView::plugin_tabs_arrived`].
    PluginTabs,
    /// What this plugin's tab shows; answer with
    /// [`InstanceDetailView::plugin_view_arrived`].
    PluginView(String),
    /// Run one action of a plugin's tab. The application performs the effects
    /// the plugin asks for and answers with the tab's new view.
    PluginAction {
        plugin: String,
        action: lumilio_plugin_api::ActionId,
    },
    /// Read assets for this inline preview. The entity ID keeps late results
    /// tied to the view that requested them.
    LoadModel {
        plugin: String,
        file: String,
        request: u64,
    },
    LoadMap {
        request: u64,
    },
    /// Read this crash report; answer with [`InstanceDetailView::crash_arrived`].
    OpenCrash(String),
    OpenGameLog(lumilio_core::GameLogSource),
    AnalyzeGameLog {
        request: u64,
        text: String,
        crash: bool,
    },
    ExportGameLog {
        source: lumilio_core::GameLogSource,
        live: String,
    },
    /// List this folder of the game directory (`""` is the directory itself);
    /// answer with [`Arrived::Files`].
    OpenFolder(String),
    /// Open the Accounts page.
    OpenAccounts,
    /// Once the page has the game: open the copy dialog. (Sent by the
    /// application after it opens the page for the Library's menu.)
    AskCopy,
    /// Once the page has the game: ask whether to delete it.
    AskDelete,
    /// Once the page has the game: do what a problem's button says. (Sent by
    /// the application after Home's button opens the page.)
    Resolve(ProblemAction),
    /// Ask where to save a full backup of this game; the application then
    /// sends [`InstanceIntent::BackupGameTo`] to itself.
    BackupGame,
    BackupGameTo(std::path::PathBuf),
    /// Make this game the current one (what Continue starts).
    SetCurrent,
    /// Stop the game that is starting or running.
    Stop,
    /// Save the latest log (or this crash report) with names and paths hidden;
    /// the application asks where.
    ExportLog(Option<String>),
    /// Show this path of the game directory in the file manager.
    RevealPath(String),
    CreateSnapshot,
    /// A snapshot with a note, of everything or of one world (by folder).
    CreateSnapshotAs {
        note: String,
        world: Option<String>,
    },
    RestoreSnapshot(String),
    DeleteSnapshot(String),
    /// Download the game files without launching.
    Install,
    /// Download Java (the needed major, if known) into the launcher.
    InstallJava(Option<u32>),
    /// Check every game file and fetch what is missing or damaged again.
    Repair,
    /// Open the dialog that changes the game version and loader.
    OpenRuntimeChange,
    /// Change to this game version and loader (from that dialog).
    ChangeRuntime {
        loader: lumilio_core::Loader,
        game_version: String,
        loader_version: Option<String>,
    },
    /// Copy this instance under a new name.
    Copy {
        name: String,
        include_worlds: bool,
    },
    /// Delete this instance; the answer closes the view on success.
    Delete,
    /// Replace an installed file with another version of its project.
    SwitchContent {
        kind: ProjectKind,
        file_name: String,
        project: String,
        version_id: String,
    },
    /// Update several files: (file, project, version) each.
    UpdateContent {
        kind: ProjectKind,
        updates: Vec<(String, String, String)>,
    },
    /// Ask for local files and add them (P-ADD-FILES).
    ImportContent(ProjectKind),
    /// A project's versions; answer with [`InstanceDetailView::versions_arrived`].
    LoadVersions(String),
    /// Show an installed file in the file manager.
    RevealContent {
        kind: ProjectKind,
        file_name: String,
    },
    /// Open a project's page in Discover.
    OpenProject {
        kind: ProjectKind,
        slug: String,
    },
    /// Browse Discover for this kind, to add to this game.
    BrowseContent(ProjectKind),
    /// The files chosen after [`InstanceIntent::ImportContent`]; the
    /// application sends this to itself.
    AddFiles {
        kind: ProjectKind,
        files: Vec<std::path::PathBuf>,
    },
}
