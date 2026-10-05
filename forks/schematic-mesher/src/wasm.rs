//! WebAssembly bindings for schematic-mesher.
//!
//! This module exposes the mesher to JavaScript via `wasm-bindgen`. It provides:
//!
//! - **[`ResourcePackHandle`]** — Load and inspect a resource pack from a ZIP `Uint8Array`.
//! - **[`MesherOptions`]** — Configure face culling, AO, atlas size, and biome tinting.
//! - **[`mesh_block()`]** / **[`mesh_blocks_json()`]** — One-shot meshing returning GLB bytes.
//! - **[`MeshOutputWrapper`]** — Zero-copy per-layer typed array access (`Float32Array` /
//!   `Uint32Array`) for direct WebGL/WebGPU upload.
//! - **[`mesh_chunks_json()`]** / **[`ChunkMeshIteratorWrapper`]** — Lazy chunk-by-chunk
//!   meshing for large worlds.
//!
//! ## Usage from JavaScript
//!
//! ```js
//! import init, { ResourcePackHandle, mesh_block } from './schematic_mesher.js';
//!
//! await init();
//! const pack = new ResourcePackHandle(zipBytes);
//! const result = mesh_block(pack, "stone");
//! const glb = result.glb_data; // Uint8Array
//! ```

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    // Set up better panic messages in the browser console
    console_error_panic_hook::set_once();
}

/// Load a resource pack from bytes and return a handle.
#[wasm_bindgen]
pub struct ResourcePackHandle {
    inner: crate::ResourcePack,
}

#[wasm_bindgen]
impl ResourcePackHandle {
    /// Load a resource pack from a ZIP file's bytes.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8]) -> Result<ResourcePackHandle, JsError> {
        let pack = crate::load_resource_pack_from_bytes(data)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(ResourcePackHandle { inner: pack })
    }

    /// Get the number of blockstates in the pack.
    #[wasm_bindgen(getter)]
    pub fn blockstate_count(&self) -> usize {
        self.inner.blockstate_count()
    }

    /// Get the number of models in the pack.
    #[wasm_bindgen(getter)]
    pub fn model_count(&self) -> usize {
        self.inner.model_count()
    }

    /// Get the number of textures in the pack.
    #[wasm_bindgen(getter)]
    pub fn texture_count(&self) -> usize {
        self.inner.texture_count()
    }
}

/// Mesher configuration options.
#[wasm_bindgen]
#[derive(Default)]
pub struct MesherOptions {
    cull_hidden_faces: bool,
    ambient_occlusion: bool,
    ao_intensity: f32,
    atlas_max_size: u32,
    biome: Option<String>,
}

