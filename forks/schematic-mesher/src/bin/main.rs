//! Schematic Mesher CLI
//!
//! Generate 3D meshes from Minecraft block data.

use clap::{Parser, Subcommand, ValueEnum};
use schematic_mesher::{
    export_glb, load_resource_pack, Mesher, MesherConfig, ObjExport,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "schematic-mesher")]
#[command(author, version, about = "Generate 3D meshes from Minecraft block data", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Mesh a single block (useful for testing)
    Block {
        /// Block name (e.g., "minecraft:stone" or "stone")
        #[arg(short, long)]
        block: String,

        /// Block properties as key=value pairs (e.g., "facing=north")
        #[arg(short, long, value_parser = parse_property)]
        property: Vec<(String, String)>,

        /// Path to resource pack (ZIP or directory)
        #[arg(short, long)]
        resource_pack: PathBuf,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value = "glb")]
        format: OutputFormat,
    },

    /// Mesh blocks from a JSON input file
    Mesh {
        /// Input JSON file containing block data
        #[arg(short, long)]
        input: PathBuf,

        /// Path to resource pack (ZIP or directory)
        #[arg(short, long)]
        resource_pack: PathBuf,

        /// Output file path (without extension)
        #[arg(short, long)]
        output: PathBuf,

        /// Output format
        #[arg(short, long, value_enum, default_value = "glb")]
        format: OutputFormat,

        /// Disable face culling
        #[arg(long)]
        no_culling: bool,

        /// Disable ambient occlusion
        #[arg(long)]
        no_ao: bool,

        /// AO intensity (0.0 to 1.0)
        #[arg(long, default_value = "0.4")]
        ao_intensity: f32,

        /// Maximum atlas size
        #[arg(long, default_value = "4096")]
        atlas_size: u32,

        /// Biome for tinting (e.g., "plains", "swamp", "jungle")
        #[arg(long)]
        biome: Option<String>,
    },

    /// Show information about a resource pack
    Info {
        /// Path to resource pack (ZIP or directory)
        #[arg(short, long)]
        resource_pack: PathBuf,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    /// Binary glTF format
    Glb,
    /// Wavefront OBJ format
    Obj,
}

fn parse_property(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid property format: '{}'. Use key=value", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Block {
            block,
            property,
            resource_pack,
            output,
            format,
        } => {
            mesh_single_block(&block, property, &resource_pack, &output, format)?;
        }
        Commands::Mesh {
            input,
            resource_pack,
            output,
            format,
            no_culling,
            no_ao,
            ao_intensity,
            atlas_size,
            biome,
        } => {
            mesh_from_json(
                &input,
                &resource_pack,
                &output,
                format,
                !no_culling,
                !no_ao,
                ao_intensity,
                atlas_size,
                biome,
            )?;
        }
        Commands::Info { resource_pack } => {
            show_pack_info(&resource_pack)?;
        }
    }

    Ok(())
}

fn mesh_single_block(
    block_name: &str,
    properties: Vec<(String, String)>,
    resource_pack_path: &PathBuf,
    output_path: &PathBuf,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading resource pack from {:?}...", resource_pack_path);
    let pack = load_resource_pack(resource_pack_path)?;
    println!("  Found {} blockstates", pack.blockstate_count());

    // Normalize block name
    let block_name = if block_name.contains(':') {
        block_name.to_string()
    } else {
        format!("minecraft:{}", block_name)
    };

    // Create the block
    let mut block = schematic_mesher::types::InputBlock::new(&block_name);
    for (key, value) in properties {
        block.properties.insert(key, value);
    }

    println!("Meshing block: {} {:?}", block_name, block.properties);

    // Create a simple block source with just this block
    let blocks = SimpleBlockSource::single(block);

    let mesher = Mesher::new(pack);
    let output = mesher.mesh(&blocks)?;

    println!(
        "  Generated {} vertices, {} triangles",
        output.total_vertices(),
        output.total_triangles()
    );

    export_output(&output, output_path, format, "block")?;

    Ok(())
}

fn mesh_from_json(
    input_path: &PathBuf,
    resource_pack_path: &PathBuf,
    output_path: &PathBuf,
    format: OutputFormat,
    cull_faces: bool,
    ambient_occlusion: bool,
    ao_intensity: f32,
    atlas_size: u32,
    biome: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading resource pack from {:?}...", resource_pack_path);
    let pack = load_resource_pack(resource_pack_path)?;
    println!("  Found {} blockstates", pack.blockstate_count());

    println!("Loading block data from {:?}...", input_path);
    let json_content = fs::read_to_string(input_path)?;
    let block_data: BlockDataInput = serde_json::from_str(&json_content)?;
    println!("  Loaded {} blocks", block_data.blocks.len());

    // Convert to InputBlocks
    let blocks = SimpleBlockSource::from_input(&block_data);

    // Configure mesher
    let mut config = MesherConfig::default();
    config.cull_hidden_faces = cull_faces;
    config.ambient_occlusion = ambient_occlusion;
    config.ao_intensity = ao_intensity;
    config.atlas_max_size = atlas_size;

    if let Some(biome_name) = &biome {
        config = config.with_biome(biome_name);
    }

    println!("Meshing with config:");
    println!("  - Face culling: {}", cull_faces);
    println!("  - Ambient occlusion: {}", ambient_occlusion);
    if ambient_occlusion {
        println!("  - AO intensity: {}", ao_intensity);
    }
    println!("  - Atlas max size: {}", atlas_size);
    if let Some(biome_name) = &biome {
        println!("  - Biome: {}", biome_name);
    }

    let mesher = Mesher::with_config(pack, config);
    let output = mesher.mesh(&blocks)?;

    println!(
        "  Generated {} vertices ({} opaque, {} transparent), {} triangles",
        output.total_vertices(),
        output.opaque_mesh.vertex_count(),
        output.transparent_mesh.vertex_count(),
        output.total_triangles()
    );
    println!(
        "  Atlas: {}x{} with {} regions",
        output.atlas.width,
        output.atlas.height,
        output.atlas.regions.len()
    );

    export_output(&output, output_path, format, "mesh")?;

    Ok(())
}

