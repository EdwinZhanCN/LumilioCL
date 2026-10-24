use serde::{Deserialize, Serialize};

use crate::instances::Loader;

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

/// Whose project and version IDs a content source speaks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Catalog {
    Modrinth,
    CurseForge,
}

/// How a source writes project bodies and changelogs.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextFormat {
    #[default]
    Markdown,
    Html,
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
