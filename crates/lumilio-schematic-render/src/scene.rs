use nucleation::UniversalSchematic;
use nucleation::meshing::{MeshConfig, MeshOutput, ResourcePackSource};
use nucleation::rendering::{CameraConfig, GpuRenderer};

use crate::{SceneError, View};

/// One rendered image.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, blue first.
    pub bgra: Vec<u8>,
}

/// A parsed, meshed schematic with a renderer sized for the last frame drawn.
pub struct Scene {
    meshes: Vec<MeshOutput>,
    renderer: GpuRenderer,
    undrawable: Vec<String>,
}

impl Scene {
    /// Parses a `.litematic` and meshes it with the pack's textures. Blocks: call it off the UI thread.
    ///
    /// When the pack is a game JAR, block IDs are first converted to that game version, so a
    /// schematic saved before a rename (`chain` → `iron_chain`) still finds its models.
    pub fn load(schematic: &[u8], pack: &[u8]) -> Result<Self, SceneError> {
        let mut schematic = nucleation::formats::litematic::from_litematic(schematic)
            .map_err(|e| SceneError::Parse(e.to_string()))?;
        if let Some(version) = game_data_version(pack) {
            schematic.convert_to_data_version(version);
        }
        Self::from_schematic(&schematic, pack)
    }

    /// Block IDs in the schematic that this pack cannot draw at all. They are left out of the
    /// picture, so the host should say so.
    pub fn undrawable_blocks(&self) -> &[String] {
        &self.undrawable
    }

    /// A useful initial flight position, just outside the build at standing eye height.
    pub fn explore_position(&self) -> [f32; 3] {
        let (min, max) = nucleation::rendering::camera::merged_bounds(&self.meshes);
        if (0..3)
            .any(|axis| !min[axis].is_finite() || !max[axis].is_finite() || min[axis] > max[axis])
        {
            return [0., 1.62, 3.];
        }
        [(min[0] + max[0]) * 0.5, min[1] + 1.62, max[2] + 3.0]
    }

    pub(crate) fn from_schematic(
        schematic: &UniversalSchematic,
        pack: &[u8],
    ) -> Result<Self, SceneError> {
        let pack =
            ResourcePackSource::from_bytes(pack).map_err(|e| SceneError::Pack(e.to_string()))?;
        let undrawable = schematic.undrawable_blocks(&pack);
        // Nucleation src/rendering/gpu.rs (MIT; see ATTRIBUTIONS.md) consumes
        // only opaque/cutout/transparent layers at this revision, not the
        // mesher's separate greedy_materials. Enabling greedy meshing makes
        // ordinary solid cubes disappear. Keep native rendering complete.
        let config = MeshConfig::default();
        let mesh = schematic
            .to_mesh(&pack, &config)
            .map_err(|e| SceneError::Mesh(e.to_string()))?;
        let meshes = vec![mesh];
        // Creating it here finds out about a missing GPU before the first frame is asked for.
        let renderer = Self::renderer(&meshes, 640, 480)?;
        Ok(Self {
            meshes,
            renderer,
            undrawable,
        })
    }

    fn renderer(meshes: &[MeshOutput], width: u32, height: u32) -> Result<GpuRenderer, SceneError> {
        pollster::block_on(GpuRenderer::new(meshes, width, height, None))
            .map_err(SceneError::from_render)
    }

    /// Draws one frame. A new size rebuilds the renderer's targets (about 13 ms), so callers should
    /// keep the size steady while the camera moves.
    pub fn render(&mut self, view: &View, width: u32, height: u32) -> Result<Frame, SceneError> {
        let (width, height) = (width.max(1), height.max(1));
        if (self.renderer.width, self.renderer.height) != (width, height) {
            self.renderer = Self::renderer(&self.meshes, width, height)?;
        }
        let camera = CameraConfig {
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
        };
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
}

/// The data version (`world_version`) a client JAR's `version.json` declares; `None` for a plain
/// resource pack.
pub(crate) fn game_data_version(pack: &[u8]) -> Option<i32> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(pack)).ok()?;
    let file = archive.by_name("version.json").ok()?;
    let json: serde_json::Value = serde_json::from_reader(file).ok()?;
    i32::try_from(json.get("world_version")?.as_i64()?).ok()
}

pub(crate) fn rgba_to_bgra(pixels: &mut [u8]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
}
