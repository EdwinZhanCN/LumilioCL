use crate::mesher::geometry::Vertex;
use crate::resolver::ModelResolver;
use crate::resource_pack::ResourcePack;
use super::EntityFaceTexture;

/// Grid layout constants (fallback transparent grid).
const COLS: u32 = 9;
const CELL_SIZE: u32 = 16;

/// Minecraft chest GUI layout constants (from generic_54.png, 256×256 texture).
const GUI_WIDTH: u32 = 176;
const GUI_TITLE_HEIGHT: u32 = 17; // space above first slot row
const GUI_SLOT_PITCH: u32 = 18; // pixels between slot rows/columns
const GUI_SLOT_INSET: u32 = 1; // border within each slot cell
const GUI_SLOT_GRID_X: u32 = 7; // x offset to first slot column
const GUI_SLOT_GRID_Y: u32 = 17; // y offset to first slot row
const GUI_BOTTOM_PAD: u32 = 7; // border below last slot row

/// Render an inventory hologram floating above a container.
///
/// Returns (vertices, indices, face_textures, texture_data) for a horizontal quad
/// at y=1.5 above the block, displaying a grid of item icons.
///
/// `inventory_str`: CSV of item IDs. Empty = empty slot. Optional `:count` suffix.
/// Example: "diamond_sword,apple,,stone:64"
pub(crate) fn render_inventory_hologram(
    resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    inventory_str: &str,
) -> Option<(Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>, crate::resource_pack::TextureData)> {
    let slots: Vec<&str> = inventory_str.split(',').collect();
    if slots.is_empty() {
        return None;
    }

    // Try GUI-based rendering first, fall back to transparent grid
    let texture_data = if let Some(td) = render_gui_inventory(resource_pack, model_resolver, &slots) {
        td
    } else {
        render_fallback_inventory(resource_pack, model_resolver, &slots)
    };

    let (vertices, indices, face_textures) = generate_hologram_quad(texture_data.width, texture_data.height);
    Some((vertices, indices, face_textures, texture_data))
}