fn show_pack_info(resource_pack_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("Loading resource pack from {:?}...", resource_pack_path);
    let pack = load_resource_pack(resource_pack_path)?;

    println!("\nResource Pack Info:");
    println!("  Blockstates: {}", pack.blockstate_count());
    println!("  Models: {}", pack.model_count());
    println!("  Textures: {}", pack.texture_count());

    Ok(())
}

fn export_output(
    output: &schematic_mesher::MesherOutput,
    path: &PathBuf,
    format: OutputFormat,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Glb => {
            let glb_path = if path.extension().is_some() {
                path.clone()
            } else {
                path.with_extension("glb")
            };
            let glb_data = export_glb(output)?;
            fs::write(&glb_path, &glb_data)?;
            println!("Exported GLB ({} bytes) to {:?}", glb_data.len(), glb_path);
        }
        OutputFormat::Obj => {
            let obj_export = ObjExport::from_output(output, name)?;

            let obj_path = if path.extension().is_some() {
                path.clone()
            } else {
                path.with_extension("obj")
            };
            let mtl_path = obj_path.with_extension("mtl");
            let png_path = obj_path.with_file_name(format!("{}_atlas.png", name));

            fs::write(&obj_path, &obj_export.obj)?;
            fs::write(&mtl_path, &obj_export.mtl)?;
            fs::write(&png_path, &obj_export.texture_png)?;

            // Write greedy material texture files
            let parent = obj_path.parent().unwrap_or(std::path::Path::new("."));
            for tex in &obj_export.greedy_textures {
                let tex_path = parent.join(&tex.filename);
                fs::write(&tex_path, &tex.png_data)?;
                println!("  Greedy texture: {:?}", tex_path);
            }

            println!("Exported OBJ to {:?}", obj_path);
            println!("  Material: {:?}", mtl_path);
            println!("  Texture: {:?}", png_path);
        }
    }

    Ok(())
}

// JSON input format
#[derive(serde::Deserialize)]
struct BlockDataInput {
    blocks: Vec<BlockEntry>,
}

#[derive(serde::Deserialize)]
struct BlockEntry {
    x: i32,
    y: i32,
    z: i32,
    #[serde(default = "default_block_name")]
    name: String,
    #[serde(default)]
    properties: HashMap<String, String>,
}

fn default_block_name() -> String {
    "minecraft:stone".to_string()
}

// Simple block source implementation
struct SimpleBlockSource {
    blocks: HashMap<schematic_mesher::types::BlockPosition, schematic_mesher::types::InputBlock>,
    bounds: schematic_mesher::types::BoundingBox,
}

impl SimpleBlockSource {
    fn single(block: schematic_mesher::types::InputBlock) -> Self {
        use schematic_mesher::types::{BlockPosition, BoundingBox};
        let mut blocks = HashMap::new();
        blocks.insert(BlockPosition::new(0, 0, 0), block);
        Self {
            blocks,
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        }
    }

    fn from_input(input: &BlockDataInput) -> Self {
        use schematic_mesher::types::{BlockPosition, BoundingBox, InputBlock};

        let mut blocks = HashMap::new();
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];

        for entry in &input.blocks {
            let pos = BlockPosition::new(entry.x, entry.y, entry.z);

            // Normalize block name
            let name = if entry.name.contains(':') {
                entry.name.clone()
            } else {
                format!("minecraft:{}", entry.name)
            };

            let mut block = InputBlock::new(name);
            for (key, value) in &entry.properties {
                block.properties.insert(key.clone(), value.clone());
            }

            blocks.insert(pos, block);

            min[0] = min[0].min(entry.x);
            min[1] = min[1].min(entry.y);
            min[2] = min[2].min(entry.z);
            max[0] = max[0].max(entry.x);
            max[1] = max[1].max(entry.y);
            max[2] = max[2].max(entry.z);
        }

        let bounds = if blocks.is_empty() {
            BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0])
        } else {
            BoundingBox::new(
                [min[0] as f32, min[1] as f32, min[2] as f32],
                [(max[0] + 1) as f32, (max[1] + 1) as f32, (max[2] + 1) as f32],
            )
        };

        Self { blocks, bounds }
    }
}

impl schematic_mesher::types::BlockSource for SimpleBlockSource {
    fn get_block(
        &self,
        pos: schematic_mesher::types::BlockPosition,
    ) -> Option<&schematic_mesher::types::InputBlock> {
        self.blocks.get(&pos)
    }

    fn iter_blocks(
        &self,
    ) -> Box<
        dyn Iterator<Item = (schematic_mesher::types::BlockPosition, &schematic_mesher::types::InputBlock)>
            + '_,
    > {
        Box::new(self.blocks.iter().map(|(k, v)| (*k, v)))
    }

    fn bounds(&self) -> schematic_mesher::types::BoundingBox {
        self.bounds
    }
}
