//! Tools over the library: which games exist and what each one is.

use lumilio_core::{InstanceRecord, Loader, Transport};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::failed;
use crate::LauncherMcp;

/// One game instance as an agent sees it.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub game_version: String,
    /// `vanilla`, `fabric`, `forge`, `neoforge` or `quilt`.
    pub loader: String,
    pub loader_version: Option<String>,
    /// The game files for this version are installed, so it can launch.
    pub installed: bool,
    pub favorite: bool,
    /// Seconds since the Unix epoch; absent when never played.
    pub last_played: Option<u64>,
    pub play_seconds: u64,
    /// The directory the game runs in: mods, saves and schematics live below it.
    pub game_directory: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Instances {
    pub instances: Vec<Instance>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InstanceId {
    /// An instance id from `list_instances`.
    pub instance: String,
}

#[tool_router(router = instance_tools, vis = "pub(super)")]
impl<T: Transport + Clone> LauncherMcp<T> {
    #[tool(
        name = "list_instances",
        description = "Lists every game instance in the launcher's library: id, name, game version, mod loader and where its files are."
    )]
    async fn list_instances(&self) -> Json<Instances> {
        let library = self.service().library().await;
        let instances = library
            .instances
            .iter()
            .map(|record| self.describe(record))
            .collect();
        Json(Instances { instances })
    }

    #[tool(
        name = "get_instance",
        description = "One game instance by id, with the same fields as list_instances."
    )]
    async fn get_instance(
        &self,
        Parameters(InstanceId { instance }): Parameters<InstanceId>,
    ) -> Result<Json<Instance>, String> {
        let record = self.service().instance(&instance).await.map_err(failed)?;
        Ok(Json(self.describe(&record)))
    }
}

impl<T: Transport + Clone> LauncherMcp<T> {
    fn describe(&self, record: &InstanceRecord) -> Instance {
        Instance {
            id: record.id.clone(),
            name: record.name.clone(),
            game_version: record.game_version.clone(),
            loader: loader_name(record.loader).to_owned(),
            loader_version: record.loader_version.clone(),
            installed: record.installed,
            favorite: record.favorite,
            last_played: record.last_played,
            play_seconds: record.play_seconds,
            game_directory: self
                .service()
                .layout()
                .game(&record.id)
                .display()
                .to_string(),
        }
    }
}

fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::Vanilla => "vanilla",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::NeoForge => "neoforge",
        Loader::Quilt => "quilt",
    }
}
