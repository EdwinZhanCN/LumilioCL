//! Small in-memory packs and block sources for LumilioCL's regression tests.
#![allow(dead_code)]

use schematic_mesher::resource_pack::TextureData;
use schematic_mesher::{BlockPosition, BlockSource, BoundingBox, InputBlock, ResourcePack};

/// Registers `block` with a single variant pointing at `model_json`, stored as
/// `minecraft:block/<block>`.
pub fn add_block(pack: &mut ResourcePack, block: &str, model_json: &str) {
    let blockstate = serde_json::from_str(&format!(
        r#"{{"variants":{{"":{{"model":"minecraft:block/{block}"}}}}}}"#
    ))
    .unwrap();
    pack.add_blockstate("minecraft", block, blockstate);
    pack.add_model(
        "minecraft",
        &format!("block/{block}"),
        serde_json::from_str(model_json).unwrap(),
    );
}

/// A 16×16 texture, fully opaque or with every third pixel see-through.
pub fn add_texture(pack: &mut ResourcePack, path: &str, has_holes: bool) {
    let mut pixels = vec![255u8; 16 * 16 * 4];
    if has_holes {
        for px in pixels.chunks_mut(4).step_by(3) {
            px[3] = 0;
        }
    }
    pack.add_texture("minecraft", path, TextureData::new(16, 16, pixels));
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
pub fn cube_pack(blocks: &[(&str, bool)]) -> ResourcePack {
    let mut pack = ResourcePack::new();
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
