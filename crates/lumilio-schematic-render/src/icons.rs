//! Block icons as an inventory shows them (plan schematic-studio, T27/D29),
//! drawn from the same baked pack as the picture.
//!
//! A block whose item is flat (a torch, redstone dust, a door) is its item's
//! texture, scaled without smoothing and tinted like the game tints grass
//! and vines. Every other block is its model, placed alone and drawn with a
//! parallel camera from above the south-east corner. All the models of a
//! batch share one renderer, which draws them one at a time; making a
//! renderer costs far more than drawing with it.

use lumilio_block_assets::{BakedPack, EntityLayers, icon_item, icon_tint};
use nucleation::UniversalSchematic;
use nucleation::meshing::{MeshConfig, MeshOutput, ResourcePackSource};
use nucleation::rendering::{CameraConfig, GpuRenderer, Projection};

use crate::scene::rgba_to_bgra;
use crate::{Frame, SceneError};

/// The box every model icon is framed to: one block.
const BLOCK: ([f32; 3], [f32; 3]) = ([0., 0., 0.], [1., 1., 1.]);

/// One icon per state (`minecraft:oak_stairs[facing=east]`), `size` pixels
/// square with a clear background; `None` where nothing can be drawn (air).
/// A block the pack does not know gets the missing-texture block the
/// picture draws for it. Blocks: call it off the UI thread.
pub fn icons(
    pack: &BakedPack,
    entities: EntityLayers,
    states: &[String],
    size: u32,
) -> Result<Vec<Option<Frame>>, SceneError> {
    let size = size.clamp(1, 512);
    let icons: Vec<Option<Frame>> = states
        .iter()
        .map(|state| flat(pack, &icon_item(state), icon_tint(state), size))
        .collect();
    let wanted: Vec<usize> = (0..states.len())
        .filter(|&at| icons[at].is_none())
        .collect();
    draw_blocks(pack, entities, states, icons, wanted, size)
}

/// One icon per item (`minecraft:golden_carrot`) as an inventory slot shows
/// it (plan schematic-studio T32): a flat item is its texture, a block item
/// its block; `None` for an item the pack has neither for (a shield, which
/// the game draws in code), never a stand-in.
pub fn item_icons(
    pack: &BakedPack,
    entities: EntityLayers,
    items: &[String],
    size: u32,
) -> Result<Vec<Option<Frame>>, SceneError> {
    let size = size.clamp(1, 512);
    let model = |item: &str| {
        let (namespace, name) = item.split_once(':').unwrap_or(("minecraft", item));
        format!("{namespace}:item/{name}")
    };
    let icons: Vec<Option<Frame>> = items
        .iter()
        .map(|item| flat(pack, &model(item), None, size))
        .collect();
    let wanted: Vec<usize> = (0..items.len())
        .filter(|&at| icons[at].is_none() && pack.assets.blockstates.contains_key(&items[at]))
        .collect();
    draw_blocks(pack, entities, items, icons, wanted, size)
}

/// Draws `states[at]` for each `at` in `wanted` into `icons`.
fn draw_blocks(
    pack: &BakedPack,
    entities: EntityLayers,
    states: &[String],
    mut icons: Vec<Option<Frame>>,
    wanted: Vec<usize>,
    size: u32,
) -> Result<Vec<Option<Frame>>, SceneError> {
    if wanted.is_empty() {
        return Ok(icons);
    }
    let source = ResourcePackSource::from_baked(pack, entities);
    let mut meshes: Vec<MeshOutput> = Vec::new();
    let mut owners = Vec::new();
    for at in wanted {
        if let Some(mesh) = block_mesh(&source, &states[at]) {
            meshes.push(mesh);
            owners.push(at);
        }
    }
    if meshes.is_empty() {
        return Ok(icons);
    }
    let mut renderer = pollster::block_on(GpuRenderer::new(&meshes, size, size, None))
        .map_err(SceneError::from_render)?;
    renderer.set_fit_bounds(BLOCK.0, BLOCK.1);
    let camera = CameraConfig {
        yaw_deg: 45.,
        pitch_deg: 30.,
        projection: Projection::Orthographic,
        background: Some([0., 0., 0., 0.]),
        ..CameraConfig::default()
    };
    for (mesh, at) in owners.into_iter().enumerate() {
        renderer.set_only_mesh(Some(mesh));
        let mut bgra = renderer
            .render_frame(&camera)
            .map_err(SceneError::from_render)?;
        rgba_to_bgra(&mut bgra);
        icons[at] = Some(Frame {
            width: size,
            height: size,
            bgra,
        });
    }
    Ok(icons)
}

/// `state` alone at the origin, meshed; `None` when nothing is drawn (air,
/// or a state the mesher refuses). An icon is a picture beside a name, so a
/// block without one costs nothing more.
fn block_mesh(source: &ResourcePackSource, state: &str) -> Option<MeshOutput> {
    let mut schematic = UniversalSchematic::new("icon".to_owned());
    if !schematic.set_block_str(0, 0, 0, state) {
        return None;
    }
    let mesh = schematic.to_mesh(source, &MeshConfig::default()).ok()?;
    let empty = [&mesh.opaque, &mesh.cutout, &mesh.transparent]
        .iter()
        .all(|layer| layer.indices.is_empty());
    (!empty).then_some(mesh)
}

/// Item model `model` drawn flat from its texture layers, the first tinted
/// with colormap `tint`; `None` when the model has elements (a cube, a
/// slab, a chest) or no textures.
fn flat(pack: &BakedPack, model: &str, tint: Option<&str>, size: u32) -> Option<Frame> {
    let model = pack.assets.models.get(model)?;
    if !model.elements.is_empty() {
        return None;
    }
    // A flat item has at least its first layer.
    pack.assets.texture_image(model.textures.get("layer0")?)?;
    let tint = tint.and_then(|map| pack.assets.tint_samples.get(map));
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    for layer in 0..4 {
        let Some(image) = model
            .textures
            .get(&format!("layer{layer}"))
            .and_then(|texture| pack.assets.texture_image(texture))
        else {
            break;
        };
        // An animated texture is a strip of square frames; draw the first.
        let side = image.width.min(image.height).max(1);
        for y in 0..size {
            for x in 0..size {
                let (u, v) = (x * side / size, y * side / size);
                let from = ((v * image.width + u) * 4) as usize;
                let Some(&[r, g, b, a]) = image.rgba.get(from..from + 4) else {
                    continue;
                };
                if a == 0 {
                    continue;
                }
                let [r, g, b] = match (layer, tint) {
                    (0, Some(&[tr, tg, tb])) => [
                        (u16::from(r) * u16::from(tr) / 255) as u8,
                        (u16::from(g) * u16::from(tg) / 255) as u8,
                        (u16::from(b) * u16::from(tb) / 255) as u8,
                    ],
                    _ => [r, g, b],
                };
                let to = ((y * size + x) * 4) as usize;
                // Layers are opaque or clear per pixel; a later layer covers.
                bgra[to..to + 4].copy_from_slice(&[b, g, r, a]);
            }
        }
    }
    Some(Frame {
        width: size,
        height: size,
        bgra,
    })
}
