use lumilio_block_assets::{BakedPack, EntityLayers};
use nucleation::UniversalSchematic;
use nucleation::meshing::{EntityInstances, MeshConfig, MeshOutput, ResourcePackSource};
use nucleation::rendering::{
    CameraConfig, CellMark, Clip, FocusMask, FocusMode, GpuRenderer, MAX_FOCUS_SIDE, SelectionBox,
};

use crate::pick::{Grid, camera_ray, march, project};
use crate::{SceneError, View};

/// The block under a pixel (plan schematic-studio T10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked {
    /// Schematic coordinates, as the inspector and Litematica show them.
    pub position: [i32; 3],
    /// The block state, `minecraft:oak_stairs[facing=east,…]`.
    pub state: String,
}

/// Cells a click passes through: nothing is drawn there.
fn is_air(name: &str) -> bool {
    matches!(
        name,
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "minecraft:structure_void"
    )
}

/// Which blocks the picture keeps in their own colour (plan
/// schematic-studio T31, D30).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Focus {
    /// Every block.
    All,
    /// Blocks with this id in colour; the rest greyed and washed out.
    Fade(String),
    /// Only blocks with this id.
    Only(String),
}

/// An axis the picture can be sliced along.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliceAxis {
    X,
    Y,
    Z,
}

impl SliceAxis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    /// Its index into a position: 0, 1 or 2.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    fn mesher(self) -> nucleation::meshing::Axis {
        match self {
            Self::X => nucleation::meshing::Axis::X,
            Self::Y => nucleation::meshing::Axis::Y,
            Self::Z => nucleation::meshing::Axis::Z,
        }
    }
}

/// The layers drawn while slicing (plan schematic-studio T25): `layer` and
/// those below it along `axis`, in schematic coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slice {
    pub axis: SliceAxis,
    pub layer: i32,
}

/// One rendered image.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, blue first.
    pub bgra: Vec<u8>,
}

/// In first-person view, entities farther than this many blocks are not drawn. The orbit view
/// fits the whole build, so it draws every entity in the frustum.
pub const ENTITY_DISTANCE: f32 = 96.0;

/// A parsed, meshed schematic with a renderer sized for the last frame drawn.
pub struct Scene {
    meshes: Vec<MeshOutput>,
    /// The schematic's entities, one model per look and one instance per entity (plan
    /// schematic-studio T8).
    entities: EntityInstances,
    renderer: GpuRenderer,
    undrawable: Vec<String>,
    /// Kept for picking: the grid the clicked ray walks.
    schematic: UniversalSchematic,
    /// What the picture keeps in colour; a pick passes through hidden blocks.
    focus: Focus,
    /// The focus mask and mode, kept to apply again to a renderer made for
    /// a new size.
    mask: Option<(FocusMask, FocusMode)>,
    /// The outlined block (plan schematic-studio T32), kept the same way.
    mark: Option<CellMark>,
    /// The pack the blocks were meshed with, to mesh them again for a slice.
    source: ResourcePackSource,
    /// The axis the mesh keeps its faces along, if it was made for a slice.
    meshed_along: Option<SliceAxis>,
    slice: Option<Slice>,
    selection: Option<SelectionBox>,
}

/// Converts `schematic`'s block IDs to the version `pack` was baked from, when both are known.
/// A file that names no data version (an older Litematica's) is left as it is: converting
/// would assume Nucleation's canonical version and could rename blocks the file never had.
/// Returns whether it converted.
pub(crate) fn to_pack_version(schematic: &mut UniversalSchematic, pack: &BakedPack) -> bool {
    let known = schematic
        .metadata
        .source_data_version
        .or(schematic.metadata.mc_version)
        .is_some();
    match pack.meta.data_version.and_then(|v| i32::try_from(v).ok()) {
        Some(version) if known => {
            schematic.convert_to_data_version(version);
            true
        }
        _ => false,
    }
}

