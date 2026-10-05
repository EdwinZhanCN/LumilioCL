use crate::resource_pack::{ResourcePack, TextureData};

/// Composite a villager texture by alpha-stacking the three layers MC's
/// `VillagerRenderer` + `VillagerProfessionLayer` would draw in separate passes:
///
///   1. `entity/villager/villager.png`             (base skin with head)
///   2. `entity/villager/type/{biome}.png`         (biome clothing overlay)
///   3. `entity/villager/profession/{prof}.png`    (job clothing overlay, if any)
///
/// Skipped layers: an empty/missing profession (e.g. "none" / unemployed) skips
/// step 3. If the biome texture is missing, the base skin is returned as-is.
pub(crate) fn composite_villager_texture(
    pack: &ResourcePack,
    biome: &str,
    profession: &str,
) -> Option<TextureData> {
    let base_tex = pack.get_texture("entity/villager/villager")?;
    let base_frame = base_tex.first_frame();
    let width = base_frame.width;
    let height = base_frame.height;
    let mut pixels = base_frame.pixels.clone();

    let biome_path = format!("entity/villager/type/{}", biome);
    if let Some(biome_tex) = pack.get_texture(&biome_path) {
        alpha_over(&mut pixels, width, height, &biome_tex.first_frame());
    }

    if !profession.is_empty() && profession != "none" {
        let prof_path = format!("entity/villager/profession/{}", profession);
        if let Some(prof_tex) = pack.get_texture(&prof_path) {
            alpha_over(&mut pixels, width, height, &prof_tex.first_frame());
        }
    }

    Some(TextureData {
        width,
        height,
        pixels,
        is_animated: false,
        frame_count: 1,
        animation: None,
    })
}

/// Standard straight-alpha "A over B" compositing for full RGBA overlays.
fn alpha_over(dst: &mut [u8], dst_w: u32, dst_h: u32, src: &TextureData) {
    let w = src.width.min(dst_w);
    let h = src.height.min(dst_h);

    for y in 0..h {
        for x in 0..w {
            let s = ((y * src.width + x) * 4) as usize;
            let d = ((y * dst_w + x) * 4) as usize;
            if s + 3 >= src.pixels.len() || d + 3 >= dst.len() {
                continue;
            }

            let sa = src.pixels[s + 3] as f32 / 255.0;
            if sa < 0.01 {
                continue;
            }

            let da = dst[d + 3] as f32 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }

            for c in 0..3 {
                let sv = src.pixels[s + c] as f32;
                let dv = dst[d + c] as f32;
                let out = (sv * sa + dv * da * (1.0 - sa)) / out_a;
                dst[d + c] = out.min(255.0) as u8;
            }
            dst[d + 3] = (out_a * 255.0).min(255.0) as u8;
        }
    }
}
