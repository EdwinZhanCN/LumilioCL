//! Synchronous, UI-independent contracts for the launcher's built-in plugins.
//! The host supplies all I/O and invokes plugin code on a worker thread.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

mod analysis;
pub mod content;
mod launch;
mod network;
mod view;
pub use analysis::{
    AnalysisInput, AnalysisSource, Analyzer, Finding, GameFacts, ModFact, Severity,
};
pub use content::ContentSource;
pub use launch::{DiscordActivity, LaunchEvent, LaunchObserver, LaunchOutcome, LaunchTarget};
pub use network::{FetchMethod, FetchRequest};
pub use view::{ActionId, Effect, ImageData, InstanceTab, KeyKind, ListItem, TabState, Tone, View};

pub const API_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub api: u32,
    pub default_enabled: bool,
    pub permissions: Vec<Permission>,
    pub settings: Vec<SettingField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Permission {
    ReadGameFiles { under: String },
    Network { hosts: Vec<String> },
    LaunchEvents,
    Native(NativeCapability),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NativeCapability {
    DiscordIpc,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SettingField {
    pub key: String,
    pub label: String,
    pub help: String,
    pub kind: SettingKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SettingKind {
    Toggle {
        default: bool,
    },
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
