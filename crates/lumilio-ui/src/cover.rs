//! Deterministic pixel covers for instances (design language §7).
//!
//! A cover is a small window into the world: layered hills under a sky,
//! seeded by the instance id, tinted by loader, and dressed by the world the
//! instance was last in. Covers are static; only a hovered card may ever
//! animate one. The raster is pure and GPUI-free, so it is unit-tested.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{App, Bounds, Corners, Hsla, Pixels, RenderImage, Window, canvas};

use crate::hero::dissolve::PageDissolve;
use crate::hero::noise::{bayer4, fbm2, hash2, smoothstep};
use crate::hero::raster::{PixelGrid, Rgb};
use crate::home::WorldHint;

/// The game loader a cover is painted for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl Loader {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vanilla => crate::tr!("loader-vanilla"),
            Self::Fabric => "Fabric",
            Self::Forge => "Forge",
            Self::NeoForge => "NeoForge",
            Self::Quilt => "Quilt",
        }
    }
}

/// Target texel size of a cover, in logical pixels.
const TEXEL: f32 = 5.;

struct Palette {
    sky_top: Rgb,
    sky_low: Rgb,
    far: Rgb,
    mid: Rgb,
    near: Rgb,
    glow: Rgb,
    speck: Rgb,
}

fn palette(loader: Loader, world: WorldHint) -> Palette {
    let (sky_top, sky_low, far, mid, near) = match loader {
        Loader::Vanilla => (0x3b6ea8, 0x9fd0e8, 0x5d8f7a, 0x3f7a4e, 0x2b5a38),
        Loader::Fabric => (0x2a4d6b, 0xa9c9c0, 0x5a8a86, 0x3c6f6a, 0x274c4b),
        Loader::Forge => (0x5a3a45, 0xf0b27a, 0x8a5a4a, 0x6a4038, 0x422a2a),
        Loader::NeoForge => (0x4a2f52, 0xf59a6a, 0x82506a, 0x633a5a, 0x3e2540),
        Loader::Quilt => (0x33366e, 0xb6a4e6, 0x6a63a8, 0x4d4886, 0x322f5e),
    };
    let (glow, speck) = match world {
        WorldHint::Overworld => (0xffe6a0, 0xf2d16b),
        WorldHint::Underground => (0x9fe8ff, 0x7fe0ff),
        WorldHint::Redstone => (0xff4a3a, 0xff5a4a),
        WorldHint::Nether => (0xff8a3a, 0xffb04a),
    };
    Palette {
        sky_top: Rgb::hex(sky_top),
        sky_low: Rgb::hex(sky_low),
        far: Rgb::hex(far),
        mid: Rgb::hex(mid),
        near: Rgb::hex(near),
        glow: Rgb::hex(glow),
        speck: Rgb::hex(speck),
    }
}

/// Which edges of a cover dissolve into the surface it sits on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Edges {
    /// The last rows, the way a card or a page cover meets what is below it.
    Bottom,
    /// The leading columns as well as the last rows: a cover anchored to the
    /// trailing corner that dies away behind a header's text.
    Corner,
}

/// Rasterises one cover, its edges dissolving into the page by `dissolve`.
pub fn render(
    seed: u32,
    loader: Loader,
    world: WorldHint,
    columns: usize,
    rows: usize,
    dissolve: PageDissolve,
) -> PixelGrid {
    let colors = palette(loader, world);
    let mut grid = PixelGrid::new(columns, rows);
    let (width, height) = (columns as f32, rows as f32);
    let seed = seed.wrapping_mul(0x9e37_79b1);
    let ridge = |x: f32, base: f32, amp: f32, salt: u32| {
        let n = fbm2(x / (width * 0.32) + salt as f32 * 3.7, 0.5, seed ^ salt);
        (base + (n - 0.5) * amp) * height
    };

    grid.shade(|x, y| {
        let (fx, fy) = (x as f32, y as f32);
        // Sky, banded by ordered dither the way low-colour art does.
        let t = smoothstep(0., 0.85, fy / height);
        let banded = ((t * 6. + bayer4(x as i32, y as i32) - 0.5).floor() / 6.).clamp(0., 1.);
        let mut color = colors.sky_top.lerp(colors.sky_low, banded);
        if fy < height * 0.5 && hash2(x as i32, y as i32, seed) > 0.985 {
            color = color.lerp(Rgb::new(1., 1., 1.), 0.6 * (1. - t));
        }
        if fy > ridge(fx, 0.5, 0.34, 1) {
            color = colors.far;
        }
        if fy > ridge(fx, 0.66, 0.3, 2) {
            color = colors.mid;
        }
        if fy > ridge(fx, 0.82, 0.22, 3) {
            color = colors.near;
            if hash2(x as i32, y as i32, seed ^ 0x51) > 0.975 {
                color = colors.speck;
            }
        }
        color
    });

    // One landmark light, placed by seed: the sun, a lamp, a portal glow.
    let cx = (width * (0.2 + 0.6 * hash2(1, 1, seed))) as i32;
    let cy = (height * (0.18 + 0.12 * hash2(2, 2, seed))) as i32;
    let radius = (height * 0.1).max(2.) as i32;
    let halo = radius * 2;
    for dy in -halo..=halo {
        for dx in -halo..=halo {
            let distance = ((dx * dx + dy * dy) as f32).sqrt();
            if distance <= radius as f32 {
                grid.put(cx + dx, cy + dy, colors.glow);
            } else if distance <= halo as f32 && bayer4(cx + dx, cy + dy) < 0.35 {
                let (x, y) = (cx + dx, cy + dy);
                if x >= 0 && y >= 0 && (x as usize) < columns && (y as usize) < rows {
                    let base = grid.get(x as usize, y as usize);
                    grid.put(x, y, base.lerp(colors.glow, 0.35));
                }
            }
        }
    }

    dissolve.apply(&mut grid);
    grid
}

