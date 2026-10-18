//! Convert model elements to mesh geometry.

use crate::atlas::{AtlasBuilder, TextureAtlas};
use crate::error::{MesherError, Result};
use crate::mesher::entity;
use crate::mesher::face_culler::FaceCuller;
use crate::mesher::geometry::{Mesh, Vertex};
use crate::mesher::greedy::{quantize_color, FaceMergeKey, GreedyMesher};
use crate::mesher::liquid::{self, FluidState};
use crate::mesher::MesherConfig;
use crate::resolver::{resolve_block, ModelResolver, ResolvedModel};
use crate::resource_pack::{ModelElement, ModelFace, ResourcePack, TextureData};

// --- Fast-path coverage instrumentation (env `MESHER_STATS`) -----------------
// Counts, per mesh, how many emitted faces are full-opaque-cube ("fast-path"
// eligible for a binary greedy mesher) vs total, and how many of those are fully
// lit (mergeable without per-AO handling). Used to size the hybrid binary mesher
// against the non-cube/state reality before integrating it.
pub(crate) static STAT_TOTAL_FACES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static STAT_CUBE_FACES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
pub(crate) static STAT_CUBE_LIT_FACES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[inline]
fn stats_enabled() -> bool {
    use std::sync::atomic::{AtomicU8, Ordering};
    static S: AtomicU8 = AtomicU8::new(2); // 2 = uninitialized
    match S.load(Ordering::Relaxed) {
        0 => false,
        1 => true,
        _ => {
            let v = std::env::var("MESHER_STATS").is_ok();
            S.store(v as u8, Ordering::Relaxed);
            v
        }
    }
}
use crate::types::{BlockPosition, BlockTransform, Direction, InputBlock};
use glam::{Mat3, Vec3};
use std::collections::{HashMap, HashSet};

/// Tracks texture mapping for a face (4 vertices).
struct FaceTextureMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Starting index in the index buffer (6 indices per quad: 2 triangles).
    index_start: usize,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
}

/// Tracks texture mapping for a greedy-merged face (bypasses atlas).
struct GreedyFaceMapping {
    /// Starting vertex index.
    vertex_start: u32,
    /// Starting index in the index buffer (6 indices per quad: 2 triangles).
    index_start: usize,
    /// Texture path for this face.
    texture_path: String,
    /// Whether this face uses a transparent texture.
    is_transparent: bool,
    /// Per-vertex AO values (from FaceMergeKey).
    ao: [u8; 4],
}

/// A material for greedy-merged faces, with its own texture (not atlas-packed).
/// UVs on these meshes exceed [0,1] for tiling via REPEAT wrapping.
#[derive(Debug)]
pub struct GreedyMaterial {
    /// Texture path (e.g., "block/stone").
    pub texture_path: String,
    /// Opaque geometry using this texture.
    pub opaque_mesh: Mesh,
    /// Transparent geometry using this texture.
    pub transparent_mesh: Mesh,
    /// PNG-encoded texture data for this material.
    pub texture_png: Vec<u8>,
}

/// Build a cache key from a block's name and properties.
/// Format: "name" for blocks with no properties, "name|k1=v1,k2=v2" with sorted keys otherwise.
fn block_cache_key(block: &InputBlock) -> String {
    if block.properties.is_empty() {
        block.name.clone()
    } else {
        let mut props: Vec<_> = block.properties.iter().collect();
        props.sort_by_key(|(k, _)| k.as_str());
        let mut key = block.name.clone();
        key.push('|');
        for (i, (k, v)) in props.iter().enumerate() {
            if i > 0 {
                key.push(',');
            }
            key.push_str(k);
            key.push('=');
            key.push_str(v);
        }
        key
    }
}

/// Send-able accumulated geometry extracted from a per-chunk [`MeshBuilder`] so
/// it can be merged on the main thread after parallel meshing. Deliberately
/// excludes the thread-local resolve/model caches (which hold `Rc`/`RefCell`).
pub(crate) struct PartialMesh {
    mesh: Mesh,
    texture_refs: HashSet<String>,
    face_textures: Vec<FaceTextureMapping>,
    greedy_face_textures: Vec<GreedyFaceMapping>,
    dynamic_textures: HashMap<String, TextureData>,
}

/// Builds a mesh from multiple blocks.
pub struct MeshBuilder<'a> {
    resource_pack: &'a ResourcePack,
    config: &'a MesherConfig,
    mesh: Mesh,
    texture_refs: HashSet<String>,
    model_resolver: ModelResolver<'a>,
    /// Track which texture each face uses for UV remapping.
    face_textures: Vec<FaceTextureMapping>,
    /// Track greedy-merged faces separately (bypass atlas, use tiled UVs).
    greedy_face_textures: Vec<GreedyFaceMapping>,
    /// Face culler for visibility and AO calculations.
    culler: Option<&'a FaceCuller<'a>>,
    /// Greedy mesher for merging adjacent coplanar faces.
    greedy: Option<GreedyMesher>,
    /// Cache of resolved models keyed by block identity (name + properties).
    /// Wrapped in `Rc` so per-block cache hits only bump a refcount instead of
    /// deep-cloning the resolved `BlockModel`s (hot path: millions of blocks).
    resolve_cache: rustc_hash::FxHashMap<String, std::rc::Rc<Vec<ResolvedModel>>>,
    /// 1-entry memo of the last resolved block, keyed by its `InputBlock` pointer.
    /// Blocks are processed in spatial scan order, so terrain has long runs of the
    /// same palette entry (same pointer) — these skip the per-block string cache
    /// key entirely. Non-palette sources (distinct pointers) simply never hit it.
    last_block_ptr: usize,
    last_resolved: std::rc::Rc<Vec<ResolvedModel>>,
    /// Block map for neighbor lookups (used by liquid geometry).
    block_map: Option<&'a rustc_hash::FxHashMap<BlockPosition, &'a InputBlock>>,
    /// Light map for brightness calculations.
    light_map: Option<&'a crate::mesher::lighting::LightMap>,
    /// Dynamic textures generated at build time (banners, inventories).
    /// Keys starting with `_` are synthetic texture paths.
    dynamic_textures: HashMap<String, TextureData>,
    /// Block names by whether a binding row could match them.
    bindable_names: rustc_hash::FxHashMap<String, bool>,
    /// Each composed text display texture's layout size in font units
    /// (LumilioCL local change, plan schematic-studio T35).
    text_sizes: HashMap<String, (u32, u32)>,
}

/// Synthetic atlas key for the fallback "unknown texture" tile. Added to every
/// atlas so faces whose declared texture couldn't be resolved have a valid
/// region to sample, instead of leaking into full-atlas [0,1] UVs.
const MISSING_TEXTURE_KEY: &str = "__missing__";

/// The placeholder for a block no model can draw: one full cube whose six
/// faces sample the missing-texture tile, culled like any solid cube. The
/// game draws unknown blocks the same way. LumilioCL local change (plan
/// schematic-studio D24).
fn placeholder_models() -> Vec<ResolvedModel> {
    let faces = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ]
    .into_iter()
    .map(|direction| {
        (
            direction,
            ModelFace {
                uv: None,
                texture: "#missing".to_string(),
                cullface: Some(direction),
                rotation: 0,
                tintindex: -1,
            },
        )
    })
    .collect();
    let mut model = crate::resource_pack::BlockModel::new();
    model.elements.push(ModelElement {
        from: [0., 0., 0.],
        to: [16., 16., 16.],
        rotation: None,
        shade: true,
        shade_direction_override: None,
        faces,
    });
    vec![ResolvedModel {
        model,
        transform: BlockTransform::default(),
    }]
}

/// Build a 16x16 magenta/black checkerboard texture, MC-style, to mark faces
/// whose texture couldn't be found in the resource pack.
fn make_missing_texture() -> TextureData {
    const SIZE: u32 = 16;
    let mut pixels = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let magenta = (x / 8 + y / 8) % 2 == 0;
            if magenta {
                pixels.extend_from_slice(&[0xF8, 0x00, 0xF8, 0xFF]);
            } else {
                pixels.extend_from_slice(&[0x00, 0x00, 0x00, 0xFF]);
            }
        }
    }
    TextureData {
        width: SIZE,
        height: SIZE,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    }
}

impl<'a> MeshBuilder<'a> {
    pub fn new(
        resource_pack: &'a ResourcePack,
        config: &'a MesherConfig,
        culler: Option<&'a FaceCuller<'a>>,
        block_map: Option<&'a rustc_hash::FxHashMap<BlockPosition, &'a InputBlock>>,
        light_map: Option<&'a crate::mesher::lighting::LightMap>,
    ) -> Self {
        let greedy = if config.greedy_meshing {
            Some(GreedyMesher::new())
        } else {
            None
        };
        Self {
            resource_pack,
            config,
            mesh: Mesh::new(),
            texture_refs: HashSet::new(),
            model_resolver: ModelResolver::new(resource_pack),
            face_textures: Vec::new(),
            greedy_face_textures: Vec::new(),
            culler,
            greedy,
            resolve_cache: rustc_hash::FxHashMap::default(),
            last_block_ptr: 0,
            last_resolved: std::rc::Rc::new(Vec::new()),
            block_map,
            light_map,
            dynamic_textures: HashMap::new(),
            bindable_names: rustc_hash::FxHashMap::default(),
            text_sizes: HashMap::new(),
        }
    }

    /// Returns the set of texture paths collected during face processing.
    /// Useful for discovering all textures needed before building a global atlas.
    pub fn texture_refs(&self) -> &HashSet<String> {
        &self.texture_refs
    }

    /// Add a block to the mesh.
    pub fn add_block(&mut self, pos: BlockPosition, block: &InputBlock) -> Result<()> {
        let first_face = self.face_textures.len();
        let result = self.add_block_geometry(pos, block);
        self.separate_coplanar_faces(first_face);
        result
    }

    /// Push every face of one block that lies in the plane of an earlier,
    /// overlapping face facing the same way a little along its normal.
    ///
    /// Minecraft draws such layers (grass side overlays, redstone line and
    /// overlay, multipart pieces) at the same depth and relies on draw order. A
    /// renderer that splits them across layers, or triangulates them
    /// differently, shows them fighting. A step of 1/1024 block keeps the
    /// original order visible and stays far below one texel.
    fn separate_coplanar_faces(&mut self, first_face: usize) {
        const STEP: f32 = 1.0 / 1024.0;
        let faces: Vec<(usize, usize)> = self.face_textures[first_face..]
            .iter()
            .map(|face| (face.vertex_start as usize, face.index_start))
            .collect();
        if faces.len() < 2 {
            return;
        }
        let vertices = &self.mesh.vertices;
        let indices = &self.mesh.indices;
        let planes: Vec<Option<(Vec3, f32, Vec3, Vec3)>> = faces
            .iter()
            .map(|&(start, index_start)| {
                let corners = vertices.get(start..start + 4)?;
                let p: [Vec3; 4] = std::array::from_fn(|k| Vec3::from(corners[k].position));
                // The first triangle's winding is the side the GPU treats as the front.
                let [a, b, c] = indices
                    .get(index_start..index_start + 3)?
                    .iter()
                    .map(|&i| Vec3::from(vertices[i as usize].position))
                    .collect::<Vec<_>>()
                    .try_into()
                    .ok()?;
                let normal = (b - a).cross(c - a);
                if normal.length_squared() < 1e-12 {
                    return None;
                }
                let normal = normal.normalize();
                let min = p.iter().copied().reduce(Vec3::min)?;
                let max = p.iter().copied().reduce(Vec3::max)?;
                Some((normal, normal.dot(p[0]), min, max))
            })
            .collect();
        for i in 1..faces.len() {
            let Some((normal, plane, min, max)) = planes[i] else {
                continue;
            };
            let below = planes[..i]
                .iter()
                .flatten()
                .filter(|(other_normal, other_plane, other_min, other_max)| {
                    let overlap = max.min(*other_max) - min.max(*other_min);
                    let spans = [overlap.x, overlap.y, overlap.z];
                    other_normal.dot(normal) > 0.9999
                        && (other_plane - plane).abs() < 1e-4
                        && spans.iter().all(|&s| s > -1e-5)
                        && spans.iter().filter(|&&s| s > 1e-5).count() >= 2
                })
                .count();
            if below > 0 {
                let shift = normal * (STEP * below as f32);
                let start = faces[i].0;
                for vertex in &mut self.mesh.vertices[start..start + 4] {
                    vertex.position = (Vec3::from(vertex.position) + shift).into();
                }
            }
        }
    }

