# Schematic Mesher

A Rust library for generating 3D meshes from Minecraft block data. Takes blocks and a resource pack as input, outputs GLB/glTF meshes with texture atlases.

## Features

- Generate triangle meshes from Minecraft blocks
- Automatic texture atlas generation
- Face culling between adjacent opaque blocks
- Transparency handling (separate opaque/transparent geometry)
- Biome-aware tinting (grass, foliage, water, redstone)
- Ambient occlusion
- Greedy meshing (merge coplanar faces for lower triangle counts)
- Occlusion culling (skip fully hidden blocks)
- Multiple output formats: GLB, OBJ, USDZ, raw mesh data
- WASM support (optional)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
schematic-mesher = { git = "https://github.com/your-username/schematic-mesher" }
```

For CLI usage:

```toml
[dependencies]
schematic-mesher = { git = "...", features = ["cli"] }
```

## CLI Usage

Build and install with the `cli` feature:

```bash
cargo install --path . --features cli
```

### Commands

**Mesh a single block (for testing):**

```bash
schematic-mesher block \
    --resource-pack path/to/pack.zip \
    --block minecraft:stone \
    --output stone.glb
```

With block properties:

```bash
schematic-mesher block \
    --resource-pack pack.zip \
    --block minecraft:oak_stairs \
    --properties "facing=north,half=bottom,shape=straight" \
    --output stairs.glb
```

**Mesh blocks from JSON input:**

```bash
schematic-mesher mesh \
    --resource-pack pack.zip \
    --input blocks.json \
    --output scene.glb
```

Input JSON format:

```json
{
    "bounds": {
        "min": [0, 0, 0],
        "max": [16, 16, 16]
    },
    "blocks": [
        {
            "position": [0, 0, 0],
            "name": "minecraft:stone",
            "properties": {}
        },
        {
            "position": [1, 0, 0],
            "name": "minecraft:grass_block",
            "properties": { "snowy": "false" }
        }
    ]
}
```

**Show resource pack info:**

```bash
schematic-mesher info --resource-pack pack.zip
```

### CLI Options

| Option | Description |
|--------|-------------|
| `--format` | Output format: `glb` (default), `obj`, `usdz` |
| `--biome` | Biome for tinting: `plains`, `forest`, `swamp`, etc. |
| `--no-cull` | Disable face culling |
| `--no-ao` | Disable ambient occlusion |

## Library Usage

### Basic Example

```rust
use schematic_mesher::{
    load_resource_pack, Mesher, MesherConfig,
    export_glb, InputBlock, BlockPosition, BoundingBox,
};

fn main() -> schematic_mesher::Result<()> {
    // Load a resource pack
    let pack = load_resource_pack("path/to/pack.zip")?;

    // Create a mesher with default config
    let mesher = Mesher::new(pack);

    // Define some blocks
    let blocks = vec![
        (BlockPosition::new(0, 0, 0), InputBlock::new("minecraft:stone")),
        (BlockPosition::new(1, 0, 0), InputBlock::new("minecraft:dirt")),
        (BlockPosition::new(0, 1, 0), InputBlock::new("minecraft:grass_block")),
    ];

    let bounds = BoundingBox::new([0, 0, 0], [2, 2, 1]);

    // Generate the mesh
    let output = mesher.mesh_blocks(
        blocks.iter().map(|(pos, block)| (*pos, block)),
        bounds,
    )?;

    // Export to GLB
    let glb_bytes = export_glb(&output)?;
    std::fs::write("output.glb", glb_bytes)?;

    Ok(())
}
```

### Using BlockSource Trait

For larger datasets, implement the `BlockSource` trait:

```rust
use schematic_mesher::{BlockSource, BlockPosition, BoundingBox, InputBlock};

struct MySchematic {
    blocks: Vec<(BlockPosition, InputBlock)>,
    bounds: BoundingBox,
}

impl BlockSource for MySchematic {
    fn bounds(&self) -> BoundingBox {
        self.bounds
    }

    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> {
        self.blocks.iter()
            .find(|(p, _)| *p == pos)
            .map(|(_, b)| b)
    }

    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }
}

// Then use with mesher.mesh(&my_schematic)
```

### Configuration

```rust
use schematic_mesher::{Mesher, MesherConfig, TintProvider};

let config = MesherConfig {
    cull_hidden_faces: true,      // Remove faces between adjacent blocks
    cull_occluded_blocks: true,   // Skip blocks with all 6 neighbors opaque
    greedy_meshing: false,        // Merge coplanar faces into larger quads
    atlas_max_size: 4096,         // Max texture atlas dimension
    atlas_padding: 1,             // Padding between atlas textures
    include_air: false,           // Skip air blocks
    ambient_occlusion: true,      // Enable AO
    ao_intensity: 0.4,            // AO darkness (0.0-1.0)
    tint_provider: TintProvider::for_biome("plains"),
};

let mesher = Mesher::with_config(pack, config);
```

### Working with MesherOutput

The mesher returns separate opaque and transparent meshes for correct rendering:

```rust
let output = mesher.mesh(&source)?;

// For renderers that support transparency sorting
let opaque_mesh = &output.opaque_mesh;      // Render first
let transparent_mesh = &output.transparent_mesh;  // Render second

// Or get combined mesh (for simpler use cases)
let combined = output.mesh();

// Access the texture atlas
let atlas = &output.atlas;
let png_bytes = atlas.to_png()?;