impl Scene {
    /// Parses a `.litematic` and meshes it with a baked pack's models and textures, and the same
    /// game version's entity layers (`entities`, from `entities.dat.z`) for block entities and
    /// mobs; without them those are left out. Blocks: call it off the UI thread.
    ///
    /// When the pack was baked from a game JAR, block IDs are first converted to that game
    /// version, so a schematic saved before a rename (`chain` → `iron_chain`) still finds its
    /// models; a file that names no data version is not converted.
    pub fn load(
        schematic: &[u8],
        pack: &BakedPack,
        entities: EntityLayers,
    ) -> Result<Self, SceneError> {
        let mut schematic = nucleation::formats::litematic::from_litematic(schematic)
            .map_err(|e| SceneError::Parse(e.to_string()))?;
        to_pack_version(&mut schematic, pack);
        Self::from_schematic(schematic, pack, entities)
    }

    /// Block IDs in the schematic that this pack cannot draw at all. They are left out of the
    /// picture, so the host should say so.
    pub fn undrawable_blocks(&self) -> &[String] {
        &self.undrawable
    }

    /// Entity IDs in the schematic that nothing draws: the binding table has no row for them, or
    /// this game version has no model layer for their row. Left out like undrawable blocks.
    pub fn undrawable_entities(&self) -> &[String] {
        &self.entities.undrawable
    }

    /// How many entities the picture draws, before culling.
    pub fn entity_count(&self) -> usize {
        self.entities.instances.len()
    }

    /// A useful initial flight position, just outside the build at standing eye height.
    pub fn explore_position(&self) -> [f32; 3] {
        let (min, max) = self.bounds();
        if (0..3)
            .any(|axis| !min[axis].is_finite() || !max[axis].is_finite() || min[axis] > max[axis])
        {
            return [0., 1.62, 3.];
        }
        [(min[0] + max[0]) * 0.5, min[1] + 1.62, max[2] + 3.0]
    }

    pub(crate) fn from_schematic(
        schematic: UniversalSchematic,
        pack: &BakedPack,
        entities: EntityLayers,
    ) -> Result<Self, SceneError> {
        if pack.assets.blockstates.is_empty() {
            return Err(SceneError::Pack("the pack has no blocks".into()));
        }
        let source = ResourcePackSource::from_baked(pack, entities);
        let undrawable = schematic.undrawable_blocks(&source);
        // Nucleation src/rendering/gpu.rs (MIT; see ATTRIBUTIONS.md) consumes
        // only opaque/cutout/transparent layers at this revision, not the
        // mesher's separate greedy_materials. Enabling greedy meshing makes
        // ordinary solid cubes disappear. Keep native rendering complete.
        let config = MeshConfig::default();
        // Blocks as one mesh; entities as instances of one model per look, drawn per model in
        // one call and culled each frame.
        let (mesh, entities) = schematic
            .to_mesh_with_entity_instances(&source, &config)
            .map_err(|e| SceneError::Mesh(e.to_string()))?;
        let meshes = vec![mesh];
        // Creating it here finds out about a missing GPU before the first frame is asked for.
        let renderer = Self::renderer(&meshes, &entities, 640, 480)?;
        Ok(Self {
            meshes,
            entities,
            renderer,
            undrawable,
            schematic,
            focus: Focus::All,
            mask: None,
            mark: None,
            source,
            meshed_along: None,
            slice: None,
            selection: None,
        })
    }

    /// The schematic's cells, both corners included: the range a slice's
    /// layer and a selection move in.
    pub fn cell_bounds(&self) -> ([i32; 3], [i32; 3]) {
        let bounds = self.schematic.get_bounding_box();
        (
            [bounds.min.0, bounds.min.1, bounds.min.2],
            [bounds.max.0, bounds.max.1, bounds.max.2],
        )
    }