    fn add_block_geometry(&mut self, pos: BlockPosition, block: &InputBlock) -> Result<()> {
        // Entities (the `entity:` namespace) are drawn from the binding table
        // alone; they have no block model.
        let bound = if self.may_bind(block) {
            entity::bindings::find_for(&self.resource_pack.entity_looks, block)
        } else {
            None
        };
        if let Some(bound) = bound.as_ref().filter(|bound| bound.is_entity) {
            return self.add_bound(pos, block, bound);
        }

        // Check if this is a liquid block — generate custom geometry
        if let Some(fluid_state) = FluidState::from_block(block) {
            return self.add_liquid(pos, block, &fluid_state);
        }

        // Fast path: identical to the previous block (same palette entry — common
        // in terrain runs). Reuse the last resolution, skipping the per-block
        // string cache key + hash entirely.
        let block_ptr = block as *const InputBlock as usize;
        let resolved = if block_ptr == self.last_block_ptr {
            std::rc::Rc::clone(&self.last_resolved)
        } else {
            let cache_key = block_cache_key(block);
            // Check resolution cache. On a hit we clone only the `Rc` (a refcount
            // bump), not the underlying resolved models — this owned handle also
            // releases the borrow on `self.resolve_cache` so `add_model` can take
            // `&mut self`.
            let rc = if let Some(cached) = self.resolve_cache.get(&cache_key) {
                std::rc::Rc::clone(cached)
            } else {
                // Resolve the block to models. A block nothing can draw gets
                // the missing-texture placeholder cube (LumilioCL local
                // change, plan schematic-studio D24): the shape stays whole
                // and the block can be picked. Fluids and bound entities draw
                // themselves and need no placeholder.
                let resolved_models = match resolve_block(self.resource_pack, block) {
                    Ok(models) => models,
                    Err(e) => {
                        eprintln!("Warning: Failed to resolve block {}: {}", block.name, e);
                        if bound.is_some() {
                            Vec::new()
                        } else {
                            placeholder_models()
                        }
                    }
                };
                let rc = std::rc::Rc::new(resolved_models);
                // Store in cache for future blocks with same identity
                self.resolve_cache
                    .insert(cache_key, std::rc::Rc::clone(&rc));
                rc
            };
            self.last_block_ptr = block_ptr;
            self.last_resolved = std::rc::Rc::clone(&rc);
            rc
        };

        // Generate geometry for each model
        for resolved in resolved.iter() {
            self.add_model(pos, block, resolved)?;
        }

        // Block entities add their bound layers to the block's own model.
        // From 26.2 signs and beds are block models: when the model has
        // elements it draws the block and the entity is not added.
        if let Some(bound) = bound {
            let model_draws_it = bound.binding.model_replaces
                && resolved.iter().any(|r| !r.model.elements.is_empty());
            if !model_draws_it {
                self.add_bound(pos, block, &bound)?;
            }
        }

        // Check for inventory property — render hologram above container
        if let Some(inventory_str) = block.properties.get("inventory") {
            self.add_inventory_hologram(pos, inventory_str)?;
        }

        // Check for particle sources (torches, campfires, candles, etc.)
        if self.config.enable_particles {
            if let Some(source) = entity::particle::detect_particle_source(block) {
                self.add_particles(pos, &source)?;
            }
        }

        // Waterlogged blocks: add water source overlay.
        if liquid::is_waterlogged(block) {
            let water_state = FluidState {
                fluid_type: liquid::FluidType::Water,
                amount: 8,
                is_source: true,
                is_falling: false,
            };
            self.add_liquid(pos, block, &water_state)?;
        }

        Ok(())
    }

    /// Consume this builder, returning its accumulated geometry as a `Send`
    /// [`PartialMesh`]. The thread-local resolve/model caches are dropped.
    pub(crate) fn into_partial(self) -> PartialMesh {
        PartialMesh {
            mesh: self.mesh,
            texture_refs: self.texture_refs,
            face_textures: self.face_textures,
            greedy_face_textures: self.greedy_face_textures,
            dynamic_textures: self.dynamic_textures,
        }
    }

    /// Add liquid geometry for a water/lava block.
    fn add_liquid(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        state: &FluidState,
    ) -> Result<()> {
        self.add_liquid_inset(pos, block, state, 0.0)
    }

