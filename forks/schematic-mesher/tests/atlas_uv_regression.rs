//! Regression test: the atlas meshes (opaque/cutout/transparent) must not
//! contain greedy-merged tile-space UVs.
//!
//! Backstory: before the greedy-routing fix, greedy-merged faces with UVs in
//! `[0, w] × [0, h]` (where w/h are the merged tile counts) were leaking into
//! the main atlas mesh. When the atlas sampler encountered UVs > 1 it repeated
//! the entire atlas texture across the face, producing a visible "tiny atlas
//! strip" artifact on blocks.
//!
//! This test meshes a small scene that exercises both paths — a 3×3×3 cube of
//! stone (greedy-eligible → merges to one material) plus a non-greedy block
//! (oak stairs) — and asserts every atlas-mesh vertex has UVs inside the unit
//! square, with a small epsilon for float slop. Greedy materials are allowed
//! to carry tile UVs since each has its own per-material texture.
//!
//! If greedy faces ever start leaking back into the atlas mesh, this test
//! fails with an explicit offending-UV report before the artifact reaches
//! anyone's viewer.

use std::collections::HashMap;

use schematic_mesher::{
    load_resource_pack, Mesher, MesherConfig, TintProvider,
    types::{BlockPosition, BlockSource, BoundingBox, InputBlock},
};

struct Scene {
    blocks: HashMap<BlockPosition, InputBlock>,
    bounds: BoundingBox,
}

impl Scene {
    fn new() -> Self {
        Self {
            blocks: HashMap::new(),
            bounds: BoundingBox::new([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
        }
    }
    fn grow(&mut self, x: i32, y: i32, z: i32) {
        self.bounds.min[0] = self.bounds.min[0].min(x as f32);
        self.bounds.min[1] = self.bounds.min[1].min(y as f32);
        self.bounds.min[2] = self.bounds.min[2].min(z as f32);
        self.bounds.max[0] = self.bounds.max[0].max(x as f32 + 1.0);
        self.bounds.max[1] = self.bounds.max[1].max(y as f32 + 1.0);
        self.bounds.max[2] = self.bounds.max[2].max(z as f32 + 1.0);
    }
    fn set(&mut self, x: i32, y: i32, z: i32, id: &str) {
        self.blocks.insert(BlockPosition::new(x, y, z), InputBlock::new(id));
        self.grow(x, y, z);
    }
    fn set_with(&mut self, x: i32, y: i32, z: i32, id: &str, props: &[(&str, &str)]) {
        let mut b = InputBlock::new(id);
        for &(k, v) in props {
            b.properties.insert(k.to_string(), v.to_string());
        }
        self.blocks.insert(BlockPosition::new(x, y, z), b);
        self.grow(x, y, z);
    }
}

impl BlockSource for Scene {
    fn get_block(&self, p: BlockPosition) -> Option<&InputBlock> { self.blocks.get(&p) }
    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.blocks.iter().map(|(p, b)| (*p, b)))
    }
    fn bounds(&self) -> BoundingBox { self.bounds }
}

/// Returns the first atlas-mesh vertex whose UVs exceed `[-EPS, 1 + EPS]` on
/// either axis, or `None` if every atlas UV is in range.
fn find_stretched_atlas_uv(output: &schematic_mesher::MesherOutput) -> Option<(&'static str, [f32; 2])> {
    const EPS: f32 = 1e-3;
    let check = |name: &'static str, verts: &[schematic_mesher::Vertex]| -> Option<(&'static str, [f32; 2])> {
        for v in verts {
            if v.uv[0] < -EPS || v.uv[0] > 1.0 + EPS
                || v.uv[1] < -EPS || v.uv[1] > 1.0 + EPS
            {
                return Some((name, v.uv));
            }
        }
        None
    };

    if let Some(hit) = check("opaque", &output.opaque_mesh.vertices) { return Some(hit); }
    if let Some(hit) = check("cutout", &output.cutout_mesh.vertices) { return Some(hit); }
    if let Some(hit) = check("transparent", &output.transparent_mesh.vertices) { return Some(hit); }
    None
}

#[test]
fn atlas_mesh_uvs_stay_normalized_with_greedy_meshing() {
    // pack.zip is the vanilla resource pack that ships with the repo.
    let pack = load_resource_pack("pack.zip")
        .expect("pack.zip must exist at repo root for this test");

    let mut scene = Scene::new();
    // 3×3×3 of stone — greedy-eligible, should fully merge into greedy materials.
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                scene.set(x, y, z, "minecraft:stone");
            }
        }
    }
    // A non-greedy block (stairs) on top — exercises the atlas path.
    scene.set_with(1, 3, 1, "minecraft:oak_stairs", &[("facing", "south")]);
    // A waterlogged stair — exercises fluid + atlas interaction.
    scene.set_with(2, 3, 1, "minecraft:oak_stairs", &[
        ("facing", "south"), ("waterlogged", "true"),
    ]);

    // Greedy ON — the bug only manifests with merging enabled.
    let config = MesherConfig {
        cull_hidden_faces: true,
        cull_occluded_blocks: true,
        greedy_meshing: true,
        atlas_max_size: 4096,
        atlas_padding: 1,
        include_air: false,
        tint_provider: TintProvider::new(),
        ambient_occlusion: true,
        ao_intensity: 0.4,
        enable_block_light: false,
        enable_sky_light: false,
        sky_light_level: 15,
        enable_particles: false,
        pre_built_atlas: None,
    };

    let mesher = Mesher::with_config(pack, config);
    let output = mesher.mesh(&scene).expect("meshing failed");

    // Sanity: we should actually have geometry in both the atlas meshes and the
    // greedy materials, so both code paths are exercised.
    let atlas_verts = output.opaque_mesh.vertices.len()
        + output.cutout_mesh.vertices.len()
        + output.transparent_mesh.vertices.len();
    assert!(atlas_verts > 0, "expected non-greedy atlas geometry from stairs");
    assert!(!output.greedy_materials.is_empty(), "expected greedy merging on the stone cube");

    if let Some((which, uv)) = find_stretched_atlas_uv(&output) {
        panic!(
            "atlas `{which}` mesh has a vertex with UV outside [0,1]: ({}, {}). \
             This is the 'tile-UV leak' regression — greedy-merged faces are \
             ending up in the atlas mesh with their per-tile UVs instead of \
             being routed to greedy_materials. See greedy_face_textures in \
             element.rs.",
            uv[0], uv[1],
        );
    }
}