#[wasm_bindgen]
impl MesherOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> MesherOptions {
        MesherOptions {
            cull_hidden_faces: true,
            ambient_occlusion: true,
            ao_intensity: 0.4,
            atlas_max_size: 4096,
            biome: None,
        }
    }

    #[wasm_bindgen(setter)]
    pub fn set_cull_hidden_faces(&mut self, value: bool) {
        self.cull_hidden_faces = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_ambient_occlusion(&mut self, value: bool) {
        self.ambient_occlusion = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_ao_intensity(&mut self, value: f32) {
        self.ao_intensity = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_atlas_max_size(&mut self, value: u32) {
        self.atlas_max_size = value;
    }

    #[wasm_bindgen(setter)]
    pub fn set_biome(&mut self, value: String) {
        self.biome = Some(value);
    }
}

/// A block to be meshed.
#[wasm_bindgen]
pub struct BlockInput {
    x: i32,
    y: i32,
    z: i32,
    name: String,
    properties: std::collections::HashMap<String, String>,
}

#[wasm_bindgen]
impl BlockInput {
    #[wasm_bindgen(constructor)]
    pub fn new(x: i32, y: i32, z: i32, name: &str) -> BlockInput {
        BlockInput {
            x,
            y,
            z,
            name: name.to_string(),
            properties: std::collections::HashMap::new(),
        }
    }

    /// Add a property to this block.
    pub fn set_property(&mut self, key: &str, value: &str) {
        self.properties.insert(key.to_string(), value.to_string());
    }
}

/// Mesh result containing GLB data.
#[wasm_bindgen]
pub struct MeshResult {
    glb_data: Vec<u8>,
    vertex_count: usize,
    triangle_count: usize,
    has_transparency: bool,
}

#[wasm_bindgen]
impl MeshResult {
    /// Get the GLB binary data.
    #[wasm_bindgen(getter)]
    pub fn glb_data(&self) -> Vec<u8> {
        self.glb_data.clone()
    }

    /// Get the total vertex count.
    #[wasm_bindgen(getter)]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get the total triangle count.
    #[wasm_bindgen(getter)]
    pub fn triangle_count(&self) -> usize {
        self.triangle_count
    }

    /// Check if the mesh has transparent geometry.
    #[wasm_bindgen(getter)]
    pub fn has_transparency(&self) -> bool {
        self.has_transparency
    }
}

/// Helper: build a MesherConfig from MesherOptions.
fn build_config(options: &MesherOptions) -> crate::MesherConfig {
    let mut config = crate::MesherConfig::default();
    config.cull_hidden_faces = options.cull_hidden_faces;
    config.ambient_occlusion = options.ambient_occlusion;
    config.ao_intensity = options.ao_intensity;
    config.atlas_max_size = options.atlas_max_size;
    if let Some(biome) = &options.biome {
        config = config.with_biome(biome);
    }
    config
}

/// Mesh a single block and return GLB data.
#[wasm_bindgen]
pub fn mesh_block(
    pack: &ResourcePackHandle,
    block_name: &str,
    options: Option<MesherOptions>,
) -> Result<MeshResult, JsError> {
    let options = options.unwrap_or_default();
    let config = build_config(&options);

    // Normalize block name
    let block_name = if block_name.contains(':') {
        block_name.to_string()
    } else {
        format!("minecraft:{}", block_name)
    };

    let block = crate::InputBlock::new(&block_name);
    let pos = crate::BlockPosition::new(0, 0, 0);

    // Create a simple block source
    let blocks = vec![(pos, block)];
    let bounds = crate::BoundingBox::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);

    let mesher = crate::Mesher::with_config(pack.inner.clone(), config);
    let output = mesher.mesh_blocks(
        blocks.iter().map(|(p, b)| (*p, b)),
        bounds,
    ).map_err(|e| JsError::new(&e.to_string()))?;

    let glb_data = crate::export_glb(&output)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(MeshResult {
        glb_data,
        vertex_count: output.total_vertices(),
        triangle_count: output.total_triangles(),
        has_transparency: output.has_transparency(),
    })
}

/// Mesh multiple blocks from a JSON string and return GLB data.
///
/// JSON format:
/// ```json
/// {
///   "blocks": [
///     { "x": 0, "y": 0, "z": 0, "name": "minecraft:stone", "properties": {} }
///   ]
/// }
/// ```
#[wasm_bindgen]
pub fn mesh_blocks_json(
    pack: &ResourcePackHandle,
    json: &str,
    options: Option<MesherOptions>,
) -> Result<MeshResult, JsError> {
    let (blocks, bounds) = parse_blocks_json(json)?;
    let options = options.unwrap_or_default();
    let config = build_config(&options);

    let mesher = crate::Mesher::with_config(pack.inner.clone(), config);
    let output = mesher.mesh_blocks(
        blocks.iter().map(|(p, b)| (*p, b)),
        bounds,
    ).map_err(|e| JsError::new(&e.to_string()))?;

    let glb_data = crate::export_glb(&output)
        .map_err(|e| JsError::new(&e.to_string()))?;

    Ok(MeshResult {
        glb_data,
        vertex_count: output.total_vertices(),
        triangle_count: output.total_triangles(),
        has_transparency: output.has_transparency(),
    })
}

/// Parse blocks JSON into a Vec of (position, block) pairs and bounding box.
fn parse_blocks_json(json: &str) -> Result<(Vec<(crate::BlockPosition, crate::InputBlock)>, crate::BoundingBox), JsError> {
    #[derive(serde::Deserialize)]
    struct BlockData {
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
        properties: std::collections::HashMap<String, String>,
    }

    fn default_block_name() -> String {
        "minecraft:stone".to_string()
    }

    let data: BlockData = serde_json::from_str(json)
        .map_err(|e| JsError::new(&format!("Invalid JSON: {}", e)))?;

    let mut blocks = Vec::new();
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];

    for entry in &data.blocks {
        let pos = crate::BlockPosition::new(entry.x, entry.y, entry.z);
        let name = if entry.name.contains(':') {
            entry.name.clone()
        } else {
            format!("minecraft:{}", entry.name)
        };
        let mut block = crate::InputBlock::new(name);
        for (k, v) in &entry.properties {
            block.properties.insert(k.clone(), v.clone());
        }
        blocks.push((pos, block));
        min[0] = min[0].min(entry.x);
        min[1] = min[1].min(entry.y);
        min[2] = min[2].min(entry.z);
        max[0] = max[0].max(entry.x);
        max[1] = max[1].max(entry.y);
        max[2] = max[2].max(entry.z);
    }

    let bounds = if blocks.is_empty() {
        crate::BoundingBox::new([0.0; 3], [0.0; 3])
    } else {
        crate::BoundingBox::new(
            [min[0] as f32, min[1] as f32, min[2] as f32],
            [(max[0] + 1) as f32, (max[1] + 1) as f32, (max[2] + 1) as f32],
        )
    };

    Ok((blocks, bounds))
}