    /// Same as `add_liquid` but shrinks the generated water cube by `inset`
    /// toward its block center. Used for waterlogged blocks so the water's
    /// boundary faces don't coincide with the host block's own faces.
    fn add_liquid_inset(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        state: &FluidState,
        inset: f32,
    ) -> Result<()> {
        // Get the block map for neighbor lookups
        let empty_map: rustc_hash::FxHashMap<BlockPosition, &InputBlock> =
            rustc_hash::FxHashMap::default();
        let block_map = self.block_map.unwrap_or(&empty_map);

        // Determine base color: water always uses the biome water tint (MC applies
        // this regardless of host block, including waterlogged stairs/slabs/etc).
        // Lava is never tinted.
        let base_color = match state.fluid_type {
            liquid::FluidType::Water => {
                let mut c = self.config.tint_provider.colors().water;
                c[3] = 0.8; // Water is semi-transparent
                c
            }
            liquid::FluidType::Lava => [1.0, 1.0, 1.0, 1.0],
        };

        // Opacity check function using the culler
        let is_opaque = |p: BlockPosition| -> bool {
            self.culler
                .map(|c| c.is_fully_opaque_at(p))
                .unwrap_or(false)
        };

        let (mut vertices, indices, face_textures) =
            liquid::generate_fluid_geometry(pos, state, block_map, is_opaque, base_color);

        // Apply inset: pull each vertex toward the block center. At inset=0.002
        // the shift is imperceptible but enough to eliminate coplanar z-fighting.
        if inset > 0.0 {
            let cx = pos.x as f32 + 0.5;
            let cy = pos.y as f32 + 0.5;
            let cz = pos.z as f32 + 0.5;
            for v in &mut vertices {
                v.position[0] += (cx - v.position[0]).signum() * inset;
                v.position[1] += (cy - v.position[1]).signum() * inset;
                v.position[2] += (cz - v.position[2]).signum() * inset;
            }
        }

        // Register texture refs
        self.texture_refs.insert(state.still_texture().to_string());
        self.texture_refs.insert(state.flow_texture().to_string());

        // Add vertices and track face texture mappings.
        // Each FaceTexture corresponds to one quad (4 vertices, 6 indices).
        let base_vertex = self.mesh.vertex_count() as u32;

        for v in &vertices {
            self.mesh.add_vertex(*v);
        }

        // Each face in face_textures corresponds to sequential groups of 4 verts / 6 indices
        let base_index = self.mesh.indices.len();

        for &idx in &indices {
            self.mesh.indices.push(base_vertex + idx);
        }

        let mut idx_offset = base_index;
        for ft in &face_textures {
            // Find the minimum vertex index in this face's 6 indices to determine vertex_start
            let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: idx_offset,
                texture_path: ft.texture.to_string(),
                is_transparent: ft.is_transparent,
            });
            idx_offset += 6;
        }

        Ok(())
    }

    /// Whether any binding row could match `block`'s name, remembered per
    /// name so terrain does not walk the table (LumilioCL local change).
    fn may_bind(&mut self, block: &InputBlock) -> bool {
        if let Some(&known) = self.bindable_names.get(&block.name) {
            return known;
        }
        let known = entity::bindings::known_for(&self.resource_pack.entity_looks, block);
        self.bindable_names.insert(block.name.clone(), known);
        known
    }

    /// Add a block or entity the binding table draws (LumilioCL local change,
    /// plan schematic-studio T7; see `entity::bindings`).
    fn add_bound(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        bound: &entity::bindings::Bound,
    ) -> Result<()> {
        use entity::bindings::Compose;
        match bound.binding.compose {
            Some(Compose::ItemFrame) => return self.add_item_frame(pos, block, bound),
            Some(Compose::DroppedItem) => return self.add_dropped_item(pos, block, bound),
            Some(Compose::Painting) => {
                let place = entity::render::placement(bound, &entity::render::Look::default());
                if let Some((vertices, indices, faces)) =
                    entity::painting::geometry(self.resource_pack, bound, place)
                {
                    self.add_item_geometry(pos, &vertices, &indices, &faces);
                }
                return Ok(());
            }
            Some(Compose::ItemDisplay) => {
                let place = entity::render::placement(bound, &entity::render::Look::default());
                let context = bound.var("item_display").unwrap_or("none");
                let Some(item) = bound.var("item") else {
                    return Ok(());
                };
                if let Some((vertices, indices, faces)) =
                    entity::item_render::render_item_in_context(
                        self.resource_pack,
                        &self.model_resolver,
                        item,
                        context,
                        place,
                    )
                {
                    self.add_item_geometry(pos, &vertices, &indices, &faces);
                }
                return Ok(());
            }
            Some(Compose::TextDisplay) => return self.add_text_display(pos, bound),
            _ => {}
        }
        if let Some(sprite) = &bound.binding.sprite {
            return self.add_sprite(pos, bound, sprite);
        }
        let mut bound = bound.clone();
        // A ridden pig or horse always shows its saddle.
        if block.properties.contains_key("rider") {
            bound
                .properties
                .insert("saddle".to_owned(), "true".to_owned());
        }
        let key = format!("{}_{}_{}", pos.x, pos.y, pos.z);
        let geometry = self.bound_geometry(block, &bound, &key);
        let drawn = geometry.is_some();
        if let Some((mut vertices, indices, faces)) = geometry {
            if !bound.is_entity {
                self.light_block_entity(pos, &mut vertices);
            }
            self.add_item_geometry(pos, &vertices, &indices, &faces);
        }
        if let Some(carried) = &bound.binding.carries {
            self.add_carried_block(pos, &bound, carried)?;
        } else if !drawn {
            return Ok(());
        }
        if bound.is_entity {
            if let Some(rider) = block.properties.get("rider").cloned() {
                self.add_rider_on(pos, block, &bound, &rider)?;
            }
        }
        Ok(())
    }

    /// Block entities take the light of their block, as before.
    fn light_block_entity(&self, pos: BlockPosition, vertices: &mut [Vertex]) {
        let Some(light) = self.light_map else {
            return;
        };
        if light.is_emissive(pos) {
            return;
        }
        let brightness = light.face_brightness(pos, Direction::Up);
        for vertex in vertices {
            for channel in &mut vertex.color[..3] {
                *channel *= brightness;
            }
        }
    }

    /// The faces of `bound` with the textures its composition makes. `key`
    /// names this block's own composed textures (skins).
    fn bound_geometry(
        &mut self,
        block: &InputBlock,
        bound: &entity::bindings::Bound,
        key: &str,
    ) -> Option<entity::model::Geometry> {
        use entity::bindings::Compose;
        use entity::sign_text::SignKind;
        let mut look = entity::render::Look::default();
        match bound.binding.compose {
            Some(Compose::Banner) => {
                look.texture = self.banner_texture(block, bound.var("color").unwrap_or("white"));
            }
            Some(Compose::StandingSignText) => {
                look.texture = self.sign_texture(block, bound, SignKind::Standing);
            }
            Some(Compose::WallSignText) => {
                look.texture = self.sign_texture(block, bound, SignKind::Wall);
            }
            Some(Compose::HangingSignText) => {
                look.texture = self.sign_texture(block, bound, SignKind::Hanging);
            }
            Some(Compose::DecoratedPot) => {
                look.bone_textures = self.pot_textures(block);
            }
            Some(Compose::PlayerSkin) => {
                let prefix = if bound.is_entity {
                    "_player"
                } else {
                    "_player_head"
                };
                look.texture = self.player_skin(block, &format!("{prefix}/{key}"));
                look.vars.insert(
                    "model".to_owned(),
                    entity::skin::player_model(block, self.resource_pack).to_owned(),
                );
            }
            Some(Compose::Villager) => {
                look.texture = self.villager_texture(block);
            }
            Some(
                Compose::ItemFrame
                | Compose::DroppedItem
                | Compose::Painting
                | Compose::ExperienceOrb
                | Compose::ItemDisplay
                | Compose::TextDisplay,
            )
            | None => {}
        }
        entity::render::draw(self.resource_pack, bound, &look)
    }

    /// The banner's base colour and patterns as one texture.
    fn banner_texture(&mut self, block: &InputBlock, color: &str) -> Option<String> {
        let patterns = block
            .properties
            .get("patterns")
            .map(|s| entity::banner::parse_patterns(s))
            .unwrap_or_default();
        let mut key = format!("_banner/{color}");
        for (pattern, dye) in &patterns {
            key.push_str(&format!("_{pattern}{dye}"));
        }
        if !self.dynamic_textures.contains_key(&key) {
            let texture =
                entity::banner::composite_banner_texture(self.resource_pack, color, &patterns)?;
            self.dynamic_textures.insert(key.clone(), texture);
        }
        Some(key)
    }

    /// The sign's own texture with `text1`–`text4` drawn on; `None` without
    /// text or without the font, which leaves the plain texture.
    fn sign_texture(
        &mut self,
        block: &InputBlock,
        bound: &entity::bindings::Bound,
        kind: entity::sign_text::SignKind,
    ) -> Option<String> {
        let lines: Vec<&str> = (1..=4)
            .filter_map(|i| {
                block
                    .properties
                    .get(&format!("text{i}"))
                    .map(String::as_str)
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        let base = entity::render::row_texture(self.resource_pack, bound)?;
        let color = block
            .properties
            .get("color")
            .map(String::as_str)
            .unwrap_or("black");
        let glowing = block.properties.get("glowing").is_some_and(|g| g == "true");
        let mut key = format!(
            "_sign/{base}_{color}_{}",
            if glowing { "glow" } else { "normal" }
        );
        for line in &lines {
            key.push('_');
            key.push_str(line);
        }
        if !self.dynamic_textures.contains_key(&key) {
            let texture = entity::sign_text::composite_sign_with_text(
                self.resource_pack,
                &base,
                &lines,
                color,
                glowing,
                kind,
            )?;
            self.dynamic_textures.insert(key.clone(), texture);
        }
        Some(key)
    }

    /// Each pot side's texture: the plain side, or its sherd's pattern drawn
    /// over it.
    fn pot_textures(&mut self, block: &InputBlock) -> std::collections::BTreeMap<String, String> {
        let mut textures = std::collections::BTreeMap::new();
        for (side, pattern) in entity::decorated_pot::SIDES
            .into_iter()
            .zip(entity::decorated_pot::patterns(block))
        {
            let key = match pattern {
                None => entity::decorated_pot::SIDE_TEXTURE.to_owned(),
                Some(pattern) => {
                    let key = format!("_pot/{pattern}");
                    if !self.dynamic_textures.contains_key(&key) {
                        if let Some(texture) =
                            entity::decorated_pot::composite_side(self.resource_pack, &pattern)
                        {
                            self.dynamic_textures.insert(key.clone(), texture);
                        }
                    }
                    key
                }
            };
            textures.insert(side.to_owned(), key);
        }
        textures
    }

    /// The skin of a player or player head, stored under `key`.
    fn player_skin(&mut self, block: &InputBlock, key: &str) -> Option<String> {
        if !self.dynamic_textures.contains_key(key) {
            let skin = entity::skin::skin_of(block).or_else(|| {
                self.resource_pack
                    .get_texture(entity::skin::player_skin_fallback_path(
                        block,
                        self.resource_pack,
                    ))
                    .cloned()
            })?;
            self.dynamic_textures.insert(key.to_owned(), skin);
        }
        Some(key.to_owned())
    }

    /// A villager's skin with its biome and profession clothes.
    fn villager_texture(&mut self, block: &InputBlock) -> Option<String> {
        let biome = block
            .properties
            .get("biome")
            .map(String::as_str)
            .unwrap_or("plains");
        let profession = block
            .properties
            .get("profession")
            .map(String::as_str)
            .unwrap_or("none");
        let key = format!("_villager/{biome}/{profession}");
        if !self.dynamic_textures.contains_key(&key) {
            let texture = entity::villager_texture::composite_villager_texture(
                self.resource_pack,
                biome,
                profession,
            )?;
            self.dynamic_textures.insert(key.clone(), texture);
        }
        Some(key)
    }

    /// An item frame: its block model, then the item it holds. An invisible
    /// frame shows only the item.
    fn add_item_frame(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        bound: &entity::bindings::Bound,
    ) -> Result<()> {
        let facing = bound.var("facing").unwrap_or("south");
        let invisible = block
            .properties
            .get("invisible")
            .is_some_and(|v| v == "true");
        if !invisible {
            let (vertices, indices, faces) =
                entity::item_frame::frame_geometry(self.resource_pack, bound.flag("glow"), facing);
            if !vertices.is_empty() {
                self.add_item_geometry(pos, &vertices, &indices, &faces);
            }
        }
        if let Some(item_id) = block.properties.get("item") {
            let rotation: u8 = block
                .properties
                .get("item_rotation")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if let Some((item_verts, item_indices, item_faces)) =
                entity::item_render::render_item_in_frame(
                    self.resource_pack,
                    &self.model_resolver,
                    item_id,
                    rotation,
                    facing,
                    invisible,
                )
            {
                self.add_item_geometry(pos, &item_verts, &item_indices, &item_faces);
            }
        }
        Ok(())
    }

    /// A dropped item: its sprite or block model.
    fn add_dropped_item(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        bound: &entity::bindings::Bound,
    ) -> Result<()> {
        // The entity's own item, else the row's default (a thrown snowball
        // that saves none is a snowball).
        let Some(item_id) = bound.var("item").map(str::to_owned) else {
            return Ok(());
        };
        let item_id = &item_id;
        let facing = block
            .properties
            .get("facing")
            .map(String::as_str)
            .unwrap_or("south");
        if let Some((item_verts, item_indices, item_faces)) =
            entity::item_render::render_dropped_item(
                self.resource_pack,
                &self.model_resolver,
                item_id,
                facing,
            )
        {
            self.add_item_geometry(pos, &item_verts, &item_indices, &item_faces);
        }
        Ok(())
    }

    /// A rider (`rider` property) sitting on the host entity at the host's
    /// `rider_offset`. One level only: the rider's own `rider` is ignored.
    fn add_rider_on(
        &mut self,
        host_pos: BlockPosition,
        host_block: &InputBlock,
        host: &entity::bindings::Bound,
        rider_name: &str,
    ) -> Result<()> {
        let bare = rider_name.strip_prefix("entity:").unwrap_or(rider_name);
        let bare = bare.strip_prefix("minecraft:").unwrap_or(bare);
        let mut rider_block = InputBlock::new(format!("entity:{bare}"));
        let facing = host.var("facing").unwrap_or("south").to_owned();
        rider_block.properties.insert("facing".to_owned(), facing);
        // The rider wears what the host was told about it (skin, armor, poses).
        for key in [
            "skin",
            "skin_base64",
            "uuid",
            "slim",
            "helmet",
            "chestplate",
            "leggings",
            "boots",
            "color",
            "variant",
            "is_baby",
            "HeadPose",
            "BodyPose",
            "RightArmPose",
            "LeftArmPose",
            "RightLegPose",
            "LeftLegPose",
        ] {
            if let Some(value) = host_block.properties.get(key) {
                rider_block.properties.insert(key.to_owned(), value.clone());
            }
        }
        let Some(rider) =
            entity::bindings::find_for(&self.resource_pack.entity_looks, &rider_block)
        else {
            return Ok(());
        };
        if !rider.is_entity || rider.binding.compose == Some(entity::bindings::Compose::ItemFrame) {
            return Ok(());
        }
        let mut rider = rider;
        // HumanoidModel.setupAnim while riding: legs forward about 81°, spread 18°.
        if rider.binding.humanoid && !rider.properties.contains_key("RightLegPose") {
            rider
                .properties
                .insert("RightLegPose".to_owned(), "-81,18,0".to_owned());
            rider
                .properties
                .insert("LeftLegPose".to_owned(), "-81,-18,0".to_owned());
        }
        let seat = host.binding.rider_offset.unwrap_or([0.0, 0.3, 0.0]);
        let key = format!("{}_{}_{}_rider", host_pos.x, host_pos.y, host_pos.z);
        if let Some((vertices, indices, faces)) = self.bound_geometry(&rider_block, &rider, &key) {
            self.add_offset_geometry(host_pos, seat, &vertices, &indices, &faces);
        }
        Ok(())
    }

    /// A row's flat picture (LumilioCL local change, plan schematic-studio
    /// T35); an experience orb shows the icon of its `value` and the colour
    /// its renderer starts at.
    fn add_sprite(
        &mut self,
        pos: BlockPosition,
        bound: &entity::bindings::Bound,
        sprite: &entity::bindings::Sprite,
    ) -> Result<()> {
        let Some(texture) = bound.expand(&sprite.texture) else {
            return Ok(());
        };
        let place = entity::render::placement(bound, &entity::render::Look::default());
        let orb = bound.binding.compose == Some(entity::bindings::Compose::ExperienceOrb);
        let uv = if orb {
            // `ExperienceOrb.getIcon`: sixteen icons of 16 pixels on a 64
            // pixel sheet, by the orb's value.
            const STEPS: [i64; 10] = [2477, 1237, 617, 307, 149, 73, 37, 17, 7, 3];
            let value: i64 = bound.var("value").and_then(|v| v.parse().ok()).unwrap_or(0);
            let icon = STEPS
                .iter()
                .position(|&step| value >= step)
                .map_or(0, |at| 10 - at) as f32;
            let (u0, v0) = ((icon % 4.0) * 0.25, (icon / 4.0).floor() * 0.25);
            [u0, v0, u0 + 0.25, v0 + 0.25]
        } else {
            sprite.uv.unwrap_or([0.0, 0.0, 1.0, 1.0])
        };
        let mut out = (Vec::new(), Vec::new(), Vec::new());
        entity::painting::flat(&mut out, place, sprite.from, sprite.to, uv, &texture, orb);
        if orb {
            // The colour at age 0, half see-through.
            for vertex in &mut out.0 {
                vertex.color = [127.0 / 255.0, 1.0, 3.0 / 255.0, 128.0 / 255.0];
            }
        }
        self.add_item_geometry(pos, &out.0, &out.1, &out.2);
        Ok(())
    }

    /// A text display's lines on their background, laid out as the game's
    /// renderer lays them out, 1/40 block a font unit (T35).
    fn add_text_display(
        &mut self,
        pos: BlockPosition,
        bound: &entity::bindings::Bound,
    ) -> Result<()> {
        let Some(text) = bound.var("text") else {
            return Ok(());
        };
        let line_width = bound
            .var("line_width")
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        let background = bound
            .var("background")
            .and_then(|v| v.parse::<i64>().ok())
            .map_or(0x4000_0000, |v| v as u32);
        let alignment = bound.var("alignment").unwrap_or("center");
        let key = format!("_text/{line_width}_{background:08x}_{alignment}_{text}");
        let size = match self.text_sizes.get(&key) {
            Some(&size) => size,
            None => {
                let display = entity::sign_text::TextDisplay {
                    text,
                    line_width,
                    background,
                    alignment,
                };
                let Some((texture, w, h)) =
                    entity::sign_text::composite_text_display(self.resource_pack, &display)
                else {
                    return Ok(());
                };
                self.dynamic_textures.insert(key.clone(), texture);
                self.text_sizes.insert(key.clone(), (w, h));
                (w, h)
            }
        };
        let (w, h) = (size.0 as f32, size.1 as f32);
        let unit = 0.025;
        let place = entity::render::placement(bound, &entity::render::Look::default());
        let mut out = (Vec::new(), Vec::new(), Vec::new());
        // The game draws text without culling: seen from behind, mirrored.
        for side in [
            place,
            place * glam::Mat4::from_scale(Vec3::new(1.0, 1.0, -1.0)),
        ] {
            entity::painting::flat(
                &mut out,
                side,
                [-unit * w / 2.0, 0.0],
                [unit * (w / 2.0 + 1.0), unit * (h + 1.0)],
                [0.0, 0.0, 1.0, 1.0],
                &key,
                true,
            );
        }
        self.add_item_geometry(pos, &out.0, &out.1, &out.2);
        Ok(())
    }

    /// The block a row carries (LumilioCL local change, plan schematic-studio
    /// T35): meshed at `pos` as the schematic's blocks are (a chest in a
    /// minecart is the chest's own model), then moved by the row's facing
    /// and the block's steps. Air draws nothing.
    fn add_carried_block(
        &mut self,
        pos: BlockPosition,
        bound: &entity::bindings::Bound,
        carried: &entity::bindings::CarriedBlock,
    ) -> Result<()> {
        let Some(state) = bound.expand(&carried.state) else {
            return Ok(());
        };
        let block = entity::bindings::parse_state(&state);
        if block.is_air() {
            return Ok(());
        }
        let lift: f32 = carried
            .lift
            .as_deref()
            .and_then(|pattern| bound.expand(pattern))
            .and_then(|text| text.trim().parse().ok())
            .unwrap_or(0.0);
        let place = entity::render::facing(bound)
            * entity::render::steps_matrix(&carried.steps)
            * glam::Mat4::from_translation(Vec3::new(0.0, lift / 16.0, 0.0))
            * entity::render::steps_matrix(&carried.then);
        let first = self.mesh.vertex_count();
        self.add_block_geometry(pos, &block)?;
        let corner = Vec3::new(pos.x as f32 - 0.5, pos.y as f32 - 0.5, pos.z as f32 - 0.5);
        for vertex in &mut self.mesh.vertices[first..] {
            let local = Vec3::from(vertex.position) - corner;
            vertex.position = (place.transform_point3(local) + corner).into();
            vertex.normal = place
                .transform_vector3(Vec3::from(vertex.normal))
                .normalize_or_zero()
                .into();
        }
        Ok(())
    }

    /// Push entity geometry onto the mesh at `pos + extra`. Used for riders.
    fn add_offset_geometry(
        &mut self,
        pos: BlockPosition,
        extra: [f32; 3],
        vertices: &[Vertex],
        indices: &[u32],
        face_textures: &[entity::EntityFaceTexture],
    ) {
        let moved: Vec<Vertex> = vertices
            .iter()
            .map(|v| {
                let mut v = *v;
                for (axis, offset) in extra.iter().enumerate() {
                    v.position[axis] += offset;
                }
                v
            })
            .collect();
        self.add_item_geometry(pos, &moved, indices, face_textures);
    }

    /// Add inventory hologram above a container block.
    fn add_inventory_hologram(&mut self, pos: BlockPosition, inventory_str: &str) -> Result<()> {
        if let Some((mut verts, indices, mut face_textures, tex_data)) =
            entity::inventory::render_inventory_hologram(
                self.resource_pack,
                &self.model_resolver,
                inventory_str,
            )
        {
            // Generate unique texture key for this inventory
            let tex_key = format!("_inventory/{}_{}_{}", pos.x, pos.y, pos.z);

            // Store the composited texture
            self.dynamic_textures.insert(tex_key.clone(), tex_data);

            // Set the texture path on all face textures
            for ft in &mut face_textures {
                ft.texture = tex_key.clone();
            }

            // Offset vertices to world position
            for v in &mut verts {
                v.position[0] += pos.x as f32 - 0.5;
                v.position[1] += pos.y as f32 - 0.5;
                v.position[2] += pos.z as f32 - 0.5;
            }

            // Register texture and add geometry
            self.texture_refs.insert(tex_key);

            let base_vertex = self.mesh.vertex_count() as u32;
            let base_index = self.mesh.indices.len();

            for v in &verts {
                self.mesh.add_vertex(*v);
            }

            for &idx in &indices {
                self.mesh.indices.push(base_vertex + idx);
            }

            let mut idx_offset = base_index;
            for ft in &face_textures {
                let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
                self.face_textures.push(FaceTextureMapping {
                    vertex_start: face_v_start,
                    index_start: idx_offset,
                    texture_path: ft.texture.clone(),
                    is_transparent: ft.is_transparent,
                });
                idx_offset += 6;
            }
        }

        Ok(())
    }

    /// Add static particle marker quads (cross-quads for flames, smoke, etc.).
    /// Builds animated sprite sheets for particle textures and stores them as
    /// dynamic textures so the viewer can cycle frames automatically.
    fn add_particles(
        &mut self,
        pos: BlockPosition,
        source: &entity::particle::ParticleSource,
    ) -> Result<()> {
        // Build animated sprite sheets for particle textures (if not already built)
        for quad in &source.quads {
            if let Some(anim) = entity::particle::particle_anim_def(quad.texture) {
                if !self.dynamic_textures.contains_key(anim.key) {
                    if let Some(tex) =
                        entity::particle::build_particle_sprite_sheet(self.resource_pack, anim)
                    {
                        self.dynamic_textures.insert(anim.key.to_string(), tex);
                    }
                }
            }
        }

        let (vertices, indices, face_textures) =
            entity::particle::generate_particle_geometry(source);

        if vertices.is_empty() {
            return Ok(());
        }

        // Register texture refs using animated key when available
        for ft in &face_textures {
            let tex_key = entity::particle::particle_anim_def(&ft.texture)
                .map(|a| a.key.to_string())
                .unwrap_or_else(|| ft.texture.clone());
            self.texture_refs.insert(tex_key);
        }

        let base_vertex = self.mesh.vertex_count() as u32;
        let base_index = self.mesh.indices.len();

        for v in &vertices {
            let mut vertex = *v;
            vertex.position[0] += pos.x as f32 - 0.5;
            vertex.position[1] += pos.y as f32 - 0.5;
            vertex.position[2] += pos.z as f32 - 0.5;
            self.mesh.add_vertex(vertex);
        }

        for &idx in &indices {
            self.mesh.indices.push(base_vertex + idx);
        }

        let mut idx_offset = base_index;
        for ft in &face_textures {
            // Use animated texture key if this particle has an animation def
            let tex_key = entity::particle::particle_anim_def(&ft.texture)
                .map(|a| a.key.to_string())
                .unwrap_or_else(|| ft.texture.clone());
            let face_v_start = (idx_offset - base_index) as u32 / 6 * 4 + base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: idx_offset,
                texture_path: tex_key,
                is_transparent: ft.is_transparent,
            });
            idx_offset += 6;
        }

        Ok(())
    }

    /// Add pre-transformed item/overlay geometry at a world position.
    fn add_item_geometry(
        &mut self,
        pos: BlockPosition,
        item_verts: &[Vertex],
        item_indices: &[u32],
        item_faces: &[entity::EntityFaceTexture],
    ) {
        for ft in item_faces {
            self.texture_refs.insert(ft.texture.clone());
        }

        let item_base_vertex = self.mesh.vertex_count() as u32;
        let item_base_index = self.mesh.indices.len();

        for v in item_verts {
            let mut vertex = *v;
            vertex.position[0] += pos.x as f32 - 0.5;
            vertex.position[1] += pos.y as f32 - 0.5;
            vertex.position[2] += pos.z as f32 - 0.5;
            self.mesh.add_vertex(vertex);
        }

        for &idx in item_indices {
            self.mesh.indices.push(item_base_vertex + idx);
        }

        let mut item_idx_offset = item_base_index;
        for ft in item_faces {
            let face_v_start =
                (item_idx_offset - item_base_index) as u32 / 6 * 4 + item_base_vertex;
            self.face_textures.push(FaceTextureMapping {
                vertex_start: face_v_start,
                index_start: item_idx_offset,
                texture_path: ft.texture.clone(),
                is_transparent: ft.is_transparent,
            });
            item_idx_offset += 6;
        }
    }

    /// Add a resolved model to the mesh.
    fn add_model(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        resolved: &ResolvedModel,
    ) -> Result<()> {
        let model = &resolved.model;
        let transform = &resolved.transform;

        // Resolve textures for this model
        let resolved_textures = self.model_resolver.resolve_textures(model);

        // Greedy merging is only safe for single-element models. Multi-element
        // models (e.g. grass blocks, which have a base cube + a coplanar tinted
        // `*_overlay` cube) would merge each element into its own coplanar quad
        // and z-fight; those must go through the normal atlas path instead.
        let single_element = model.elements.len() == 1;

        // Process each element
        for element in &model.elements {
            self.add_element(
                pos,
                block,
                element,
                transform,
                &resolved_textures,
                single_element,
                model.uses_ambient_occlusion(),
            )?;
        }

        Ok(())
    }

    /// Check if an element/face is eligible for greedy merging.
    fn is_greedy_eligible(
        &self,
        element: &ModelElement,
        face: &ModelFace,
        transform: &BlockTransform,
    ) -> bool {
        // Must be a full cube element: from=[0,0,0], to=[16,16,16]
        const EPSILON: f32 = 0.001;
        if element.from[0].abs() > EPSILON
            || element.from[1].abs() > EPSILON
            || element.from[2].abs() > EPSILON
        {
            return false;
        }
        if (element.to[0] - 16.0).abs() > EPSILON
            || (element.to[1] - 16.0).abs() > EPSILON
            || (element.to[2] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // No element rotation
        if element.rotation.is_some() {
            return false;
        }

        // Block transform must be identity
        if !transform.is_identity() {
            return false;
        }

        // UV must cover full texture (default [0,0,16,16])
        let uv = face.uv_or_default();
        if uv[0].abs() > EPSILON
            || uv[1].abs() > EPSILON
            || (uv[2] - 16.0).abs() > EPSILON
            || (uv[3] - 16.0).abs() > EPSILON
        {
            return false;
        }

        // UV rotation must be 0
        if face.rotation != 0 {
            return false;
        }

        true
    }

    /// Add an element to the mesh.
    fn add_element(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        transform: &BlockTransform,
        resolved_textures: &std::collections::HashMap<String, String>,
        single_element: bool,
        model_ambient_occlusion: bool,
    ) -> Result<()> {
        // Compute lighting factor for this block position
        let is_emissive = self
            .light_map
            .map(|lm| lm.is_emissive(pos))
            .unwrap_or(false);
        // Ambient occlusion darkens this element only if the config, the block's
        // light, the element and its model (`ambientocclusion`) all allow it.
        let takes_ao = self.config.ambient_occlusion
            && !is_emissive
            && element.shade
            && model_ambient_occlusion;

        // Process each face. Iterate the fixed Direction::ALL order (not the
        // model's `faces` HashMap, whose iteration order is hash-random) so the
        // emitted vertex order — and thus the exported mesh — is deterministic.
        for direction in Direction::ALL.iter() {
            let face = match element.faces.get(direction) {
                Some(f) => f,
                None => continue,
            };
            // Transform the face direction by block rotation (for AO and cullface)
            let world_direction = direction.rotate_by_transform(transform.x, transform.y);

            // Check if face should be culled
            if let Some(cullface) = &face.cullface {
                if let Some(culler) = self.culler {
                    // Transform cullface direction to world space
                    let world_cullface = cullface.rotate_by_transform(transform.x, transform.y);
                    if culler.should_cull(pos, world_cullface) {
                        continue;
                    }
                }
            }

            // Resolve the texture reference
            let texture_path = self.resolve_face_texture(&face.texture, resolved_textures);
            self.texture_refs.insert(texture_path.clone());

            // Fast-path coverage stats: classify this surviving face.
            if stats_enabled() {
                use std::sync::atomic::Ordering::Relaxed;
                STAT_TOTAL_FACES.fetch_add(1, Relaxed);
                if single_element && self.is_greedy_eligible(element, face, transform) {
                    STAT_CUBE_FACES.fetch_add(1, Relaxed);
                    let lit = !takes_ao
                        || self
                            .culler
                            .map(|c| c.calculate_ao(pos, world_direction))
                            .unwrap_or([3, 3, 3, 3])
                            == [3, 3, 3, 3];
                    if lit {
                        STAT_CUBE_LIT_FACES.fetch_add(1, Relaxed);
                    }
                }
            }

            // Check if texture has transparency
            let is_transparent = self
                .resource_pack
                .get_texture(&texture_path)
                .map(|t| t.has_transparency())
                .unwrap_or(false);

            // Compute light factor for this face
            let light_factor = if is_emissive {
                1.0 // Emissive blocks are always fully bright
            } else if let Some(lm) = self.light_map {
                lm.face_brightness(pos, world_direction)
            } else {
                1.0 // No lighting → full brightness
            };

            // Quantized light level for greedy merge key (0-15)
            let light_key = if self.light_map.is_some() {
                (light_factor * 15.0).round() as u8
            } else {
                15
            };

            // Route to greedy mesher if eligible. We only merge fully-lit faces
            // (AO == [3,3,3,3]); AO'd faces fall through to the shared-atlas path
            // below, where per-block AO is correct. This avoids the per-AO-pattern
            // material/texture explosion that baking AO into greedy tiles caused,
            // and is the only practical greedy mode (full greedy produced
            // thousands of materials). Multi-element / overlay blocks (grass etc.)
            // are excluded via `single_element` to avoid coplanar z-fighting.
            if self.greedy.is_some()
                && single_element
                && self.is_greedy_eligible(element, face, transform)
            {
                // Compute per-vertex AO; only fully-lit faces are eligible to merge.
                let ao = if takes_ao {
                    self.culler
                        .map(|c| c.calculate_ao(pos, world_direction))
                        .unwrap_or([3, 3, 3, 3])
                } else {
                    [3, 3, 3, 3]
                };
                if ao == [3, 3, 3, 3] {
                    let mut base_color = self.config.tint_provider.get_tint(block, face.tintindex);
                    // Apply lighting to tint color before quantization
                    base_color[0] *= light_factor;
                    base_color[1] *= light_factor;
                    base_color[2] *= light_factor;
                    let key = FaceMergeKey {
                        texture: texture_path.clone(),
                        tint: quantize_color(base_color),
                        ao,
                        light: light_key,
                    };
                    self.greedy.as_mut().unwrap().add_face(
                        pos,
                        world_direction,
                        key,
                        is_transparent,
                    );
                    continue;
                }
                // AO'd face: fall through to the atlas path below.
            }

            // Detect glow overlay: shade:false, single face, no cullface.
            // These are decorative halo quads (e.g., repeater/comparator/torch glow).
            // Render them semi-transparent so they blend softly instead of appearing
            // as opaque panels (Minecraft uses bloom post-processing for the glow).
            let is_glow_overlay =
                !element.shade && element.faces.len() == 1 && face.cullface.is_none();
            let is_transparent = is_transparent || is_glow_overlay;

            // Track texture mapping for UV remapping
            let vertex_start = self.mesh.vertex_count() as u32;
            let index_start = self.mesh.indices.len();
            self.face_textures.push(FaceTextureMapping {
                vertex_start,
                index_start,
                texture_path,
                is_transparent,
            });

            // Calculate AO if enabled (use world direction for neighbor checks)
            // Skip AO for emissive blocks and elements with shade: false
            let ao_values = if takes_ao {
                self.culler.map(|c| c.calculate_ao(pos, world_direction))
            } else {
                None
            };

            // Glow overlays get reduced alpha for soft glow appearance
            let alpha_override = if is_glow_overlay { Some(0.4_f32) } else { None };

            // Generate face geometry (with lighting applied)
            self.add_face(
                pos,
                block,
                element,
                *direction,
                face,
                transform,
                ao_values,
                light_factor,
                alpha_override,
            )?;
        }

        Ok(())
    }

    /// Resolve a texture reference to a path.
    fn resolve_face_texture(
        &self,
        reference: &str,
        resolved_textures: &std::collections::HashMap<String, String>,
    ) -> String {
        if reference.starts_with('#') {
            let key = &reference[1..];
            resolved_textures
                .get(key)
                .cloned()
                .unwrap_or_else(|| "block/missing".to_string())
        } else {
            reference.to_string()
        }
    }

    /// Add a face to the mesh.
    fn add_face(
        &mut self,
        pos: BlockPosition,
        block: &InputBlock,
        element: &ModelElement,
        direction: Direction,
        face: &ModelFace,
        transform: &BlockTransform,
        ao_values: Option<[u8; 4]>,
        light_factor: f32,
        alpha_override: Option<f32>,
    ) -> Result<()> {
        // Use auto-UV calculation from element bounds when face has no explicit UV
        let uv = face.normalized_uv_auto(direction, &element.from, &element.to);

        // Get element bounds in normalized space.
        // Use raw coordinates directly — if from > to on any axis, the reversed
        // vertex positions create reversed winding (inside-out geometry, e.g. glow overlays).
        let from = element.normalized_from();
        let to = element.normalized_to();

        // For inverted bounds, the cross product of reversed edges flips the normal
        let bounds_inverted = from[0] > to[0] || from[1] > to[1] || from[2] > to[2];
        let normal = if bounds_inverted {
            let n = direction.normal();
            [-n[0], -n[1], -n[2]]
        } else {
            direction.normal()
        };

        // Generate the 4 vertices for this face
        let (positions, uvs) = self.generate_face_vertices(direction, from, to, uv, face.rotation);
        // A uvlock variant keeps the texture aligned to the world, not the model.
        let uvs = match transform.uvlock_matrix(direction) {
            Some(lock) => uvs.map(|[u, v]| {
                let p = lock * Vec3::new(u - 0.5, v - 0.5, 0.0);
                [p.x + 0.5, p.y + 0.5]
            }),
            None => uvs,
        };

        // Apply element rotation if present. The normal turns with the face (inverse
        // transpose, as rescale is not uniform), so a face rotated past 90° — the
        // 26.x Euler rotations — is lit from the side it actually faces.
        let (positions, normal) = if let Some(rot) = &element.rotation {
            let turned = rot.matrix().inverse().transpose() * Vec3::from(normal);
            (
                self.apply_element_rotation(&positions, rot),
                turned.normalize_or(Vec3::from(normal)).into(),
            )
        } else {
            (positions, normal)
        };

        // A 26.x shade override lights the face as if it faced that direction;
        // the renderer shades by the vertex normal (winding still decides culling).
        let normal = element
            .shade_direction_override
            .map_or(normal, |shade| shade.normal());

        // Apply block transform rotation
        let positions = self.apply_block_transform(&positions, transform);
        let normal = self.rotate_normal(normal, transform);

        // Translate to world position
        let offset = [pos.x as f32, pos.y as f32, pos.z as f32];

        // Get tint color from the tint provider based on block type and tint index
        let mut base_color = self.config.tint_provider.get_tint(block, face.tintindex);
        if let Some(alpha) = alpha_override {
            base_color[3] = alpha;
        }

        // Calculate per-vertex colors with AO and lighting
        let colors = if let Some(ao) = ao_values {
            let intensity = self.config.ao_intensity;
            [
                apply_ao_and_light(base_color, ao[0], intensity, light_factor),
                apply_ao_and_light(base_color, ao[1], intensity, light_factor),
                apply_ao_and_light(base_color, ao[2], intensity, light_factor),
                apply_ao_and_light(base_color, ao[3], intensity, light_factor),
            ]
        } else {
            let lit = [
                base_color[0] * light_factor,
                base_color[1] * light_factor,
                base_color[2] * light_factor,
                base_color[3],
            ];
            [lit; 4]
        };

        let v0 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[0][0] + offset[0],
                    positions[0][1] + offset[1],
                    positions[0][2] + offset[2],
                ],
                normal,
                uvs[0],
            )
            .with_color(colors[0]),
        );
        let v1 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[1][0] + offset[0],
                    positions[1][1] + offset[1],
                    positions[1][2] + offset[2],
                ],
                normal,
                uvs[1],
            )
            .with_color(colors[1]),
        );
        let v2 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[2][0] + offset[0],
                    positions[2][1] + offset[1],
                    positions[2][2] + offset[2],
                ],
                normal,
                uvs[2],
            )
            .with_color(colors[2]),
        );
        let v3 = self.mesh.add_vertex(
            Vertex::new(
                [
                    positions[3][0] + offset[0],
                    positions[3][1] + offset[1],
                    positions[3][2] + offset[2],
                ],
                normal,
                uvs[3],
            )
            .with_color(colors[3]),
        );

        // Use AO-aware quad triangulation to fix anisotropy
        if let Some(ao) = ao_values {
            self.mesh.add_quad_ao(v0, v1, v2, v3, ao);
        } else {
            self.mesh.add_quad(v0, v1, v2, v3);
        }

        Ok(())
    }

    /// Generate the 4 vertices for a face.
    /// Returns (positions, uvs) in CCW order.
    fn generate_face_vertices(
        &self,
        direction: Direction,
        from: [f32; 3],
        to: [f32; 3],
        uv: [f32; 4],
        rotation: i32,
    ) -> ([[f32; 3]; 4], [[f32; 2]; 4]) {
        let (u1, v1, u2, v2) = (uv[0], uv[1], uv[2], uv[3]);

        // Base UVs (before rotation)
        // glTF uses v=0 at top (same as Minecraft), so no V flip needed
        // UV order: top-left, top-right, bottom-right, bottom-left (CCW)
        let base_uvs = [[u1, v1], [u2, v1], [u2, v2], [u1, v2]];

        // Rotate UVs
        let uvs = self.rotate_uvs(base_uvs, rotation);

        // Generate positions based on direction
        let positions = match direction {
            Direction::Down => [
                [from[0], from[1], to[2]],
                [to[0], from[1], to[2]],
                [to[0], from[1], from[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::Up => [
                [from[0], to[1], from[2]],
                [to[0], to[1], from[2]],
                [to[0], to[1], to[2]],
                [from[0], to[1], to[2]],
            ],
            Direction::North => [
                [to[0], to[1], from[2]],
                [from[0], to[1], from[2]],
                [from[0], from[1], from[2]],
                [to[0], from[1], from[2]],
            ],
            Direction::South => [
                [from[0], to[1], to[2]],
                [to[0], to[1], to[2]],
                [to[0], from[1], to[2]],
                [from[0], from[1], to[2]],
            ],
            Direction::West => [
                [from[0], to[1], from[2]],
                [from[0], to[1], to[2]],
                [from[0], from[1], to[2]],
                [from[0], from[1], from[2]],
            ],
            Direction::East => [
                [to[0], to[1], to[2]],
                [to[0], to[1], from[2]],
                [to[0], from[1], from[2]],
                [to[0], from[1], to[2]],
            ],
        };

        (positions, uvs)
    }

    /// Rotate UV coordinates.
    fn rotate_uvs(&self, uvs: [[f32; 2]; 4], rotation: i32) -> [[f32; 2]; 4] {
        let steps = ((rotation / 90) % 4 + 4) % 4;
        let mut result = uvs;
        for _ in 0..steps {
            result = [result[3], result[0], result[1], result[2]];
        }
        result
    }

    /// Apply element rotation to positions.
    fn apply_element_rotation(
        &self,
        positions: &[[f32; 3]; 4],
        rotation: &crate::types::ElementRotation,
    ) -> [[f32; 3]; 4] {
        let origin = rotation.normalized_origin();
        let matrix = rotation.matrix();

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            // Translate to origin, rotate (and rescale), translate back
            let p = Vec3::new(pos[0] - origin[0], pos[1] - origin[1], pos[2] - origin[2]);
            let scaled = matrix * p;

            result[i] = [
                scaled.x + origin[0],
                scaled.y + origin[1],
                scaled.z + origin[2],
            ];
        }
        result
    }

    /// Apply block-level transform to positions.
    fn apply_block_transform(
        &self,
        positions: &[[f32; 3]; 4],
        transform: &BlockTransform,
    ) -> [[f32; 3]; 4] {
        if transform.is_identity() {
            return *positions;
        }

        // Negate angles: Minecraft rotations are clockwise from above (Y) and
        // from +X looking toward origin (X), but glam uses right-hand rule (CCW).
        let x_rot = Mat3::from_rotation_x((-transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((-transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let mut result = [[0.0; 3]; 4];
        for (i, pos) in positions.iter().enumerate() {
            let p = Vec3::new(pos[0], pos[1], pos[2]);
            let rotated = rotation_matrix * p;
            result[i] = [rotated.x, rotated.y, rotated.z];
        }
        result
    }

    /// Rotate a normal by the block transform.
    fn rotate_normal(&self, normal: [f32; 3], transform: &BlockTransform) -> [f32; 3] {
        if transform.is_identity() {
            return normal;
        }

        let x_rot = Mat3::from_rotation_x((-transform.x as f32).to_radians());
        let y_rot = Mat3::from_rotation_y((-transform.y as f32).to_radians());
        let rotation_matrix = y_rot * x_rot;

        let n = Vec3::new(normal[0], normal[1], normal[2]);
        let rotated = rotation_matrix * n;
        [rotated.x, rotated.y, rotated.z]
    }

    /// Emit merged quads from the greedy mesher into the mesh.
    /// Greedy faces use tiled UVs [0, width] x [0, height] and bypass the atlas.
    /// AO is baked into the tile texture (not vertex colors) for universal viewer support.
    pub(crate) fn emit_greedy_quads(&mut self) {
        let greedy = match self.greedy.take() {
            Some(g) => g,
            None => return,
        };

        let merged_quads = greedy.merge();

        for quad in &merged_quads {
            let positions = quad.world_positions();
            let normal = quad.direction.normal();

            // Convert quantized tint back to f32 color (no AO — it's baked into the texture)
            let base_color = [
                quad.tint[0] as f32 / 255.0,
                quad.tint[1] as f32 / 255.0,
                quad.tint[2] as f32 / 255.0,
                quad.tint[3] as f32 / 255.0,
            ];

            // Tiled UVs: [0, width] x [0, height] so texture repeats per block
            let w = quad.width as f32;
            let h = quad.height as f32;
            let uvs = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];

            // Track greedy face separately (bypasses atlas UV remapping)
            let vertex_start = self.mesh.vertex_count() as u32;
            let index_start = self.mesh.indices.len();
            self.greedy_face_textures.push(GreedyFaceMapping {
                vertex_start,
                index_start,
                texture_path: quad.texture.clone(),
                is_transparent: quad.is_transparent,
                ao: quad.ao,
            });

            let v0 = self
                .mesh
                .add_vertex(Vertex::new(positions[0], normal, uvs[0]).with_color(base_color));
            let v1 = self
                .mesh
                .add_vertex(Vertex::new(positions[1], normal, uvs[1]).with_color(base_color));
            let v2 = self
                .mesh
                .add_vertex(Vertex::new(positions[2], normal, uvs[2]).with_color(base_color));
            let v3 = self
                .mesh
                .add_vertex(Vertex::new(positions[3], normal, uvs[3]).with_color(base_color));

            // Use AO-aware triangulation even though colors are uniform,
            // to keep consistent winding with the AO baked into the texture
            if self.config.ambient_occlusion && quad.ao != [3, 3, 3, 3] {
                self.mesh.add_quad_ao(v0, v1, v2, v3, quad.ao);
            } else {
                self.mesh.add_quad(v0, v1, v2, v3);
            }
        }
    }

    /// Build the final meshes (opaque, cutout, and transparent) and atlas.
    /// Returns (opaque_mesh, cutout_mesh, transparent_mesh, atlas, greedy_materials, animated_exports).
    /// Render order: opaque first, then cutout (alpha-tested), then transparent (alpha-blended).
    /// Greedy materials have their own textures with REPEAT wrapping for tiling.
    /// Animated exports contain sprite sheets from dynamic textures (particles, etc.).
    ///
    /// If `pre_built_atlas` is `Some`, it is used directly instead of building a new atlas.
    /// Dynamic textures (banners, signs, skins) that are NOT in the pre-built atlas will be
    /// added to it via a supplemental atlas build pass.
    pub fn build(
        mut self,
        pre_built_atlas: Option<TextureAtlas>,
    ) -> Result<(
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
        TextureAtlas,
        Vec<GreedyMaterial>,
        Vec<super::AnimatedTextureExport>,
    )> {
        // Emit greedy-merged quads into the mesh before atlas building
        self.emit_greedy_quads();

        let atlas = self.build_atlas(pre_built_atlas)?;

        // Split faces into opaque/cutout/transparent layers, applying each face's
        // atlas UV transform during the copy (see `separate_by_transparency`).
        let (opaque_mesh, cutout_mesh, transparent_mesh) = self.separate_by_transparency(&atlas);

        // Build greedy materials: group greedy faces by texture path
        let greedy_materials = self.build_greedy_materials();

        let animated_exports = self.collect_dynamic_animated(&atlas);

        Ok((
            opaque_mesh,
            cutout_mesh,
            transparent_mesh,
            atlas,
            greedy_materials,
            animated_exports,
        ))
    }

    /// Collect animated-texture sprite-sheet exports for any dynamic textures
    /// (particles, banners, etc.) that ended up in the atlas.
    fn collect_dynamic_animated(&self, atlas: &TextureAtlas) -> Vec<super::AnimatedTextureExport> {
        let mut animated_exports = Vec::new();
        // Iterate in sorted key order so animated-texture exports (and thus the
        // glTF image/texture order) are deterministic — dynamic_textures is a hash map.
        let mut keys: Vec<&String> = self.dynamic_textures.keys().collect();
        keys.sort_unstable();
        for key in keys {
            let tex = &self.dynamic_textures[key];
            if tex.is_animated && tex.frame_count > 1 {
                if let Some(region) = atlas.get_region(key) {
                    let sprite_sheet_png = match tex.to_png() {
                        Ok(png) => png,
                        Err(_) => continue,
                    };
                    let anim = tex.animation.as_ref();
                    let frame_width = anim.and_then(|a| a.frame_width).unwrap_or(tex.width);
                    let frame_height = anim.and_then(|a| a.frame_height).unwrap_or(frame_width);
                    let frametime = anim.map(|a| a.frametime).unwrap_or(1);
                    let interpolate = anim.map(|a| a.interpolate).unwrap_or(false);
                    let frames = anim
                        .and_then(|a| a.frames.as_ref())
                        .map(|fs| fs.iter().map(|f| f.index).collect());
                    let atlas_x = (region.u_min * atlas.width as f32).round() as u32;
                    let atlas_y = (region.v_min * atlas.height as f32).round() as u32;
                    animated_exports.push(super::AnimatedTextureExport {
                        sprite_sheet_png,
                        frame_count: tex.frame_count,
                        frametime,
                        interpolate,
                        frames,
                        frame_width,
                        frame_height,
                        atlas_x,
                        atlas_y,
                    });
                }
            }
        }
        animated_exports
    }

    /// Build the texture atlas from `self.texture_refs` (+ dynamic textures),
    /// or augment a `pre_built_atlas` with any missing dynamic textures.
    fn build_atlas(&self, pre_built_atlas: Option<TextureAtlas>) -> Result<TextureAtlas> {
        Ok(if let Some(mut atlas) = pre_built_atlas {
            // Use pre-built atlas. Any dynamic textures not already in it need to be added.
            let mut missing_textures = Vec::new();
            for texture_ref in &self.texture_refs {
                if !atlas.contains(texture_ref) {
                    if let Some(texture) = self.dynamic_textures.get(texture_ref) {
                        missing_textures.push((texture_ref.clone(), texture.first_frame()));
                    } else if let Some(texture) = self.resource_pack.get_texture(texture_ref) {
                        missing_textures.push((texture_ref.clone(), texture.first_frame()));
                    }
                }
            }
            if !missing_textures.is_empty() {
                // Rebuild atlas with both pre-built and missing textures
                let mut atlas_builder =
                    AtlasBuilder::new(self.config.atlas_max_size, self.config.atlas_padding);
                // Re-add all existing textures from the pre-built atlas
                for texture_ref in atlas.regions.keys() {
                    if let Some(texture) = self.resource_pack.get_texture(texture_ref) {
                        atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
                    }
                }
                // Add dynamic/missing textures
                for (path, tex) in missing_textures {
                    atlas_builder.add_texture(path, tex);
                }
                atlas = atlas_builder.build()?;
            }
            atlas
        } else {
            // Build texture atlas from scratch (only for non-greedy faces)
            let mut atlas_builder =
                AtlasBuilder::new(self.config.atlas_max_size, self.config.atlas_padding);

            // Atlas packing order (and thus determinism) is handled inside
            // AtlasBuilder::build, which sorts by height + path — so add order
            // here doesn't matter.
            for texture_ref in &self.texture_refs {
                // Check dynamic textures first, then resource pack
                if let Some(texture) = self.dynamic_textures.get(texture_ref) {
                    atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
                } else if let Some(texture) = self.resource_pack.get_texture(texture_ref) {
                    atlas_builder.add_texture(texture_ref.clone(), texture.first_frame());
                }
            }

            // Always add a "__missing__" sentinel tile so faces whose texture
            // couldn't be resolved have *something* to sample — otherwise their
            // UVs stay in local [0,1] space and sample the entire atlas, which
            // renders as a tiny collapsed strip of the full atlas on the block.
            atlas_builder.add_texture(MISSING_TEXTURE_KEY.to_string(), make_missing_texture());

            atlas_builder.build()?
        })
    }

    /// Union only the cheap metadata (texture refs + dynamic textures) from a set
    /// of per-chunk partials into this builder — *not* the vertex/index buffers.
    /// Used by [`build_from_partials`](Self::build_from_partials) so the atlas can
    /// be built without first copying tens of millions of vertices into one mesh.
    pub(crate) fn merge_metadata_only(&mut self, partials: &[PartialMesh]) {
        for p in partials {
            for t in &p.texture_refs {
                if !self.texture_refs.contains(t) {
                    self.texture_refs.insert(t.clone());
                }
            }
            for (k, v) in &p.dynamic_textures {
                self.dynamic_textures
                    .entry(k.clone())
                    .or_insert_with(|| v.clone());
            }
        }
    }

    /// Like [`build`](Self::build), but sources geometry from a list of per-chunk
    /// [`PartialMesh`]es instead of a single merged mesh. The layer-split reads
    /// each partial's own buffers directly, so the ~N-million-vertex merge copy is
    /// skipped entirely (the split is the only vertex copy). Only valid when greedy
    /// meshing is disabled (the parallel path guarantees this), so there are no
    /// greedy materials to emit.
    ///
    /// `merge_metadata_only` must have been called with these `partials` first so
    /// the atlas covers every texture they reference.
    pub(crate) fn build_from_partials(
        self,
        partials: Vec<PartialMesh>,
        pre_built_atlas: Option<TextureAtlas>,
    ) -> Result<(
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
        TextureAtlas,
        Vec<GreedyMaterial>,
        Vec<super::AnimatedTextureExport>,
    )> {
        let _prof = std::env::var("MESHER_PROFILE").is_ok();
        let _t = super::prof_now();
        let atlas = self.build_atlas(pre_built_atlas)?;
        if _prof {
            eprintln!(
                "MPROFILE\t  build.atlas\t{}",
                _t.map_or(0, |t| t.elapsed().as_micros())
            );
        }
        let _t = super::prof_now();

        let mut opaque_mesh = crate::mesh_output::MeshLayer::new();
        let mut cutout_mesh = crate::mesh_output::MeshLayer::new();
        let mut transparent_mesh = crate::mesh_output::MeshLayer::new();
        // Pre-size the layer buffers: in the common (mostly-opaque) case nearly
        // all vertices land in `opaque_mesh`, so reserving the grand total avoids
        // reallocations without meaningfully over-allocating the small layers.
        let total_v: usize = partials.iter().map(|p| p.mesh.vertices.len()).sum();
        let total_i: usize = partials.iter().map(|p| p.mesh.indices.len()).sum();
        opaque_mesh.reserve(total_v, total_i);

        // Copy the pack reference so the parallel split closures can borrow it
        // (for per-texture translucency lookup) without capturing `self`.
        let pack = self.resource_pack;

        // Split each partial into its OWN per-chunk SoA layers in parallel, then
        // concat. The split is per-vertex compute (UV transform + layer classify),
        // ~40x above the memory-bandwidth floor, so it parallelizes well; the
        // final concat is the only bandwidth-bound copy.
        #[cfg(not(target_arch = "wasm32"))]
        let per_chunk: Vec<(
            crate::mesh_output::MeshLayer,
            crate::mesh_output::MeshLayer,
            crate::mesh_output::MeshLayer,
        )> = {
            use rayon::prelude::*;
            partials
                .par_iter()
                .map(|p| {
                    let mut op = crate::mesh_output::MeshLayer::new();
                    op.reserve(p.mesh.vertices.len(), p.mesh.indices.len());
                    let mut cut = crate::mesh_output::MeshLayer::new();
                    let mut tr = crate::mesh_output::MeshLayer::new();
                    split_faces_into(
                        &p.mesh.vertices,
                        &p.mesh.indices,
                        &p.face_textures,
                        &atlas,
                        pack,
                        &mut op,
                        &mut cut,
                        &mut tr,
                    );
                    (op, cut, tr)
                })
                .collect()
        };
        #[cfg(target_arch = "wasm32")]
        let per_chunk: Vec<(
            crate::mesh_output::MeshLayer,
            crate::mesh_output::MeshLayer,
            crate::mesh_output::MeshLayer,
        )> = partials
            .iter()
            .map(|p| {
                let mut op = crate::mesh_output::MeshLayer::new();
                let mut cut = crate::mesh_output::MeshLayer::new();
                let mut tr = crate::mesh_output::MeshLayer::new();
                split_faces_into(
                    &p.mesh.vertices,
                    &p.mesh.indices,
                    &p.face_textures,
                    &atlas,
                    pack,
                    &mut op,
                    &mut cut,
                    &mut tr,
                );
                (op, cut, tr)
            })
            .collect();
        for (op, cut, tr) in &per_chunk {
            opaque_mesh.merge(op);
            cutout_mesh.merge(cut);
            transparent_mesh.merge(tr);
        }
        if _prof {
            eprintln!(
                "MPROFILE\t  build.split\t{}",
                _t.map_or(0, |t| t.elapsed().as_micros())
            );
        }
        let _t = super::prof_now();

        // Greedy materials: each partial already merged & emitted its greedy
        // quads (per-chunk, before into_partial), so just accumulate them across
        // partials and finalize. (Faces are not merged across chunk boundaries —
        // a small, invisible vertex overhead.)
        let mut greedy_map = std::collections::HashMap::new();
        for p in &partials {
            accumulate_greedy_materials(
                &p.mesh.vertices,
                &p.mesh.indices,
                &p.greedy_face_textures,
                &mut greedy_map,
            );
        }
        let greedy_materials = self.finalize_greedy_materials(greedy_map);
        if _prof {
            eprintln!(
                "MPROFILE\t  build.greedy\t{}",
                _t.map_or(0, |t| t.elapsed().as_micros())
            );
        }
        let _t = super::prof_now();

        let animated_exports = self.collect_dynamic_animated(&atlas);
        if _prof {
            eprintln!(
                "MPROFILE\t  build.animated\t{}",
                _t.map_or(0, |t| t.elapsed().as_micros())
            );
        }
        Ok((
            opaque_mesh,
            cutout_mesh,
            transparent_mesh,
            atlas,
            greedy_materials,
            animated_exports,
        ))
    }

    /// Build per-texture GreedyMaterial meshes from greedy faces.
    /// Groups by (texture_path, ao_pattern) so each AO variant gets its own
    /// baked texture with AO darkening applied at the pixel level.
    fn build_greedy_materials(&self) -> Vec<GreedyMaterial> {
        let mut material_map = std::collections::HashMap::new();
        accumulate_greedy_materials(
            &self.mesh.vertices,
            &self.mesh.indices,
            &self.greedy_face_textures,
            &mut material_map,
        );
        self.finalize_greedy_materials(material_map)
    }

    /// Turn an accumulated `(texture, AO) -> (opaque, transparent)` map into final
    /// [`GreedyMaterial`]s, baking AO into the tile texture where AO isn't full.
    fn finalize_greedy_materials(
        &self,
        material_map: std::collections::HashMap<(String, [u8; 4]), (Mesh, Mesh)>,
    ) -> Vec<GreedyMaterial> {
        let ao_intensity = self.config.ao_intensity;

        material_map
            .into_iter()
            .map(|((texture_path, ao), (opaque_mesh, transparent_mesh))| {
                let texture_png = if ao == [3, 3, 3, 3] {
                    // No AO darkening — use original tile texture
                    self.resource_pack
                        .get_texture(&texture_path)
                        .and_then(|t| t.first_frame().to_png().ok())
                        .unwrap_or_default()
                } else {
                    // Bake AO gradient into tile texture pixels
                    self.resource_pack
                        .get_texture(&texture_path)
                        .map(|t| {
                            let frame = t.first_frame();
                            bake_ao_into_tile(
                                &frame.pixels,
                                frame.width,
                                frame.height,
                                ao,
                                ao_intensity,
                            )
                        })
                        .unwrap_or_default()
                };
                GreedyMaterial {
                    texture_path,
                    opaque_mesh,
                    transparent_mesh,
                    texture_png,
                }
            })
            .collect()
    }

    /// Separate the mesh into opaque, cutout, and transparent parts.
    /// - Opaque: no transparency at all
    /// - Cutout: binary alpha (texture has transparency but vertex alpha ≈ 1.0) — uses MASK mode
    /// - Transparent: semi-transparent (vertex alpha < 1.0, e.g. water) — uses BLEND mode
    fn separate_by_transparency(
        &self,
        atlas: &TextureAtlas,
    ) -> (
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
        crate::mesh_output::MeshLayer,
    ) {
        let mut opaque_mesh = crate::mesh_output::MeshLayer::new();
        let mut cutout_mesh = crate::mesh_output::MeshLayer::new();
        let mut transparent_mesh = crate::mesh_output::MeshLayer::new();
        split_faces_into(
            &self.mesh.vertices,
            &self.mesh.indices,
            &self.face_textures,
            atlas,
            self.resource_pack,
            &mut opaque_mesh,
            &mut cutout_mesh,
            &mut transparent_mesh,
        );
        (opaque_mesh, cutout_mesh, transparent_mesh)
    }
}

/// Copy greedy-merged faces from one buffer into per-`(texture, AO)` material
/// meshes, accumulating into `material_map`. Shared by the sequential build and
/// the parallel per-partial build so greedy works on both paths.
fn accumulate_greedy_materials(
    vertices: &[Vertex],
    indices: &[u32],
    greedy_face_textures: &[GreedyFaceMapping],
    material_map: &mut std::collections::HashMap<(String, [u8; 4]), (Mesh, Mesh)>,
) {
    for face_mapping in greedy_face_textures {
        let vstart = face_mapping.vertex_start as usize;
        let istart = face_mapping.index_start;

        if vstart + 4 > vertices.len() || istart + 6 > indices.len() {
            continue;
        }

        let key = (face_mapping.texture_path.clone(), face_mapping.ao);
        let (opaque_mesh, transparent_mesh) = material_map
            .entry(key)
            .or_insert_with(|| (Mesh::new(), Mesh::new()));

        let target_mesh = if face_mapping.is_transparent {
            transparent_mesh
        } else {
            opaque_mesh
        };

        let orig_v0 = face_mapping.vertex_start;
        let v0 = target_mesh.add_vertex(vertices[vstart]);
        let v1 = target_mesh.add_vertex(vertices[vstart + 1]);
        let v2 = target_mesh.add_vertex(vertices[vstart + 2]);
        let v3 = target_mesh.add_vertex(vertices[vstart + 3]);

        for tri in 0..2 {
            let base = istart + tri * 3;
            let i0 = indices[base];
            let i1 = indices[base + 1];
            let i2 = indices[base + 2];
            let new_i0 = match i0 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            let new_i1 = match i1 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            let new_i2 = match i2 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            target_mesh.add_triangle(new_i0, new_i1, new_i2);
        }
    }
}

/// Split one buffer of faces into opaque/cutout/transparent layer meshes,
/// applying each face's atlas UV transform during the copy. Appends into the
/// provided meshes so it can be called once per source buffer (a single merged
/// mesh, or each per-chunk partial directly — avoiding a merge copy).
///
/// The atlas region is memoized across runs of same-textured faces (strong
/// locality) and always borrowed, never cloned.
fn split_faces_into(
    vertices: &[Vertex],
    indices: &[u32],
    face_textures: &[FaceTextureMapping],
    atlas: &TextureAtlas,
    resource_pack: &ResourcePack,
    opaque_mesh: &mut crate::mesh_output::MeshLayer,
    cutout_mesh: &mut crate::mesh_output::MeshLayer,
    transparent_mesh: &mut crate::mesh_output::MeshLayer,
) {
    let missing_region = atlas.get_region(MISSING_TEXTURE_KEY);
    let mut warned_missing: HashSet<String> = HashSet::new();
    let mut last_path: Option<&str> = None;
    let mut last_region = None;
    // Memoized alongside the atlas region: does this texture have intermediate
    // alpha (slime, honey, stained glass) -> must alpha-BLEND, not alpha-test.
    let mut last_translucent = false;

    // Process each face using tracked index positions (O(n) instead of O(n²))
    for face_mapping in face_textures {
        let vstart = face_mapping.vertex_start as usize;
        let istart = face_mapping.index_start;

        // Get the 4 vertices for this face
        if vstart + 4 > vertices.len() || istart + 6 > indices.len() {
            continue;
        }

        if last_path != Some(face_mapping.texture_path.as_str()) {
            last_region = match atlas.get_region(&face_mapping.texture_path) {
                Some(r) => Some(r),
                None => {
                    if warned_missing.insert(face_mapping.texture_path.clone()) {
                        eprintln!(
                            "Warning: no atlas region for texture '{}' — falling back to missing-texture tile",
                            face_mapping.texture_path,
                        );
                    }
                    missing_region
                }
            };
            last_path = Some(face_mapping.texture_path.as_str());
            last_translucent = resource_pack
                .get_texture(&face_mapping.texture_path)
                .map(|t| t.has_translucency())
                .unwrap_or(false);
        }
        let region = last_region;

        let target_mesh = if !face_mapping.is_transparent {
            &mut *opaque_mesh
        } else {
            // Distinguish cutout (alpha-test) from blend (translucent). A face
            // blends if EITHER its texture has intermediate alpha (slime, honey,
            // stained glass) OR a vertex is tinted semi-transparent. Otherwise
            // its alpha is binary (leaves, glass holes) -> cutout.
            let has_blend_alpha = (0..4).any(|i| vertices[vstart + i].color[3] < 0.99);
            if last_translucent || has_blend_alpha {
                &mut *transparent_mesh
            } else {
                &mut *cutout_mesh
            }
        };

        // Copy the 4 vertices straight into the target SoA layer, applying the
        // atlas UV transform during the copy (fused — no separate remap pass, and
        // no AoS Vertex/Mesh intermediate).
        let attrs = |i: usize| {
            let v = &vertices[i];
            let uv = match region {
                Some(region) => region.transform_uv(v.uv[0], v.uv[1]),
                None => v.uv,
            };
            (v.position, v.normal, uv, v.color)
        };
        let orig_v0 = face_mapping.vertex_start;
        let (p, n, u, c) = attrs(vstart);
        let v0 = target_mesh.push_vertex(p, n, u, c);
        let (p, n, u, c) = attrs(vstart + 1);
        let v1 = target_mesh.push_vertex(p, n, u, c);
        let (p, n, u, c) = attrs(vstart + 2);
        let v2 = target_mesh.push_vertex(p, n, u, c);
        let (p, n, u, c) = attrs(vstart + 3);
        let v3 = target_mesh.push_vertex(p, n, u, c);

        // Directly read the 6 indices (2 triangles) from the tracked position
        for tri in 0..2 {
            let base = istart + tri * 3;
            let i0 = indices[base];
            let i1 = indices[base + 1];
            let i2 = indices[base + 2];
            let new_i0 = match i0 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            let new_i1 = match i1 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            let new_i2 = match i2 - orig_v0 {
                0 => v0,
                1 => v1,
                2 => v2,
                _ => v3,
            };
            target_mesh.indices.push(new_i0);
            target_mesh.indices.push(new_i1);
            target_mesh.indices.push(new_i2);
        }
    }
}

/// Apply AO and lighting to a color.
/// ao_level: 0-3 (0=darkest, 3=brightest)
/// intensity: AO intensity (0.0-1.0)
/// light_factor: lighting brightness multiplier (0.0-1.0)
fn apply_ao_and_light(
    color: [f32; 4],
    ao_level: u8,
    intensity: f32,
    light_factor: f32,
) -> [f32; 4] {
    let ao_brightness = 1.0 - intensity * (1.0 - ao_level as f32 / 3.0);
    let combined = ao_brightness * light_factor;
    [
        color[0] * combined,
        color[1] * combined,
        color[2] * combined,
        color[3],
    ]
}

/// Bake ambient occlusion into a tile texture's pixels.
///
/// Creates a new PNG where each pixel is darkened according to a bilinear
/// interpolation of the 4 corner AO values. The tile repeats with REPEAT
/// wrapping, so each block in a merged quad independently shows the AO gradient.
///
/// AO corner mapping (image coordinates, row 0 = top):
///   (0,0)=AO[0]  (w,0)=AO[1]
///   (0,h)=AO[3]  (w,h)=AO[2]
///
/// In glTF, UV (0,0) = top-left of image. So emit_greedy_quads maps:
///   vertex 0 UV(0,0) → image top-left    → AO[0]
///   vertex 1 UV(w,0) → image top-right   → AO[1]
///   vertex 2 UV(w,h) → image bottom-right→ AO[2]
///   vertex 3 UV(0,h) → image bottom-left → AO[3]
fn bake_ao_into_tile(
    pixels: &[u8],
    width: u32,
    height: u32,
    ao: [u8; 4],
    intensity: f32,
) -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, pixels.to_vec())
            .expect("Failed to create image buffer for AO baking");

    let w = width.max(1) as f32;
    let h = height.max(1) as f32;

    // Precompute brightness for each AO level
    let brightness: [f32; 4] =
        std::array::from_fn(|i| 1.0 - intensity * (1.0 - ao[i] as f32 / 3.0));

    for row in 0..height {
        for col in 0..width {
            // Normalized position in image space (row 0 = top)
            let cx = col as f32 / (w - 1.0).max(1.0); // 0=left, 1=right
            let cy = row as f32 / (h - 1.0).max(1.0); // 0=top, 1=bottom

            // Bilinear interpolation of corner brightness values
            // Image corners: top-left=AO[0], top-right=AO[1], bottom-left=AO[3], bottom-right=AO[2]
            let b = (1.0 - cx) * (1.0 - cy) * brightness[0]
                + cx * (1.0 - cy) * brightness[1]
                + (1.0 - cx) * cy * brightness[3]
                + cx * cy * brightness[2];

            let pixel = img.get_pixel_mut(col, row);
            pixel[0] = (pixel[0] as f32 * b).round().min(255.0) as u8;
            pixel[1] = (pixel[1] as f32 * b).round().min(255.0) as u8;
            pixel[2] = (pixel[2] as f32 * b).round().min(255.0) as u8;
            // Alpha unchanged
        }
    }

    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .expect("Failed to encode AO-baked tile as PNG");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rotate_uvs() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

        // 0 degrees - no change
        assert_eq!(builder.rotate_uvs(uvs, 0), uvs);

        // 90 degrees
        let rotated_90 = builder.rotate_uvs(uvs, 90);
        assert_eq!(rotated_90[0], uvs[3]);
        assert_eq!(rotated_90[1], uvs[0]);

        // 180 degrees
        let rotated_180 = builder.rotate_uvs(uvs, 180);
        assert_eq!(rotated_180[0], uvs[2]);
        assert_eq!(rotated_180[2], uvs[0]);
    }

    fn full_cube_element() -> ModelElement {
        ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: None,
            shade: true,
            shade_direction_override: None,
            faces: HashMap::new(),
        }
    }

    fn full_face() -> ModelFace {
        ModelFace {
            uv: None, // defaults to [0,0,16,16]
            texture: "#all".to_string(),
            cullface: Some(Direction::Up),
            rotation: 0,
            tintindex: -1,
        }
    }

    #[test]
    fn test_greedy_eligible_full_cube() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_partial_element() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        // Slab-like element (half height)
        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 8.0, 16.0],
            rotation: None,
            shade: true,
            shade_direction_override: None,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_block() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = full_face();
        let rotated = BlockTransform::new(0, 90, false);

        assert!(!builder.is_greedy_eligible(&element, &face, &rotated));
    }

    #[test]
    fn test_greedy_ineligible_custom_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: Some([0.0, 0.0, 8.0, 8.0]), // Half-size UV
            texture: "#all".to_string(),
            cullface: None,
            rotation: 0,
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_rotated_uv() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = full_cube_element();
        let face = ModelFace {
            uv: None,
            texture: "#all".to_string(),
            cullface: None,
            rotation: 90, // Rotated UV
            tintindex: -1,
        };
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }

    #[test]
    fn test_greedy_ineligible_element_rotation() {
        let pack = ResourcePack::new();
        let config = MesherConfig::default();
        let builder = MeshBuilder::new(&pack, &config, None, None, None);

        let element = ModelElement {
            from: [0.0, 0.0, 0.0],
            to: [16.0, 16.0, 16.0],
            rotation: Some(crate::types::ElementRotation::single(
                [8.0, 8.0, 8.0],
                crate::types::Axis::Y,
                22.5,
                false,
            )),
            shade: true,
            shade_direction_override: None,
            faces: HashMap::new(),
        };
        let face = full_face();
        let identity = BlockTransform::default();

        assert!(!builder.is_greedy_eligible(&element, &face, &identity));
    }
}
