use super::SITE_BASE;
use crate::instance::Loader;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Modpack,
    Mod,
    ResourcePack,
    Shader,
}

impl ProjectKind {
    /// The name Modrinth uses for this kind in queries and page addresses.
    #[must_use]
    pub const fn protocol_name(self) -> &'static str {
        match self {
            Self::Modpack => "modpack",
            Self::Mod => "mod",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
        }
    }

    #[must_use]
    pub fn from_protocol(name: &str) -> Option<Self> {
        match name {
            "modpack" => Some(Self::Modpack),
            "mod" => Some(Self::Mod),
            "resourcepack" => Some(Self::ResourcePack),
            "shader" => Some(Self::Shader),
            _ => None,
        }
    }

    /// The folder inside an instance's game directory that holds this kind of
    /// file; modpacks are not dropped into a folder.
    #[must_use]
    pub const fn install_folder(self) -> Option<&'static str> {
        match self {
            Self::Mod => Some("mods"),
            Self::ResourcePack => Some("resourcepacks"),
            Self::Shader => Some("shaderpacks"),
            Self::Modpack => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl SortIndex {
    pub(super) const fn protocol_name(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
        }
    }
}

/// The public web page of a project — what the webview shows.
#[must_use]
pub fn project_page_url(kind: ProjectKind, slug_or_id: &str) -> String {
    format!("{SITE_BASE}/{}/{slug_or_id}", kind.protocol_name())
}

/// The public listing of every project of a kind, e.g. `…/mods`.
#[must_use]
pub fn browse_page_url(kind: ProjectKind) -> String {
    format!("{SITE_BASE}/{}s", kind.protocol_name())
}

pub(super) fn loader_tag(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::Vanilla => None,
        Loader::Fabric => Some("fabric"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
        Loader::Quilt => Some("quilt"),
    }
}
