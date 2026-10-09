//! Synchronous, UI-independent contracts for the launcher's built-in plugins.
//! The host supplies all I/O and invokes plugin code on a worker thread.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

mod analysis;
pub mod content;
mod launch;
pub mod map;
mod network;
#[cfg(test)]
mod tests;
mod view;
pub use analysis::{
    AnalysisInput, AnalysisSource, Analyzer, Finding, GameFacts, ModFact, Severity,
};
pub use content::ContentSource;
pub use launch::{DiscordActivity, LaunchEvent, LaunchObserver, LaunchOutcome, LaunchTarget};
pub use network::{FetchMethod, FetchRequest};
pub use view::{ActionId, Effect, ImageData, InstanceTab, KeyKind, ListItem, TabState, Tone, View};

pub const API_VERSION: u32 = 1;

/// A plugin's own words in both languages the launcher speaks. A plugin
/// supplies them together; the host picks by the launcher's locale and never
/// translates plugin text itself.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Words {
    pub zh_cn: String,
    pub en: String,
}

impl Words {
    #[must_use]
    pub fn new(zh_cn: impl Into<String>, en: impl Into<String>) -> Self {
        Self {
            zh_cn: zh_cn.into(),
            en: en.into(),
        }
    }

    /// The words for `locale`: English when the tag starts with `en` and an
    /// English string exists, Simplified Chinese otherwise.
    #[must_use]
    pub fn get(&self, locale: &str) -> &str {
        if locale.starts_with("en") && !self.en.is_empty() {
            &self.en
        } else {
            &self.zh_cn
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub id: String,
    pub name: Words,
    pub description: Words,
    pub version: String,
    pub api: u32,
    pub default_enabled: bool,
    pub permissions: Vec<Permission>,
    pub settings: Vec<SettingField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Permission {
    ReadGameFiles {
        under: String,
    },
    /// Writes below `under`, only to files whose name matches `names`: one `*`
    /// stands for any text (`mw$*.txt`). The host also refuses to write while
    /// the game is running or the file changed since the plugin read it.
    WriteGameFiles {
        under: String,
        names: String,
    },
    Network {
        hosts: Vec<String>,
    },
    LaunchEvents,
    Native(NativeCapability),
}

/// A file's size and modification time, without reading it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileStat {
    pub len: u64,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// One page of a directory, sorted by name. `next` is the name to pass as
/// `after` for the following page; `None` means this was the last one.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DirPage {
    pub entries: Vec<DirEntry>,
    pub next: Option<String>,
}

/// Largest range one `read_range` call returns.
pub const MAX_RANGE: usize = 8 * 1024 * 1024;
/// Most entries one `list_dir` page holds.
pub const MAX_PAGE: usize = 1000;

/// What a file looked like when a plugin read it. A write is refused unless
/// the file still looks exactly like this.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileInfo {
    pub len: u64,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: i64,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NativeCapability {
    DiscordIpc,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettingField {
    pub key: String,
    pub label: Words,
    pub help: Words,
    pub kind: SettingKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SettingKind {
    Toggle {
        default: bool,
    },
    /// `options` are stored values, never shown text: a plugin matches them
    /// against its own logic and the host persists whichever one is chosen.
    /// They are not translated, so they stay identical in every language.
    Choice {
        options: Vec<String>,
        default: String,
    },
    Text {
        default: String,
    },
    Number {
        min: i64,
        max: i64,
        default: i64,
    },
}

impl SettingKind {
    #[must_use]
    pub fn default_value(&self) -> SettingValue {
        match self {
            Self::Toggle { default } => SettingValue::Toggle(*default),
            Self::Choice { default, .. } => SettingValue::Choice(default.clone()),
            Self::Text { default } => SettingValue::Text(default.clone()),
            Self::Number { default, .. } => SettingValue::Number(*default),
        }
    }

    #[must_use]
    pub fn accepts(&self, value: &SettingValue) -> bool {
        match (self, value) {
            (Self::Toggle { .. }, SettingValue::Toggle(_))
            | (Self::Text { .. }, SettingValue::Text(_)) => true,
            (Self::Choice { options, .. }, SettingValue::Choice(value)) => options.contains(value),
            (Self::Number { min, max, .. }, SettingValue::Number(value)) => {
                (*min..=*max).contains(value)
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SettingValue {
    Toggle(bool),
    Choice(String),
    Text(String),
    Number(i64),
}

/// Only user preferences are persisted; runtime failures belong to the host.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PluginState {
    pub enabled: Option<bool>,
    pub values: BTreeMap<String, SettingValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PluginError {
    PermissionDenied,
    InvalidInput(String),
    Unavailable(String),
    /// The outside world did not cooperate (no connection, an HTTP error
    /// status): this call fails, but the plugin is not at fault and the host
    /// keeps it running, so a retry can succeed.
    Transient(String),
}

impl Display for PluginError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::PermissionDenied => f.write_str("permission denied"),
            Self::InvalidInput(message) | Self::Unavailable(message) | Self::Transient(message) => {
                f.write_str(message)
            }
        }
    }
}

impl Error for PluginError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FetchResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait HostContext: Send + Sync {
    /// The launcher's language tag, such as `zh-CN` or `en`. A plugin picks
    /// its words with [`Words::get`] against this.
    fn locale(&self) -> &str {
        "zh-CN"
    }
    /// Cooperative cancellation for bounded map work on a host worker.
    fn cancelled(&self) -> bool {
        false
    }
    /// Identifies this observed process, including when several games overlap.
    fn launch_id(&self) -> Option<u64> {
        None
    }
    /// Changes whenever this plugin's saved preferences change.
    fn settings_revision(&self) -> u64 {
        0
    }
    /// Host-owned Discord IPC. `None` clears this plugin's activity.
    /// Missing Discord is a successful, quiet skip.
    fn discord_activity(&self, _activity: Option<DiscordActivity>) -> Result<(), PluginError> {
        Err(PluginError::PermissionDenied)
    }
    fn setting(&self, key: &str) -> Option<SettingValue>;
    /// Reads a file below a granted `ReadGameFiles` directory. Paths are
    /// relative to the game directory.
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError>;
    /// Lists files (recursively, relative to the game directory) below a
    /// granted `ReadGameFiles` directory.
    fn list_files(&self, dir: &str) -> Result<Vec<String>, PluginError>;
    /// Size and modification time of a file below a granted `ReadGameFiles`
    /// directory; `Ok(None)` when it does not exist.
    fn file_stat(&self, _path: &str) -> Result<Option<FileStat>, PluginError> {
        Err(PluginError::PermissionDenied)
    }
    /// Up to `len` bytes (at most [`MAX_RANGE`]) from `offset`, so a file larger
    /// than a whole-file read allows can still be read in pieces. A range
    /// that runs past the end is cut short.
    fn read_range(&self, _path: &str, _offset: u64, _len: usize) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::PermissionDenied)
    }
    /// One directory level, paged, below a granted `ReadGameFiles` directory.
    /// A missing directory is an empty page.
    fn list_dir(
        &self,
        _dir: &str,
        _after: Option<&str>,
        _limit: usize,
    ) -> Result<DirPage, PluginError> {
        Err(PluginError::PermissionDenied)
    }
    /// Reads a file below a granted `ReadGameFiles` directory together with
    /// the [`FileInfo`] to pass back when writing it.
    fn read_file_info(&self, _path: &str) -> Result<(Vec<u8>, FileInfo), PluginError> {
        Err(PluginError::PermissionDenied)
    }
    /// Replaces a file below a `WriteGameFiles` grant. `expected` is what
    /// [`read_file_info`](Self::read_file_info) returned; `None` means the file
    /// must not exist yet. The host backs the old file up first.
    fn write_file(
        &self,
        _path: &str,
        _bytes: &[u8],
        _expected: Option<&FileInfo>,
    ) -> Result<(), PluginError> {
        Err(PluginError::PermissionDenied)
    }
    fn fetch(&self, url: &str) -> Result<FetchResponse, PluginError>;
    /// HTTP requests still pass through the host's permissions and transport.
    /// Existing contexts support a plain GET only until they implement this.
    fn request(&self, request: FetchRequest) -> Result<FetchResponse, PluginError> {
        if request.method == FetchMethod::Get
            && request.headers.is_empty()
            && request.body.is_none()
        {
            self.fetch(&request.url)
        } else {
            Err(PluginError::PermissionDenied)
        }
    }
}

pub trait Plugin: Send + Sync + 'static {
    fn base_map_provider(&self) -> Option<&dyn map::BaseMapProvider> {
        None
    }
    fn overlay_provider(&self) -> Option<&dyn map::OverlayProvider> {
        None
    }
    fn manifest(&self) -> Manifest;
    fn analyzer(&self) -> Option<&dyn Analyzer> {
        None
    }
    fn instance_tab(&self) -> Option<&dyn InstanceTab> {
        None
    }
    fn content_source(&self) -> Option<&dyn ContentSource> {
        None
    }
    fn launch_observer(&self) -> Option<&dyn LaunchObserver> {
        None
    }
}