// ---------------------------------------------------------------------------
// New canonical MeshOutput wrapper with per-layer typed array access
// ---------------------------------------------------------------------------

/// WASM wrapper around [`MeshOutput`](crate::mesh_output::MeshOutput) providing
/// zero-copy typed array views into each transparency layer's vertex data.
///
/// Each accessor (e.g., `opaque_positions()`) returns a typed array view directly
/// into WASM linear memory — no copies. These views are valid until the
/// `MeshOutputWrapper` is dropped or the WASM memory grows.
///
/// ## Per-Layer Accessors
///
/// Each of the three layers (opaque, cutout, transparent) exposes:
/// - `{layer}_positions()` → `Float32Array` (3 floats/vertex)
/// - `{layer}_normals()` → `Float32Array` (3 floats/vertex)
/// - `{layer}_uvs()` → `Float32Array` (2 floats/vertex)
/// - `{layer}_colors()` → `Float32Array` (4 floats/vertex, RGBA)
/// - `{layer}_indices()` → `Uint32Array`
/// - `{layer}_vertex_count()` → `usize`
/// - `{layer}_triangle_count()` → `usize`
/// - `{layer}_is_empty()` → `bool`
///
/// ## Atlas
///
/// - `atlas_rgba()` → `Uint8Array` (RGBA pixels)
/// - `atlas_width()` / `atlas_height()` → `u32`
///
/// ## Export
///
/// - `to_glb()` → `Uint8Array` (GLB binary)
/// - `to_usdz()` → `Uint8Array` (USDZ archive)
#[wasm_bindgen]
pub struct MeshOutputWrapper {
    inner: crate::mesh_output::MeshOutput,
}

/// Helper: create a `Float32Array` view over raw bytes that are `f32`-aligned.
///
/// # Safety
/// The input bytes must be aligned to 4 bytes and have a length that is a multiple of 4.
/// The returned view is only valid for the lifetime of the underlying data.
fn f32_typed_array(bytes: &[u8]) -> js_sys::Float32Array {
    let f32_slice = unsafe {
        std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4)
    };
    unsafe { js_sys::Float32Array::view(f32_slice) }
}

