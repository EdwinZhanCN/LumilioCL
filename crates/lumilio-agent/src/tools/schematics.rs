//! Tools over an instance's `.litematic` files, read through the Litematica
//! plugin's grants: an agent sees exactly the files the studio sees, and
//! nothing outside the `schematics` folder.

use lumilio_core::Transport;
use lumilio_schematic::{metadata, stats};
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::failed;
use super::instances::InstanceId;
use crate::LauncherMcp;

/// Where Litematica keeps schematics, below the game directory.
const FOLDER: &str = "schematics";
const EXTENSION: &str = ".litematic";

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SchematicRef {
    /// An instance id from `list_instances`.
    pub instance: String,
    /// A path from `list_schematics`, relative to the instance's game directory.
    pub file: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Schematics {
    pub instance: String,
    /// Paths relative to the game directory, sorted.
    pub files: Vec<String>,
}

/// What one schematic is made of.
#[derive(Debug, Serialize, JsonSchema)]
pub struct Schematic {
    pub file: String,
    pub name: String,
    pub author: String,
    pub description: String,
    /// The box enclosing every region, in blocks: x, y, z.
    pub size: [i32; 3],
    /// Cells in that box.
    pub volume: u64,
    /// Non-air blocks.
    pub block_count: u64,
    /// The Minecraft data version the content is at, when the file names one.
    pub data_version: Option<i32>,
    pub regions: Vec<Region>,
    /// Non-air blocks by id, most frequent first.
    pub blocks: Vec<Count>,
    pub entities: Vec<Count>,
    pub block_entities: Vec<Count>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Region {
    pub name: String,
    /// The region's lowest corner: x, y, z.
    pub origin: [i32; 3],
    pub size: [i32; 3],
    /// Non-air blocks.
    pub block_count: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct Count {
    /// A namespaced id such as `minecraft:redstone_wire`.
    pub id: String,
    pub count: u64,
}

#[tool_router(router = schematic_tools, vis = "pub(super)")]
impl<T: Transport + Clone> LauncherMcp<T> {
    #[tool(
        name = "list_schematics",
        description = "Lists the .litematic files in an instance's schematics folder."
    )]
    async fn list_schematics(
        &self,
        Parameters(InstanceId { instance }): Parameters<InstanceId>,
    ) -> Result<Json<Schematics>, String> {
        let files = self
            .service()
            .schematic_files(&instance, lumilio_plugin_litematica::ID, FOLDER)
            .await
            .map_err(failed)?;
        let files = files
            .into_iter()
            .filter(|file| file.ends_with(EXTENSION))
            .collect();
        Ok(Json(Schematics { instance, files }))
    }

    #[tool(
        name = "read_schematic",
        description = "Reads one .litematic file: its metadata, size, regions, and the blocks, entities and block entities it contains, by count."
    )]
    async fn read_schematic(
        &self,
        Parameters(SchematicRef { instance, file }): Parameters<SchematicRef>,
    ) -> Result<Json<Schematic>, String> {
        let (bytes, _info) = self
            .service()
            .schematic_read(&instance, lumilio_plugin_litematica::ID, &file)
            .await
            .map_err(failed)?;
        // Decoding a large schematic is CPU work; keep it off the protocol task.
        let schematic = tokio::task::spawn_blocking(move || summarize(file, &bytes))
            .await
            .map_err(failed)?
            .map_err(failed)?;
        Ok(Json(schematic))
    }
}

fn summarize(file: String, bytes: &[u8]) -> Result<Schematic, lumilio_schematic::Error> {
    let schematic = lumilio_schematic::load_litematic(bytes)?;
    let info = metadata::read(&schematic);
    let stats = stats::stats(&schematic);
    Ok(Schematic {
        file,
        name: info.name,
        author: info.author,
        description: info.description,
        size: triple(stats.size),
        volume: stats.volume,
        block_count: stats.block_count,
        data_version: info.data_version,
        regions: stats
            .regions
            .iter()
            .map(|region| Region {
                name: region.name.clone(),
                origin: triple(region.origin),
                size: triple(region.size),
                block_count: region.block_count,
            })
            .collect(),
        blocks: stats
            .blocks
            .iter()
            .map(|block| Count {
                id: block.id.clone(),
                count: block.count,
            })
            .collect(),
        entities: counts(&stats.entities),
        block_entities: counts(&stats.block_entities),
    })
}

fn counts(types: &[stats::TypeCount]) -> Vec<Count> {
    types
        .iter()
        .map(|kind| Count {
            id: kind.id.clone(),
            count: kind.count,
        })
        .collect()
}

fn triple((x, y, z): (i32, i32, i32)) -> [i32; 3] {
    [x, y, z]
}