    /// Draws only `slice`'s layer and those below it, or every layer, from
    /// the next frame on (plan schematic-studio T25). The first slice along
    /// an axis meshes the blocks again with their faces kept along it, so
    /// the cut shows each layer's tops; stepping through layers after that
    /// changes a uniform only.
    pub fn set_slice(&mut self, slice: Option<Slice>) -> Result<(), SceneError> {
        if let Some(axis) = slice.map(|slice| slice.axis)
            && self.meshed_along != Some(axis)
        {
            let config = MeshConfig {
                keep_faces_along: Some(axis.mesher()),
                ..MeshConfig::default()
            };
            let (mesh, entities) = self
                .schematic
                .to_mesh_with_entity_instances(&self.source, &config)
                .map_err(|e| SceneError::Mesh(e.to_string()))?;
            self.meshes = vec![mesh];
            self.entities = entities;
            let (width, height) = (self.renderer.width, self.renderer.height);
            self.renderer = Self::renderer(&self.meshes, &self.entities, width, height)?;
            self.meshed_along = Some(axis);
            self.apply_focus();
        }
        self.slice = slice;
        self.renderer.set_clip(self.clip());
        Ok(())
    }

    fn clip(&self) -> Option<Clip> {
        self.slice.map(|slice| Clip {
            axis: slice.axis.index() as u8,
            layer: slice.layer,
        })
    }

    /// The blocks (not air) from `min` to `max`, both included, by id, most
    /// first: a slice's layer or a selection (plan schematic-studio T25).
    pub fn blocks_in(&self, min: [i32; 3], max: [i32; 3]) -> Vec<(String, u64)> {
        let (low, high) = self.cell_bounds();
        let (min, max) = (
            [0, 1, 2].map(|axis| min[axis].min(max[axis]).max(low[axis])),
            [0, 1, 2].map(|axis| min[axis].max(max[axis]).min(high[axis])),
        );
        let mut counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
        for x in min[0]..=max[0] {
            for y in min[1]..=max[1] {
                for z in min[2]..=max[2] {
                    if let Some(block) = self.schematic.get_block(x, y, z)
                        && !is_air(&block.name)
                    {
                        *counts.entry(block.name.to_string()).or_default() += 1;
                    }
                }
            }
        }
        let mut counts: Vec<(String, u64)> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }

    /// The cells of `slice`'s layer: the whole schematic's extent across it.
    pub fn layer_cells(&self, slice: Slice) -> ([i32; 3], [i32; 3]) {
        let (mut min, mut max) = self.cell_bounds();
        let axis = slice.axis.index();
        (min[axis], max[axis]) = (slice.layer, slice.layer);
        (min, max)
    }

    /// Shows the cells from `min` to `max` (both included) as selected, in
    /// `color` (linear RGBA), or none (plan schematic-studio T25).
    pub fn set_selection(&mut self, cells: Option<([i32; 3], [i32; 3])>, color: [f32; 4]) {
        self.selection = cells.map(|(a, b)| SelectionBox {
            min: [0, 1, 2].map(|axis| a[axis].min(b[axis])),
            max: [0, 1, 2].map(|axis| a[axis].max(b[axis])),
            color,
        });
        self.renderer.set_selection(self.selection);
    }

    /// The schematic as the picture draws it: converted to the pack's game
    /// version, so a pick's position reads the block it names.
    pub fn schematic(&self) -> &UniversalSchematic {
        &self.schematic
    }

    /// Keeps blocks of one id in colour and fades or hides the rest, from
    /// the next frame on. Only a one-byte-per-cell mask is made and
    /// uploaded: the mesh stays as it is (D30). A schematic larger than the
    /// renderer's mask limit a side keeps every block.
    pub fn set_focus(&mut self, focus: &Focus) {
        self.mask = match focus {
            Focus::All => None,
            Focus::Fade(id) => Some((self.mask_of(id), FocusMode::Fade)),
            Focus::Only(id) => Some((self.mask_of(id), FocusMode::Hide)),
        };
        self.focus = focus.clone();
        self.apply_focus();
    }

    /// Outlines the block at `position` in `color` (linear RGBA) with a band
    /// `width` pixels wide, or none, from the next frame on (plan
    /// schematic-studio T32): a selection of one block, apart from the focus
    /// of a kind. The band follows the block's own geometry, so a chest's
    /// edges are outlined and not its cell's. Nothing is meshed again.
    pub fn set_mark(&mut self, position: Option<[i32; 3]>, color: [f32; 4], width: f32) {
        self.mark = position.map(|cell| {
            let (min, max) = self.shape(cell);
            CellMark {
                cell,
                min,
                max,
                color,
                width,
            }
        });
        self.renderer.set_mark(self.mark);
    }