/// Helper: create a `Uint32Array` view over raw bytes that are `u32`-aligned.
///
/// # Safety
/// The input bytes must be aligned to 4 bytes and have a length that is a multiple of 4.
/// The returned view is only valid for the lifetime of the underlying data.
fn u32_typed_array(bytes: &[u8]) -> js_sys::Uint32Array {
    let u32_slice = unsafe {
        std::slice::from_raw_parts(bytes.as_ptr() as *const u32, bytes.len() / 4)
    };
    unsafe { js_sys::Uint32Array::view(u32_slice) }
}

#[wasm_bindgen]
impl MeshOutputWrapper {
    // --- Opaque layer ---

    /// Opaque layer positions as `Float32Array` (3 floats per vertex).
    pub fn opaque_positions(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.opaque.positions_bytes())
    }

    /// Opaque layer normals as `Float32Array` (3 floats per vertex).
    pub fn opaque_normals(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.opaque.normals_bytes())
    }

    /// Opaque layer UVs as `Float32Array` (2 floats per vertex).
    pub fn opaque_uvs(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.opaque.uvs_bytes())
    }

    /// Opaque layer vertex colors as `Float32Array` (4 floats per vertex, RGBA).
    pub fn opaque_colors(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.opaque.colors_bytes())
    }

    /// Opaque layer triangle indices as `Uint32Array`.
    pub fn opaque_indices(&self) -> js_sys::Uint32Array {
        u32_typed_array(self.inner.opaque.indices_bytes())
    }

    /// Opaque layer vertex count.
    pub fn opaque_vertex_count(&self) -> usize {
        self.inner.opaque.vertex_count()
    }

    /// Opaque layer triangle count.
    pub fn opaque_triangle_count(&self) -> usize {
        self.inner.opaque.triangle_count()
    }

    /// Whether the opaque layer is empty.
    pub fn opaque_is_empty(&self) -> bool {
        self.inner.opaque.is_empty()
    }

    // --- Cutout layer ---

    /// Cutout layer positions as `Float32Array` (3 floats per vertex).
    pub fn cutout_positions(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.cutout.positions_bytes())
    }

    /// Cutout layer normals as `Float32Array` (3 floats per vertex).
    pub fn cutout_normals(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.cutout.normals_bytes())
    }

    /// Cutout layer UVs as `Float32Array` (2 floats per vertex).
    pub fn cutout_uvs(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.cutout.uvs_bytes())
    }

    /// Cutout layer vertex colors as `Float32Array` (4 floats per vertex, RGBA).
    pub fn cutout_colors(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.cutout.colors_bytes())
    }

    /// Cutout layer triangle indices as `Uint32Array`.
    pub fn cutout_indices(&self) -> js_sys::Uint32Array {
        u32_typed_array(self.inner.cutout.indices_bytes())
    }

    /// Cutout layer vertex count.
    pub fn cutout_vertex_count(&self) -> usize {
        self.inner.cutout.vertex_count()
    }

    /// Cutout layer triangle count.
    pub fn cutout_triangle_count(&self) -> usize {
        self.inner.cutout.triangle_count()
    }

    /// Whether the cutout layer is empty.
    pub fn cutout_is_empty(&self) -> bool {
        self.inner.cutout.is_empty()
    }

    // --- Transparent layer ---

    /// Transparent layer positions as `Float32Array` (3 floats per vertex).
    pub fn transparent_positions(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.transparent.positions_bytes())
    }

    /// Transparent layer normals as `Float32Array` (3 floats per vertex).
    pub fn transparent_normals(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.transparent.normals_bytes())
    }

    /// Transparent layer UVs as `Float32Array` (2 floats per vertex).
    pub fn transparent_uvs(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.transparent.uvs_bytes())
    }

    /// Transparent layer vertex colors as `Float32Array` (4 floats per vertex, RGBA).
    pub fn transparent_colors(&self) -> js_sys::Float32Array {
        f32_typed_array(self.inner.transparent.colors_bytes())
    }

    /// Transparent layer triangle indices as `Uint32Array`.
    pub fn transparent_indices(&self) -> js_sys::Uint32Array {
        u32_typed_array(self.inner.transparent.indices_bytes())
    }

    /// Transparent layer vertex count.
    pub fn transparent_vertex_count(&self) -> usize {
        self.inner.transparent.vertex_count()
    }

    /// Transparent layer triangle count.
    pub fn transparent_triangle_count(&self) -> usize {
        self.inner.transparent.triangle_count()
    }

    /// Whether the transparent layer is empty.
    pub fn transparent_is_empty(&self) -> bool {
        self.inner.transparent.is_empty()
    }

    // --- Atlas ---

    /// Atlas RGBA pixel data as `Uint8Array`.
    pub fn atlas_rgba(&self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view(&self.inner.atlas.pixels) }
    }

    /// Atlas width in pixels.
    pub fn atlas_width(&self) -> u32 {
        self.inner.atlas.width
    }

    /// Atlas height in pixels.
    pub fn atlas_height(&self) -> u32 {
        self.inner.atlas.height
    }

    // --- Spatial info ---

    /// Bounding box minimum as `[x, y, z]`.
    pub fn bounds_min(&self) -> Vec<f32> {
        self.inner.bounds.min.to_vec()
    }

    /// Bounding box maximum as `[x, y, z]`.
    pub fn bounds_max(&self) -> Vec<f32> {
        self.inner.bounds.max.to_vec()
    }

    /// LOD level (0 = full detail).
    pub fn lod_level(&self) -> u8 {
        self.inner.lod_level
    }

    /// Chunk coordinate as `[cx, cy, cz]`, or `undefined` if not a chunk mesh.
    pub fn chunk_coord(&self) -> Option<Vec<i32>> {
        self.inner.chunk_coord.map(|(cx, cy, cz)| vec![cx, cy, cz])
    }

    // --- Export ---

    /// Export to GLB (binary glTF) format.
    pub fn to_glb(&self) -> Result<js_sys::Uint8Array, JsValue> {
        let bytes = self.inner.to_glb()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let len = bytes.len();
        let arr = js_sys::Uint8Array::new_with_length(len as u32);
        arr.copy_from(&bytes);
        Ok(arr)
    }

    /// Export to USDZ format.
    pub fn to_usdz(&self) -> Result<js_sys::Uint8Array, JsValue> {
        let bytes = self.inner.to_usdz()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let len = bytes.len();
        let arr = js_sys::Uint8Array::new_with_length(len as u32);
        arr.copy_from(&bytes);
        Ok(arr)
    }

    // --- Aggregate stats ---

    /// Total vertex count across all layers.
    pub fn total_vertices(&self) -> usize {
        self.inner.total_vertices()
    }

    /// Total triangle count across all layers.
    pub fn total_triangles(&self) -> usize {
        self.inner.total_triangles()
    }

    /// Whether there is any cutout or transparent geometry.
    pub fn has_transparency(&self) -> bool {
        self.inner.has_transparency()
    }

    /// Whether all layers are empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Chunk mesh iterator wrapper