// Mesh statistics
println!("Vertices: {}", output.total_vertices());
println!("Triangles: {}", output.total_triangles());
println!("Has transparency: {}", output.has_transparency());
```

### Export Formats

**GLB (recommended):**

```rust
use schematic_mesher::export_glb;

let glb_bytes = export_glb(&output)?;
```

**OBJ + MTL:**

```rust
use schematic_mesher::{export_obj, ObjExport};

// Get strings
let (obj_content, mtl_content) = export_obj(&output, "my_mesh")?;

// Or get everything including PNG texture
let export = ObjExport::from_output(&output, "my_mesh")?;
std::fs::write("mesh.obj", &export.obj)?;
std::fs::write("mesh.mtl", &export.mtl)?;
std::fs::write("mesh_atlas.png", &export.texture_png)?;
```

**USDZ (Apple ecosystem / AR Quick Look):**

```rust
use schematic_mesher::export_usdz;

let usdz_bytes = export_usdz(&output)?;
std::fs::write("mesh.usdz", usdz_bytes)?;
```

**USDA (human-readable USD text + textures):**

```rust
use schematic_mesher::export_usda;

let export = export_usda(&output)?;
std::fs::write("mesh.usda", &export.usda)?;
std::fs::write("textures/atlas.png", &export.atlas_png)?;
for tex in &export.greedy_textures {
    std::fs::write(&tex.filename, &tex.png_data)?;
}
```

**Raw mesh data:**

```rust
use schematic_mesher::export_raw;

let raw = export_raw(&output);

// Access vertex data
let positions: &[[f32; 3]] = &raw.positions;
let normals: &[[f32; 3]] = &raw.normals;
let uvs: &[[f32; 2]] = &raw.uvs;
let colors: &[[f32; 4]] = &raw.colors;
let indices: &[u32] = &raw.indices;

// Or as flat arrays for GPU upload
let pos_flat: Vec<f32> = raw.positions_flat();
let norm_flat: Vec<f32> = raw.normals_flat();
```

## Integration with Nucleation

For integration with [Nucleation](https://github.com/your-username/nucleation) schematic library:

```rust
use nucleation::UniversalSchematic;
use schematic_mesher::{
    load_resource_pack, Mesher, InputBlock, BlockPosition, BoundingBox,
};

fn mesh_schematic(schematic: &UniversalSchematic, pack_path: &str) -> schematic_mesher::Result<Vec<u8>> {
    let pack = load_resource_pack(pack_path)?;
    let mesher = Mesher::new(pack);

    // Convert schematic blocks to InputBlock
    let mut blocks = Vec::new();
    for region in schematic.regions() {
        for (pos, block_state) in region.iter_blocks() {
            let input_block = InputBlock {
                name: block_state.name.clone(),
                properties: block_state.properties.clone(),
            };
            blocks.push((BlockPosition::new(pos.x, pos.y, pos.z), input_block));
        }
    }

    let bounds = BoundingBox::new(
        [0, 0, 0],
        [schematic.width() as i32, schematic.height() as i32, schematic.length() as i32],
    );

    let output = mesher.mesh_blocks(
        blocks.iter().map(|(p, b)| (*p, b)),
        bounds,
    )?;

    export_glb(&output)
}
```

### Chunk-based Meshing

For large schematics, mesh in chunks to manage memory:

```rust
const CHUNK_SIZE: i32 = 16;

fn mesh_in_chunks(schematic: &UniversalSchematic, mesher: &Mesher) -> Vec<MesherOutput> {
    let mut outputs = Vec::new();

    let width = schematic.width() as i32;
    let height = schematic.height() as i32;
    let length = schematic.length() as i32;

    for cx in (0..width).step_by(CHUNK_SIZE as usize) {
        for cy in (0..height).step_by(CHUNK_SIZE as usize) {
            for cz in (0..length).step_by(CHUNK_SIZE as usize) {
                let chunk_bounds = BoundingBox::new(
                    [cx, cy, cz],
                    [
                        (cx + CHUNK_SIZE).min(width),
                        (cy + CHUNK_SIZE).min(height),
                        (cz + CHUNK_SIZE).min(length),
                    ],
                );

                // Collect blocks in this chunk
                let chunk_blocks: Vec<_> = /* filter blocks in chunk_bounds */;

                if !chunk_blocks.is_empty() {
                    let output = mesher.mesh_blocks(
                        chunk_blocks.iter().map(|(p, b)| (*p, b)),
                        chunk_bounds,
                    ).unwrap();
                    outputs.push(output);
                }
            }
        }
    }

    outputs
}
```

## Resource Pack Requirements

The resource pack should be a standard Minecraft Java Edition resource pack containing:

```
assets/
  minecraft/
    blockstates/     # Block state JSON files
    models/
      block/         # Block model JSON files
    textures/
      block/         # Block texture PNG files
```

Both ZIP files and extracted directories are supported.

## Supported Block Features

- Standard cube blocks
- Rotated/oriented blocks (stairs, logs, etc.)
- Multi-part blocks (fences, walls, redstone)
- Transparent blocks (glass, ice, slime)
- Tinted blocks (grass, leaves, water, redstone)
- Custom models with arbitrary elements

## Limitations

- Animated textures use first frame only
- No entity models (chests, signs, banners)
- No fluid rendering (water/lava flow shapes)
- No custom block entity rendering

## License

AGPL-3.0-only