    /// Where the outlined block is in a `width` × `height` frame drawn with
    /// `view`: the screen box of its geometry as fractions of the frame,
    /// `[left, top, right, bottom]`, for a card to float over it (T32).
    /// `None` without an outline or when part of it is behind the eye.
    pub fn mark_rect(&self, view: &View, width: u32, height: u32) -> Option<[f32; 4]> {
        let mark = self.mark?;
        let (camera, bounds) = (camera(view), self.bounds());
        let mut rect = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for corner in 0..8 {
            let point = [0, 1, 2].map(|axis| {
                if corner >> axis & 1 == 0 {
                    mark.min[axis]
                } else {
                    mark.max[axis]
                }
            });
            let [x, y] = project(&camera, bounds, width, height, point)?;
            rect = [
                rect[0].min(x),
                rect[1].min(y),
                rect[2].max(x),
                rect[3].max(y),
            ];
        }
        let (width, height) = (width.max(1) as f32, height.max(1) as f32);
        Some([
            rect[0] / width,
            rect[1] / height,
            rect[2] / width,
            rect[3] / height,
        ])
    }

    /// The box of the triangles that belong to `cell` (each one's centre,
    /// moved a hair against its normal, lies in it: the rule the shader
    /// uses); the cell itself when it has none.
    pub(crate) fn shape(&self, cell: [i32; 3]) -> ([f32; 3], [f32; 3]) {
        let low = cell.map(|v| v as f32 - 0.5);
        let high = cell.map(|v| v as f32 + 0.5);
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        let layers = self
            .meshes
            .iter()
            .flat_map(|mesh| [&mesh.opaque, &mesh.cutout, &mesh.transparent]);
        for layer in layers {
            for triangle in layer.indices.as_chunks::<3>().0 {
                let corners = triangle.map(|index| layer.positions[index as usize]);
                let normal = layer.normals[triangle[0] as usize];
                let inside = (0..3).all(|axis| {
                    let centre = corners.iter().map(|corner| corner[axis]).sum::<f32>() / 3.
                        - normal[axis] * 0.01;
                    centre >= low[axis] && centre < high[axis]
                });
                if inside {
                    for corner in corners {
                        for axis in 0..3 {
                            min[axis] = min[axis].min(corner[axis]);
                            max[axis] = max[axis].max(corner[axis]);
                        }
                    }
                }
            }
        }
        if min[0] > max[0] {
            return (low, high);
        }
        (min, max)
    }

    fn apply_focus(&mut self) {
        match &self.mask {
            Some((mask, mode)) => self.renderer.set_focus(Some(mask), *mode),
            None => self.renderer.set_focus(None, FocusMode::Off),
        }
        self.renderer.set_mark(self.mark);
        self.renderer.set_clip(self.clip());
        self.renderer.set_selection(self.selection);
    }

    /// The cells of the schematic's box that hold block `id`.
    fn mask_of(&self, id: &str) -> FocusMask {
        let bounds = self.schematic.get_bounding_box();
        let origin = [bounds.min.0, bounds.min.1, bounds.min.2];
        let size = [
            bounds.max.0 - bounds.min.0 + 1,
            bounds.max.1 - bounds.min.1 + 1,
            bounds.max.2 - bounds.min.2 + 1,
        ]
        .map(|side| u32::try_from(side).unwrap_or(0));
        if size.iter().any(|&side| side == 0 || side > MAX_FOCUS_SIDE) {
            return FocusMask {
                origin,
                size,
                cells: Vec::new(),
            };
        }
        let mut cells = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
        for z in 0..size[2] as i32 {
            for y in 0..size[1] as i32 {
                for x in 0..size[0] as i32 {
                    let block =
                        self.schematic
                            .get_block(origin[0] + x, origin[1] + y, origin[2] + z);
                    cells.push(u8::from(block.is_some_and(|block| block.name == id)));
                }
            }
        }
        FocusMask {
            origin,
            size,
            cells,
        }
    }