/// Render inventory using the actual Minecraft chest GUI texture as background.
fn render_gui_inventory(
    resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    slots: &[&str],
) -> Option<crate::resource_pack::TextureData> {
    let gui_tex = resource_pack.get_texture("gui/container/generic_54")?;
    let gui_frame = gui_tex.first_frame();

    // Single chest (≤27 slots) = 3 rows, double chest = 6 rows
    let rows = if slots.len() <= 27 { 3u32 } else { 6u32 };
    let crop_w = GUI_WIDTH;

    // For 6 rows: straight crop from y=0 to y=132
    // For 3 rows: composite top section (title + 3 rows) + bottom border strip
    //   The bottom border lives at y=(17+6*18) to y=(17+6*18+7) in the texture.
    //   We copy it after our 3 rows to close the GUI frame.
    let top_h = GUI_TITLE_HEIGHT + rows * GUI_SLOT_PITCH; // content above border
    let crop_h = top_h + GUI_BOTTOM_PAD;
    let border_src_y = GUI_TITLE_HEIGHT + 6 * GUI_SLOT_PITCH; // always from 6-row position

    let mut pixels = vec![0u8; (crop_w * crop_h * 4) as usize];

    // Copy top section (title + slot rows)
    copy_region(&gui_frame.pixels, gui_frame.width, &mut pixels, crop_w, 0, 0, crop_w, top_h);

    // Copy bottom border strip from the full 6-row frame bottom
    copy_region(&gui_frame.pixels, gui_frame.width, &mut pixels, crop_w, border_src_y, top_h, crop_w, GUI_BOTTOM_PAD);

    // Blit item icons at correct slot positions
    for (i, slot) in slots.iter().enumerate() {
        let slot = slot.trim();
        if slot.is_empty() {
            continue;
        }

        let item_name = slot.split(':').next().unwrap_or(slot);
        let item_id = if item_name.contains(':') {
            item_name.to_string()
        } else {
            format!("minecraft:{}", item_name)
        };

        if let Some(tex_path) = resolve_item_icon(resource_pack, model_resolver, &item_id) {
            if let Some(tex) = resource_pack.get_texture(&tex_path) {
                let frame = tex.first_frame();
                let col = (i as u32) % COLS;
                let row = (i as u32) / COLS;
                let dst_x = GUI_SLOT_GRID_X + col * GUI_SLOT_PITCH + GUI_SLOT_INSET;
                let dst_y = GUI_SLOT_GRID_Y + row * GUI_SLOT_PITCH + GUI_SLOT_INSET;
                blit_texture(
                    &mut pixels,
                    crop_w,
                    crop_h,
                    dst_x,
                    dst_y,
                    &frame.pixels,
                    frame.width,
                    frame.height,
                );
            }
        }
    }

    Some(crate::resource_pack::TextureData {
        width: crop_w,
        height: crop_h,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Fallback: render item icons on a transparent grid (original behavior).
fn render_fallback_inventory(
    resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    slots: &[&str],
) -> crate::resource_pack::TextureData {
    let rows = ((slots.len() as u32 + COLS - 1) / COLS).max(1);
    let grid_w = COLS * CELL_SIZE;
    let grid_h = rows * CELL_SIZE;

    let mut pixels = vec![0u8; (grid_w * grid_h * 4) as usize];

    for (i, slot) in slots.iter().enumerate() {
        let slot = slot.trim();
        if slot.is_empty() {
            continue;
        }

        let item_name = slot.split(':').next().unwrap_or(slot);
        let item_id = if item_name.contains(':') {
            item_name.to_string()
        } else {
            format!("minecraft:{}", item_name)
        };

        if let Some(tex_path) = resolve_item_icon(resource_pack, model_resolver, &item_id) {
            if let Some(tex) = resource_pack.get_texture(&tex_path) {
                let frame = tex.first_frame();
                let col = (i as u32) % COLS;
                let row = (i as u32) / COLS;
                blit_texture(
                    &mut pixels,
                    grid_w,
                    grid_h,
                    col * CELL_SIZE,
                    row * CELL_SIZE,
                    &frame.pixels,
                    frame.width,
                    frame.height,
                );
            }
        }
    }

    crate::resource_pack::TextureData {
        width: grid_w,
        height: grid_h,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    }
}

/// Resolve an item to its primary icon texture path.
fn resolve_item_icon(
    _resource_pack: &ResourcePack,
    model_resolver: &ModelResolver,
    item_id: &str,
) -> Option<String> {
    let name = item_id.strip_prefix("minecraft:").unwrap_or(item_id);

    // Try item model first
    let item_path = format!("minecraft:item/{}", name);
    if let Ok(model) = model_resolver.resolve(&item_path) {
        let textures = model_resolver.resolve_textures(&model);
        // Generated items: layer0
        if let Some(tex) = textures.get("layer0") {
            return Some(tex.clone());
        }
        // Block items: check for common texture vars
        for key in &["all", "top", "front", "side", "particle"] {
            if let Some(tex) = textures.get(*key) {
                return Some(tex.clone());
            }
        }
    }

    // Fallback: try block model
    let block_path = format!("minecraft:block/{}", name);
    if let Ok(model) = model_resolver.resolve(&block_path) {
        let textures = model_resolver.resolve_textures(&model);
        for key in &["all", "top", "front", "side", "particle"] {
            if let Some(tex) = textures.get(*key) {
                return Some(tex.clone());
            }
        }
    }

    None
}

/// Copy a rectangular region from src pixels (full-width row stride) into dst.
/// `src_y` is the source row to start from, `dst_y` is where to place it in dst.
fn copy_region(
    src: &[u8],
    src_w: u32,
    dst: &mut [u8],
    dst_w: u32,
    src_y: u32,
    dst_y: u32,
    width: u32,
    height: u32,
) {
    for row in 0..height {
        for x in 0..width {
            let si = (((src_y + row) * src_w + x) * 4) as usize;
            let di = (((dst_y + row) * dst_w + x) * 4) as usize;
            if si + 3 < src.len() && di + 3 < dst.len() {
                dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
            }
        }
    }
}

/// Blit a source texture into the grid at (dst_x, dst_y).
/// Scales if source dimensions don't match CELL_SIZE.
fn blit_texture(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    dst_x: u32,
    dst_y: u32,
    src: &[u8],
    src_w: u32,
    src_h: u32,
) {
    for y in 0..CELL_SIZE {
        for x in 0..CELL_SIZE {
            let dx = dst_x + x;
            let dy = dst_y + y;
            if dx >= dst_w || dy >= dst_h {
                continue;
            }

            // Sample from source (nearest neighbor scaling)
            let sx = (x * src_w / CELL_SIZE).min(src_w - 1);
            let sy = (y * src_h / CELL_SIZE).min(src_h - 1);
            let si = ((sy * src_w + sx) * 4) as usize;
            let di = ((dy * dst_w + dx) * 4) as usize;

            if si + 3 < src.len() && di + 3 < dst.len() {
                // Alpha-over compositing (in case of layered items)
                let sa = src[si + 3] as f32 / 255.0;
                if sa > 0.01 {
                    let da = dst[di + 3] as f32 / 255.0;
                    let out_a = sa + da * (1.0 - sa);
                    if out_a > 0.0 {
                        dst[di] = ((src[si] as f32 * sa + dst[di] as f32 * da * (1.0 - sa)) / out_a) as u8;
                        dst[di + 1] = ((src[si + 1] as f32 * sa + dst[di + 1] as f32 * da * (1.0 - sa)) / out_a) as u8;
                        dst[di + 2] = ((src[si + 2] as f32 * sa + dst[di + 2] as f32 * da * (1.0 - sa)) / out_a) as u8;
                        dst[di + 3] = (out_a * 255.0) as u8;
                    }
                }
            }
        }
    }
}

/// Generate a double-sided vertical quad for the inventory hologram.
///
/// Stands upright above the block, centered at x=0.5, z=0.5.
/// Bottom edge at y=1.05 (just above the block top).
/// Aspect ratio adjusted based on grid dimensions.
fn generate_hologram_quad(
    grid_w: u32,
    grid_h: u32,
) -> (Vec<Vertex>, Vec<u32>, Vec<EntityFaceTexture>) {
    let aspect = grid_w as f32 / grid_h as f32;

    // Width along X, height along Y
    let half_w = 0.4;
    let height = half_w * 2.0 / aspect;

    let x0 = 0.5 - half_w;
    let x1 = 0.5 + half_w;
    let y0: f32 = 1.05; // just above block top
    let y1 = y0 + height;
    let z: f32 = 0.5;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Front face (facing +Z)
    let v_base = vertices.len() as u32;
    vertices.push(Vertex::new([x0, y1, z], [0.0, 0.0, 1.0], [0.0, 0.0]));
    vertices.push(Vertex::new([x1, y1, z], [0.0, 0.0, 1.0], [1.0, 0.0]));
    vertices.push(Vertex::new([x1, y0, z], [0.0, 0.0, 1.0], [1.0, 1.0]));
    vertices.push(Vertex::new([x0, y0, z], [0.0, 0.0, 1.0], [0.0, 1.0]));
    indices.extend_from_slice(&[v_base, v_base + 1, v_base + 2, v_base, v_base + 2, v_base + 3]);

    // Back face (facing -Z, reversed winding)
    let v_base2 = vertices.len() as u32;
    vertices.push(Vertex::new([x0, y1, z], [0.0, 0.0, -1.0], [0.0, 0.0]));
    vertices.push(Vertex::new([x1, y1, z], [0.0, 0.0, -1.0], [1.0, 0.0]));
    vertices.push(Vertex::new([x1, y0, z], [0.0, 0.0, -1.0], [1.0, 1.0]));
    vertices.push(Vertex::new([x0, y0, z], [0.0, 0.0, -1.0], [0.0, 1.0]));
    indices.extend_from_slice(&[v_base2, v_base2 + 2, v_base2 + 1, v_base2, v_base2 + 3, v_base2 + 2]);

    let face_textures = vec![
        EntityFaceTexture { texture: String::new(), is_transparent: true },
        EntityFaceTexture { texture: String::new(), is_transparent: true },
    ];

    (vertices, indices, face_textures)
}