// ---------------------------------------------------------------------------

/// WASM wrapper for lazy chunk iteration.
///
/// Created by [`mesh_chunks_json()`]. Call `advance()` to step to the next chunk,
/// then `current()` to get the [`MeshOutputWrapper`] for that chunk.
///
/// ```js
/// const iter = mesh_chunks_json(pack, json, null, 16);
/// while (iter.advance()) {
///     const coord = iter.current_coord(); // [cx, cy, cz]
///     const mesh = iter.current();        // MeshOutputWrapper
///     // Upload mesh.opaque_positions(), etc. to GPU
/// }
/// ```
///
/// `current()` transfers ownership of the chunk mesh — calling it twice without
/// `advance()` returns an empty mesh the second time.
#[wasm_bindgen]
pub struct ChunkMeshIteratorWrapper {
    results: Vec<crate::mesh_output::MeshOutput>,
    index: usize,
}

#[wasm_bindgen]
impl ChunkMeshIteratorWrapper {
    /// Advance to the next chunk. Returns `false` when exhausted.
    pub fn advance(&mut self) -> bool {
        if self.index < self.results.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }

    /// Get the current chunk's mesh output, or `undefined` if not yet advanced or exhausted.
    /// Note: this transfers ownership — calling `current()` twice without
    /// `advance()` will return an empty mesh the second time.
    pub fn current(&mut self) -> Option<MeshOutputWrapper> {
        if self.index == 0 || self.index > self.results.len() {
            return None;
        }
        let idx = self.index - 1;
        let empty = crate::mesh_output::MeshOutput {
            opaque: crate::mesh_output::MeshLayer::new(),
            cutout: crate::mesh_output::MeshLayer::new(),
            transparent: crate::mesh_output::MeshLayer::new(),
            atlas: crate::atlas::TextureAtlas::empty(),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
            bounds: crate::BoundingBox::new([0.0; 3], [0.0; 3]),
            chunk_coord: None,
            lod_level: 0,
        };
        let taken = std::mem::replace(&mut self.results[idx], empty);
        Some(MeshOutputWrapper { inner: taken })
    }