    fn renderer(
        meshes: &[MeshOutput],
        entities: &EntityInstances,
        width: u32,
        height: u32,
    ) -> Result<GpuRenderer, SceneError> {
        pollster::block_on(GpuRenderer::with_entities(
            meshes, entities, width, height, None,
        ))
        .map_err(SceneError::from_render)
    }

    /// Draws one frame. A new size rebuilds the renderer's targets (about 13 ms), so callers should
    /// keep the size steady while the camera moves.
    pub fn render(&mut self, view: &View, width: u32, height: u32) -> Result<Frame, SceneError> {
        let (width, height) = (width.max(1), height.max(1));
        if (self.renderer.width, self.renderer.height) != (width, height) {
            self.renderer = Self::renderer(&self.meshes, &self.entities, width, height)?;
            self.apply_focus();
        }
        self.renderer
            .set_entity_distance(view.position.map(|_| ENTITY_DISTANCE));
        let camera = camera(view);
        let mut pixels = self
            .renderer
            .render_frame(&camera)
            .map_err(SceneError::from_render)?;
        rgba_to_bgra(&mut pixels);
        Ok(Frame {
            width,
            height,
            bgra: pixels,
        })
    }

    /// The bounds the renderer fits the camera to: the blocks' mesh and the entities.
    fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        let (mut min, mut max) = nucleation::rendering::camera::merged_bounds(&self.meshes);
        if let Some((low, high)) = self.entities.bounds() {
            for axis in 0..3 {
                min[axis] = min[axis].min(low[axis]);
                max[axis] = max[axis].max(high[axis]);
            }
        }
        (min, max)
    }

    /// The block under pixel `(x, y)` of a `width` × `height` frame drawn with `view`, or `None`
    /// over air and empty space. Entities are not picked, nor blocks the focus hides.
    pub fn pick(&self, view: &View, width: u32, height: u32, x: f32, y: f32) -> Option<Picked> {
        let ray = camera_ray(&camera(view), self.bounds(), width, height, x, y)?;
        let bounds = self.schematic.get_bounding_box();
        let grid = Grid {
            min: [bounds.min.0, bounds.min.1, bounds.min.2],
            max: [bounds.max.0, bounds.max.1, bounds.max.2],
        };
        let only = match &self.focus {
            Focus::Only(id) => Some(id.as_str()),
            Focus::All | Focus::Fade(_) => None,
        };
        // Layers a slice leaves out are not there to click either.
        let cut = |cell: [i32; 3]| {
            self.slice
                .is_some_and(|slice| cell[slice.axis.index()] > slice.layer)
        };
        let cell = march(ray, grid, |[x, y, z]| {
            !cut([x, y, z])
                && self.schematic.get_block(x, y, z).is_some_and(|block| {
                    !is_air(&block.name) && only.is_none_or(|only| block.name == only)
                })
        })?;
        self.block_at(cell)
    }

    /// The block at `position`, or `None` over air and outside the schematic.
    pub fn block_at(&self, position: [i32; 3]) -> Option<Picked> {
        let [x, y, z] = position;
        let block = self
            .schematic
            .get_block(x, y, z)
            .filter(|block| !is_air(&block.name))?;
        Some(Picked {
            position,
            state: block.to_string(),
        })
    }
}

/// The renderer's camera for `view`; drawing and picking share it.
fn camera(view: &View) -> CameraConfig {
    CameraConfig {
        yaw_deg: view.yaw_deg,
        pitch_deg: view.pitch_deg,
        zoom: view.zoom,
        target: view.target,
        position: view.position,
        fov_deg: if view.position.is_some() { 70. } else { 45. },
        background: view.background,
        // Fitting the bounding sphere keeps the distance constant while orbiting.
        sphere_fit: true,
        ..CameraConfig::default()
    }
}

pub(crate) fn rgba_to_bgra(pixels: &mut [u8]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
}
