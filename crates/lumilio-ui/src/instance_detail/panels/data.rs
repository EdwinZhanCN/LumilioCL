use super::super::Section;
use super::CONTENT_KINDS;
use lumilio_core::{
    ContentList, FileEntry, GameLogs, HistoryRead, Problem, ProjectKind, ServerEntry, ServerStatus,
    SnapshotInfo, WorldInfo,
};

pub type Loaded<T> = Option<Result<T, String>>;

/// What the sections show. `None` means not loaded yet; an old value stays on
/// screen while a refresh is under way.
#[derive(Default)]
pub struct Data {
    pub content: [Loaded<ContentList>; 3],
    pub worlds: Loaded<Vec<WorldInfo>>,
    pub servers: Loaded<Vec<ServerEntry>>,
    pub snapshots: Loaded<Vec<SnapshotInfo>>,
    pub history: Loaded<HistoryRead>,
    pub problems: Loaded<Vec<Problem>>,
    pub logs: Loaded<GameLogs>,
    /// The folder shown in 文件 (relative to the game directory) and its entries.
    pub files: Loaded<(String, Vec<FileEntry>)>,
    pub size: Loaded<u64>,
    pub pending: Vec<Section>,
}

/// One data set arriving from the service.
pub enum Arrived {
    Content(ProjectKind, Result<ContentList, String>),
    Worlds(Result<Vec<WorldInfo>, String>),
    Servers(Result<Vec<ServerEntry>, String>),
    Snapshots(Result<Vec<SnapshotInfo>, String>),
    History(Result<HistoryRead, String>),
    Problems(Result<Vec<Problem>, String>),
    Logs(Result<GameLogs, String>),
    Files(String, Result<Vec<FileEntry>, String>),
    Size(Result<u64, String>),
}

impl Arrived {
    pub fn section(&self) -> Section {
        match self {
            Self::Content(kind, _) => Section::Content(*kind),
            Self::Worlds(_) => Section::Worlds,
            Self::Servers(_) => Section::Servers,
            Self::Snapshots(_) => Section::Snapshots,
            Self::History(_) => Section::History,
            Self::Problems(_) => Section::Problems,
            Self::Logs(_) => Section::Logs,
            Self::Files(..) => Section::Files,
            Self::Size(_) => Section::Size,
        }
    }
}

impl Data {
    pub fn store(&mut self, arrived: Arrived) {
        self.pending.retain(|section| *section != arrived.section());
        match arrived {
            Arrived::Content(kind, result) => {
                if let Some(index) = CONTENT_KINDS.iter().position(|k| *k == kind) {
                    self.content[index] = Some(result);
                }
            }
            Arrived::Worlds(result) => self.worlds = Some(result),
            Arrived::Servers(result) => self.servers = Some(result),
            Arrived::Snapshots(result) => self.snapshots = Some(result),
            Arrived::History(result) => self.history = Some(result),
            Arrived::Problems(result) => self.problems = Some(result),
            Arrived::Logs(result) => self.logs = Some(result),
            Arrived::Size(result) => self.size = Some(result),
            Arrived::Files(folder, result) => {
                self.files = Some(result.map(|entries| (folder, entries)));
            }
        }
    }

    pub fn has(&self, section: Section) -> bool {
        match section {
            Section::Content(kind) => CONTENT_KINDS
                .iter()
                .position(|k| *k == kind)
                .is_some_and(|index| self.content[index].is_some()),
            Section::Worlds => self.worlds.is_some(),
            Section::Servers => self.servers.is_some(),
            Section::Snapshots => self.snapshots.is_some(),
            Section::History => self.history.is_some(),
            Section::Problems => self.problems.is_some(),
            Section::Logs => self.logs.is_some(),
            Section::Files => self.files.is_some(),
            Section::Size => self.size.is_some(),
        }
    }
}

/// What is known of a server's state right now.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerState {
    Checking,
    Online(ServerStatus),
    Offline,
}

/// A destructive step waiting for a second click.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Confirm {
    DeleteWorld(String),
    DeleteServer { index: usize, entry: ServerEntry },
    DeleteSnapshot(String),
    RestoreSnapshot(String),
}

impl Confirm {
    /// The question and what it costs, for the dialog; the last is the
    /// confirming button's label.
    pub fn words(
        &self,
        worlds: &[WorldInfo],
        snapshots: &[SnapshotInfo],
    ) -> (String, String, &'static str) {
        match self {
            Self::DeleteWorld(folder) => {
                let name = worlds
                    .iter()
                    .find(|world| &world.folder == folder)
                    .map_or(folder.as_str(), |world| world.name.as_str());
                (
                    format!("删除世界“{name}”？"),
                    "世界和里面的东西都会删除，之后不能找回。想留个后手的话，先创建备份。"
                        .to_owned(),
                    "删除",
                )
            }
            Self::DeleteServer { entry, .. } => (
                format!("删除服务器“{}”？", entry.name),
                "只从这个游戏的列表里移除，不影响服务器本身；之后可以重新添加。".to_owned(),
                "删除",
            ),
            Self::RestoreSnapshot(id) => {
                let label = snapshots
                    .iter()
                    .find(|snapshot| &snapshot.id == id)
                    .map_or("这份快照", |snapshot| snapshot.label.as_str());
                (
                    format!("恢复“{label}”？"),
                    "恢复会用快照替换现有的世界和设置，当前状态会先自动存一份；中途失败或退出会回到恢复前的样子。".to_owned(),
                    "恢复",
                )
            }
            Self::DeleteSnapshot(id) => {
                let label = snapshots
                    .iter()
                    .find(|snapshot| &snapshot.id == id)
                    .map_or("这份快照", |snapshot| snapshot.label.as_str());
                (
                    format!("删除快照“{label}”？"),
                    "删除后不能再恢复到这个时间点。".to_owned(),
                    "删除",
                )
            }
        }
    }
}