    /// Get the current chunk coordinate as `[cx, cy, cz]`.
    pub fn current_coord(&self) -> Vec<i32> {
        if self.index == 0 || self.index > self.results.len() {
            return vec![0, 0, 0];
        }
        let idx = self.index - 1;
        match self.results[idx].chunk_coord {
            Some((cx, cy, cz)) => vec![cx, cy, cz],
            None => vec![0, 0, 0],
        }
    }

    /// Total number of chunks.
    pub fn chunk_count(&self) -> usize {
        self.results.len()
    }
}

// ---------------------------------------------------------------------------
// Block source wrapper for chunk iteration from WASM
// ---------------------------------------------------------------------------

/// A simple in-memory block source for WASM chunk meshing.
struct WasmBlockSource {
    blocks: Vec<(crate::BlockPosition, crate::InputBlock)>,
    bounds: crate::BoundingBox,
}

impl crate::BlockSource for WasmBlockSource {
    fn get_block(&self, pos: crate::BlockPosition) -> Option<&crate::InputBlock> {
        self.blocks.iter().find(|(p, _)| *p == pos).map(|(_, b)| b)
    }

    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (crate::BlockPosition, &crate::InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }

    fn bounds(&self) -> crate::BoundingBox {
        self.bounds
    }
}

/// Mesh blocks from JSON as chunks, returning a [`ChunkMeshIteratorWrapper`].
///
/// The JSON format is the same as `mesh_blocks_json`. `chunk_size` is the side
/// length of each cubic chunk in blocks (e.g., 16).
#[wasm_bindgen]
pub fn mesh_chunks_json(
    pack: &ResourcePackHandle,
    json: &str,
    options: Option<MesherOptions>,
    chunk_size: i32,
) -> Result<ChunkMeshIteratorWrapper, JsError> {
    let (blocks, bounds) = parse_blocks_json(json)?;
    let options = options.unwrap_or_default();
    let config = build_config(&options);

    let source = WasmBlockSource { blocks, bounds };
    let mesher = crate::Mesher::with_config(pack.inner.clone(), config);
    let chunk_iter = mesher.mesh_chunks(&source, chunk_size);

    let mut results = Vec::new();
    for result in chunk_iter {
        results.push(result.map_err(|e| JsError::new(&e.to_string()))?);
    }

    Ok(ChunkMeshIteratorWrapper {
        results,
        index: 0,
    })
}
