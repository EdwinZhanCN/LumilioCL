//! The content-only view tree and the Elm-style `InstanceTab` extension point.
//! Components say what is shown, never how it is drawn.

use serde::{Deserialize, Serialize};

use crate::{GameFacts, HostContext, PluginError, Words};

/// Per-instance, per-plugin UI state. The host stores it; plugins do not.
pub type TabState = serde_json::Value;

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ActionId(pub String);

impl ActionId {
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum Tone {
    #[default]
    Body,
    Secondary,
    Mono,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum KeyKind {
    #[default]
    Primary,
    Ghost,
}

/// RGBA pixels; the host scales them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ImageData {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && u64::from(self.width) * u64::from(self.height) * 4 == self.rgba.len() as u64
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ListItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub value: Option<String>,
    pub image: Option<ImageData>,
    pub tags: Vec<String>,
    pub open: Option<ActionId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum View {
    Section {
        title: String,
        children: Vec<View>,
    },
    List {
        items: Vec<ListItem>,
    },
    Detail {
        title: String,
        subtitle: Option<String>,
        image: Option<ImageData>,
        facts: Vec<(String, String)>,
        children: Vec<View>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Empty {
        title: String,
        message: String,
    },
    Text {
        text: String,
        tone: Tone,
    },
    Tags(Vec<String>),
    Image(ImageData),
    /// A 3D model of a file under one of the plugin's `ReadGameFiles` grants
    /// (a path relative to the game directory). The host reads the file and
    /// decides how and where to show it; the plugin never touches the viewer.
    Model {
        file: String,
    },
    Key {
        id: ActionId,
        label: String,
        kind: KeyKind,
        /// The host asks for confirmation before sending the action.
        destructive: bool,
    },
}

/// What a plugin asks the host to do for it; the plugin never touches the
/// disk or the system interface itself.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Effect {
    /// Show a file in the file manager; must lie under a `ReadGameFiles` grant.
    RevealGameFile {
        path: String,
    },
    /// The host asks where to save; the user's choice is the consent.
    SaveAs {
        suggested_name: String,
        bytes: Vec<u8>,
    },
    Toast(String),
}

pub trait InstanceTab: Send + Sync {
    fn title(&self) -> Words;
    /// Whether the tab shows for an instance with these facts.
    fn appears(&self, game: &GameFacts, ctx: &dyn HostContext) -> bool;
    fn view(&self, ctx: &dyn HostContext, state: &TabState) -> Result<View, PluginError>;
    fn update(
        &self,
        ctx: &dyn HostContext,
        state: TabState,
        action: ActionId,
    ) -> Result<(TabState, Vec<Effect>), PluginError>;
}
