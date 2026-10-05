use lumilio_plugin_api::PluginError;
use lumilio_plugin_api::content::{Environment, ProjectKind, SideSupport, Sort};
use serde::Deserialize;

pub(crate) fn decode<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, PluginError> {
    serde_json::from_slice(bytes)
        .map_err(|error| PluginError::Unavailable(format!("unexpected Modrinth answer: {error}")))
}

pub(crate) trait Kind {
    fn protocol_name(self) -> &'static str;
    fn from_protocol(name: &str) -> Option<ProjectKind>;
}

impl Kind for ProjectKind {
    fn protocol_name(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::ResourcePack => "resourcepack",
            Self::Shader => "shader",
        }
    }

    fn from_protocol(name: &str) -> Option<Self> {
        match name {
            "mod" => Some(Self::Mod),
            "modpack" => Some(Self::Modpack),
            "resourcepack" => Some(Self::ResourcePack),
            "shader" => Some(Self::Shader),
            _ => None,
        }
    }
}

pub(crate) trait Sorting {
    fn protocol_name(self) -> &'static str;
}

impl Sorting for Sort {
    fn protocol_name(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Follows => "follows",
            Self::Newest => "newest",
            Self::Updated => "updated",
            // Search validates capabilities before encoding. These sorts are
            // part of the shared contract but are not offered by Modrinth.
            Self::Name => "title",
            Self::Author => "author",
        }
    }
}

pub(crate) fn side(name: Option<&str>) -> SideSupport {
    match name {
        Some("required") => SideSupport::Required,
        Some("optional") => SideSupport::Optional,
        Some("unsupported") => SideSupport::Unsupported,
        _ => SideSupport::Unknown,
    }
}

pub(crate) fn environment(name: &str) -> Option<Environment> {
    Some(match name {
        "client_or_server" | "client_or_server_prefers_both" => Environment::ClientOrServer,
        "client_and_server" => Environment::ClientAndServer,
        "client_only" | "client_only_server_optional" => Environment::ClientOnly,
        "server_only" | "server_only_client_optional" => Environment::ServerOnly,
        "singleplayer_only" => Environment::SingleplayerOnly,
        "dedicated_server_only" => Environment::DedicatedServerOnly,
        _ => return None,
    })
}
