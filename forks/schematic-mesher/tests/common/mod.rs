//! Small packs and block sources for LumilioCL's regression tests.
//!
//! A fixture is written as resource-pack files and baked by
//! `lumilio-block-assets`, exactly as the launcher bakes a client jar, so the
//! guards check what the launcher draws (plan schematic-studio, T4).
#![allow(dead_code)]

use std::io::{Cursor, Write};

use lumilio_block_assets::{bake, AssetSource, EntityDump, EntityLayers};
use schematic_mesher::{BlockPosition, BlockSource, BoundingBox, InputBlock, ResourcePack};

/// Resource-pack files, by their path in the archive.
#[derive(Clone, Default)]
pub struct Fixture(Vec<(String, Vec<u8>)>);

impl Fixture {
    pub fn file(&mut self, path: &str, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.0.retain(|(name, _)| name != path);
        self.0.push((path.to_owned(), bytes.into()));
        self
    }

    /// `minecraft:<path>` as a model JSON.
    pub fn model(&mut self, path: &str, json: &str) -> &mut Self {
        self.file(&format!("assets/minecraft/models/{path}.json"), json)
    }

    /// `minecraft:<block>`'s blockstate JSON.
    pub fn blockstate(&mut self, block: &str, json: &str) -> &mut Self {
        self.file(&format!("assets/minecraft/blockstates/{block}.json"), json)
    }

    /// The files as a zip, baked, as the mesher sees them.
    pub fn bake(&self) -> ResourcePack {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in &self.0 {
            writer
                .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        let archive = writer.finish().unwrap().into_inner();
        let baked = bake(&[AssetSource::ArchiveBytes(archive)]).unwrap();
        assert!(
            baked.meta.skipped.is_empty(),
            "fixture files the baker skipped: {:?}",
            baked.meta.skipped
        );
        ResourcePack::from_baked(&baked)
    }
}

/// Registers `block` with a single variant pointing at `model_json`, stored as
/// `minecraft:block/<block>`.
pub fn add_block(pack: &mut Fixture, block: &str, model_json: &str) {
    pack.blockstate(
        block,
        &format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{block}"}}}}}}"#),
    );
    pack.model(&format!("block/{block}"), model_json);
}

/// A 16×16 texture, fully opaque or with every third pixel see-through.
pub fn add_texture(pack: &mut Fixture, path: &str, has_holes: bool) {
    let mut pixels = vec![255u8; 16 * 16 * 4];
    if has_holes {
        for px in pixels.chunks_mut(4).step_by(3) {
            px[3] = 0;
        }
    }
    let mut png = Vec::new();
    image::RgbaImage::from_raw(16, 16, pixels)
        .unwrap()
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    pack.file(&format!("assets/minecraft/textures/{path}.png"), png);
}

/// Entity model layers the tests draw, taken from the real `entities.dat.z`
/// (26.3; beds and signs from 26.1.2, their last entity version; one 1.20.1
/// armor layer) so the guards run without `target/` (plan schematic-studio T7).
/// Regenerate by picking the same layer ids from a fresh bake.
pub fn entity_layers() -> EntityLayers {
    EntityDump::from_json(include_bytes!("../fixtures/entity_layers.json"))
        .unwrap()
        .into_layers()
}

/// An empty pack holding only [`entity_layers`].
pub fn entity_pack() -> ResourcePack {
    ResourcePack::new().with_entity_layers(entity_layers())
}

/// Model JSON for one full cube using `#all` on every face.
pub fn cube_model(texture: &str) -> String {
    let face = r##"{"texture":"#all"}"##;
    format!(
        r#"{{"textures":{{"all":"{texture}"}},"elements":[{{"from":[0,0,0],"to":[16,16,16],"faces":{{"down":{face},"up":{face},"north":{face},"south":{face},"west":{face},"east":{face}}}}}]}}"#
    )
}

/// A pack where each named block is a single full cube with its own texture,
/// either solid or with see-through pixels.
pub fn cube_pack(blocks: &[(&str, bool)]) -> Fixture {
    let mut pack = Fixture::default();
    for &(name, has_holes) in blocks {
        add_block(
            &mut pack,
            name,
            &cube_model(&format!("minecraft:block/{name}")),
        );
        add_texture(&mut pack, &format!("block/{name}"), has_holes);
    }
    pack
}

/// A block list that meshes directly.
pub struct Blocks(pub Vec<(BlockPosition, InputBlock)>);

impl BlockSource for Blocks {
    fn get_block(&self, pos: BlockPosition) -> Option<&InputBlock> {
        self.0.iter().find(|(p, _)| *p == pos).map(|(_, b)| b)
    }

    fn iter_blocks(&self) -> Box<dyn Iterator<Item = (BlockPosition, &InputBlock)> + '_> {
        Box::new(self.0.iter().map(|(p, b)| (*p, b)))
    }

    fn bounds(&self) -> BoundingBox {
        BoundingBox::from_points(self.0.iter().flat_map(|(p, _)| {
            let min = [p.x as f32, p.y as f32, p.z as f32];
            [min, min.map(|v| v + 1.0)]
        }))
        .unwrap_or(BoundingBox::new([0.0; 3], [0.0; 3]))
    }
}