/// A cover element filling its parent. `page` is the surface it dissolves
/// into, so the art meets the card without a hard edge; on a translucent
/// surface the dissolved texels are left to what is behind it.
pub fn element(
    seed: u32,
    loader: Loader,
    world: WorldHint,
    page: Hsla,
    fade: Pixels,
) -> impl IntoElement {
    painted(seed, loader, world, page, fade, Edges::Bottom)
}

/// A header's backdrop: the cover fills its parent, dissolving on the left
/// and bottom so only the trailing corner keeps its colour. The parent is
/// anchored to the corner and sized by the caller.
pub fn corner(
    seed: u32,
    loader: Loader,
    world: WorldHint,
    page: Hsla,
    fade: Pixels,
) -> impl IntoElement {
    painted(seed, loader, world, page, fade, Edges::Corner)
}

fn painted(
    seed: u32,
    loader: Loader,
    world: WorldHint,
    page: Hsla,
    fade: Pixels,
    edges: Edges,
) -> impl IntoElement {
    let dissolve = PageDissolve::of(page);
    canvas(
        |_, _, _| (),
        move |bounds: Bounds<Pixels>, _, window, _| {
            let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            if width < 1. || height < 1. {
                return;
            }
            let scale = window.scale_factor();
            let (px_w, px_h) = (
                (width * scale).round() as u32,
                (height * scale).round() as u32,
            );
            let key = (
                seed,
                loader as u8,
                world as u8,
                px_w,
                px_h,
                (f32::from(fade) * scale).round() as u32,
                edges as u8,
                dissolve.page.pack(),
                dissolve.translucent,
            );
            let image = cached(key, window, || {
                let columns = ((width / TEXEL).round() as usize).clamp(8, 200);
                let texel = width / columns as f32;
                let rows = ((height / texel).round() as usize).max(4);
                let fade_rows = (f32::from(fade) / (height / rows as f32)).round() as usize;
                let fade_rows = fade_rows.min(rows / 2);
                let fade_cols = match edges {
                    Edges::Bottom => 0,
                    // Most of the width, so the left edge is the page and the
                    // text over it never meets busy art.
                    Edges::Corner => columns * 9 / 10,
                };
                let dissolve = dissolve.bottom(fade_rows).left(fade_cols);
                let grid = render(seed, loader, world, columns, rows, dissolve);
                let bytes = rasterize(&grid, dissolve, px_w as usize, px_h as usize);
                let buffer = image::RgbaImage::from_raw(px_w, px_h, bytes)?;
                Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
            });
            if let Some(image) = image {
                window
                    .paint_image(bounds, bounds, Corners::default(), image, 0, false)
                    .ok();
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A cover rasterised at device resolution: BGRA, as GPUI's sprite atlas
/// expects, with each texel mapped to a whole block of device pixels so the
/// art stays crisp instead of being smoothed by the sampler. The texels
/// `dissolve` leaves to the glass are transparent.
fn rasterize(grid: &PixelGrid, dissolve: PageDissolve, width: usize, height: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        let row = y * grid.height() / height;
        for x in 0..width {
            let column = x * grid.width() / width;
            let color = grid.get(column, row).pack();
            if dissolve.clears(column, row, grid.height(), color) {
                bytes.extend_from_slice(&[0; 4]);
                continue;
            }
            bytes.extend_from_slice(&[
                (color & 0xff) as u8,
                ((color >> 8) & 0xff) as u8,
                ((color >> 16) & 0xff) as u8,
                0xff,
            ]);
        }
    }
    bytes
}

type CoverKey = (u32, u8, u8, u32, u32, u32, u8, u32, bool);

thread_local! {
    /// Covers already uploaded, so a repaint reuses the same atlas entry
    /// instead of building and uploading a new bitmap every frame.
    static CACHE: RefCell<HashMap<CoverKey, Arc<RenderImage>>> = RefCell::new(HashMap::new());
}

/// The most covers kept alive; the whole cache is dropped past this.
const CACHE_LIMIT: usize = 96;

/// Lets go of every cover built so far, a game having started. A cover on
/// screen is built again when next painted, which costs little.
pub fn release(cx: &mut App) {
    let covers: Vec<_> =
        CACHE.with(|cache| cache.borrow_mut().drain().map(|(_, image)| image).collect());
    if covers.is_empty() {
        return;
    }
    // Outside any window update, so every window's atlas is reached.
    cx.defer(move |cx| {
        for cover in covers {
            cx.drop_image(cover, None);
        }
        cx.refresh_windows();
    });
}

fn cached(
    key: CoverKey,
    window: &mut Window,
    build: impl FnOnce() -> Option<Arc<RenderImage>>,
) -> Option<Arc<RenderImage>> {
    if let Some(hit) = CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return Some(hit);
    }
    let image = build()?;
    let evicted: Vec<_> = CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let evicted = if cache.len() >= CACHE_LIMIT {
            cache.drain().map(|(_, image)| image).collect()
        } else {
            Vec::new()
        };
        cache.insert(key, image.clone());
        evicted
    });
    for old in evicted {
        window.drop_image(old).ok();
    }
    Some(image)
}

#[cfg(test)]
mod tests;
