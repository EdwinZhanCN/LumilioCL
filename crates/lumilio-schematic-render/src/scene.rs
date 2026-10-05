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
}

impl Scene {
    /// Parses a `.litematic` and meshes it with the pack's textures. Blocks: call it off the UI thread.
    pub fn load(schematic: &[u8], pack: &[u8]) -> Result<Self, SceneError> {
        let schematic = nucleation::formats::litematic::from_litematic(schematic)
            .map_err(|e| SceneError::Parse(e.to_string()))?;
        Self::from_schematic(&schematic, pack)
    }

    pub(crate) fn from_schematic(
        schematic: &UniversalSchematic,
        pack: &[u8],
    ) -> Result<Self, SceneError> {
        let pack =
            ResourcePackSource::from_bytes(pack).map_err(|e| SceneError::Pack(e.to_string()))?;
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
        Ok(Self { meshes, renderer })
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

pub(crate) fn rgba_to_bgra(pixels: &mut [u8]) {
    for px in pixels.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
}
