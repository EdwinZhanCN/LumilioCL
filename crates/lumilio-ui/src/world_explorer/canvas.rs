//! A GPUI-native painter for the map: the same tiles, sprites, camera and grid
//! the wgpu scene takes, handed to GPUI as images instead of being composed on
//! our own GPU device and read back. GPUI draws them at device resolution, so
//! there is no readback, no extra copy and no frame that can arrive late.
//!
//! The map's default backend; `LUMILIO_MAP_BACKEND=wgpu` selects the older
//! composed-and-read-back path. What it gives up: GPUI samples images
//! bilinearly (our scene used nearest), and opacity is baked into a sprite's
//! alpha because `paint_image` has no opacity parameter.
use super::stats::{Stats, add};
use gpui::{Bounds, Corners, Pixels, RenderImage, Window, fill, point, px};
use lumilio_map_render::{Camera, Grid, Sprite, Tile, scale_to};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Instant;

/// Texels of edge-replicated border around each tile. GPUI filters bilinearly
/// and its atlas packs tiles side by side, so without a border the edge pixels
/// of a tile blend in whatever neighbours it in the atlas.
const BORDER: u32 = 1;
const TILE: u32 = 256;
const TILES_KEPT: usize = 700;
const SPRITES_KEPT: usize = 256;
/// Grid lines drawn per axis, at most.
const LINES: i64 = 600;

/// What to paint in one frame, in the same terms as the wgpu scene: tile and
/// camera positions in blocks, sprites and the viewport in logical pixels.
pub(super) struct Frame {
    pub camera: Camera,
    pub tiles: Vec<Tile>,
    pub sprites: Vec<Sprite>,
    pub grid: Grid,
    /// Translucent rectangles between the tiles and the icons: area layers.
    pub fills: Vec<Fill>,
}

/// A filled rectangle in logical pixels from the viewport's top left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Fill {
    pub rect: [f64; 4],
    pub rgba: [u8; 4],
}

/// Where a tile's edges fall, in logical pixels from the viewport's top left:
/// `[left, top, right, bottom]`. Each edge is computed from its own block
/// coordinate, so the right edge of one tile and the left edge of the next are
/// the same number bit for bit and can never leave a gap or overlap.
pub(super) fn tile_edges(camera: &Camera, width: f64, height: f64, tile: &Tile) -> [f64; 4] {
    let at = |block: f64, around: f64, extent: f64| {
        (block - around) / camera.blocks_per_pixel + extent / 2.
    };
    [
        at(tile.x, camera.x, width),
        at(tile.z, camera.z, height),
        at(tile.x + tile.span, camera.x, width),
        at(tile.z + tile.span, camera.z, height),
    ]
}

fn render_image(width: u32, height: u32, bgra: Vec<u8>) -> Arc<RenderImage> {
    let buffer = image::RgbaImage::from_raw(width, height, bgra).expect("sized to match");
    Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}

/// A 256-pixel tile as a GPUI image: BGRA, with a replicated border.
pub(super) fn tile_image(rgba: &[u8]) -> Arc<RenderImage> {
    let edge = TILE + 2 * BORDER;
    let mut bgra = Vec::with_capacity((edge * edge * 4) as usize);
    for y in 0..edge {
        let source_y = y.saturating_sub(BORDER).min(TILE - 1);
        for x in 0..edge {
            let source_x = x.saturating_sub(BORDER).min(TILE - 1);
            let at = ((source_y * TILE + source_x) * 4) as usize;
            bgra.extend([rgba[at + 2], rgba[at + 1], rgba[at], rgba[at + 3]]);
        }
    }
    render_image(edge, edge, bgra)
}

/// A sprite scaled to `target` device pixels, with `opacity` baked into alpha.
pub(super) fn sprite_image(sprite: &Sprite, target: u32, opacity: f32) -> Arc<RenderImage> {
    let mut pixels = scale_to(&sprite.rgba, sprite.width, sprite.height, target);
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
        pixel[3] = (f32::from(pixel[3]) * opacity).round() as u8;
    }
    render_image(target, target, pixels)
}

#[derive(Default)]
pub(super) struct Painter {
    tiles: HashMap<String, (Arc<RenderImage>, u64)>,
    sprites: HashMap<(String, u32, u8), (Arc<RenderImage>, u64)>,
    clock: u64,
}

fn device(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}

impl Painter {
    pub fn paint(
        &mut self,
        window: &mut Window,
        bounds: Bounds<Pixels>,
        frame: &Frame,
        stats: &Stats,
    ) {
        let started = Instant::now();
        self.clock += 1;
        let (origin_x, origin_y) = (f64::from(bounds.origin.x), f64::from(bounds.origin.y));
        let (width, height) = (f64::from(bounds.size.width), f64::from(bounds.size.height));
        let dpr = f64::from(window.scale_factor());
        let (mut tiles, mut made) = (0, 0);
        for tile in &frame.tiles {
            let [left, top, right, bottom] = tile_edges(&frame.camera, width, height, tile);
            if right < 0. || bottom < 0. || left > width || top > height {
                continue;
            }
            let entry = self.tiles.entry(tile.id.clone()).or_insert_with(|| {
                made += 1;
                (tile_image(&tile.rgba), 0)
            });
            entry.1 = self.clock;
            // The image reaches one texel past the tile on every side; only the
            // tile's own rectangle is drawn, but filtering may read the border.
            let (texel_x, texel_y) = (
                (right - left) / f64::from(TILE),
                (bottom - top) / f64::from(TILE),
            );
            let at = |x: f64, y: f64| point(px((origin_x + x) as f32), px((origin_y + y) as f32));
            let _ = window.paint_image(
                Bounds::from_corners(at(left, top), at(right, bottom)),
                Bounds::from_corners(
                    at(
                        left - texel_x * f64::from(BORDER),
                        top - texel_y * f64::from(BORDER),
                    ),
                    at(
                        right + texel_x * f64::from(BORDER),
                        bottom + texel_y * f64::from(BORDER),
                    ),
                ),
                Corners::default(),
                entry.0.clone(),
                0,
                false,
            );
            tiles += 1;
        }
        self.paint_grid(window, bounds, frame, (width, height), dpr);
        for fill_rect in &frame.fills {
            let [left, top, right, bottom] = fill_rect.rect.map(|edge| device(edge, dpr));
            if right < 0. || bottom < 0. || left > width || top > height {
                continue;
            }
            let [r, g, b, a] = fill_rect.rgba;
            window.paint_quad(fill(
                Bounds::from_corners(
                    point(px((origin_x + left) as f32), px((origin_y + top) as f32)),
                    point(
                        px((origin_x + right) as f32),
                        px((origin_y + bottom) as f32),
                    ),
                ),
                gpui::rgba(
                    u32::from(r) << 24 | u32::from(g) << 16 | u32::from(b) << 8 | u32::from(a),
                ),
            ));
        }
        let mut painted = 0;
        for sprite in &frame.sprites {
            let edge = f64::from(sprite.size);
            let target = (edge * dpr).round().max(1.) as u32;
            let (left, top) = (
                device(sprite.x - edge / 2., dpr),
                device(sprite.y - edge / 2., dpr),
            );
            if left + edge < 0. || top + edge < 0. || left > width || top > height {
                continue;
            }
            let opacity = (sprite.opacity.clamp(0., 1.) * 100.).round() as u8;
            let entry = self
                .sprites
                .entry((sprite.id.clone(), target, opacity))
                .or_insert_with(|| {
                    made += 1;
                    (sprite_image(sprite, target, f32::from(opacity) / 100.), 0)
                });
            entry.1 = self.clock;
            let rect = Bounds::from_corners(
                point(px((origin_x + left) as f32), px((origin_y + top) as f32)),
                point(
                    px((origin_x + left + edge) as f32),
                    px((origin_y + top + edge) as f32),
                ),
            );
            let _ = window.paint_image(rect, rect, Corners::default(), entry.0.clone(), 0, false);
            painted += 1;
        }
        self.evict(window);
        stats.canvas_paints.fetch_add(1, Relaxed);
        stats.canvas_tiles.fetch_add(tiles, Relaxed);
        stats.canvas_sprites.fetch_add(painted, Relaxed);
        stats.canvas_images.fetch_add(made, Relaxed);
        add(&stats.canvas_us, started.elapsed());
    }

    /// Chunk and Region lines as thin translucent quads over the tiles.
    fn paint_grid(
        &self,
        window: &mut Window,
        bounds: Bounds<Pixels>,
        frame: &Frame,
        (width, height): (f64, f64),
        dpr: f64,
    ) {
        let camera = &frame.camera;
        let (min_x, min_z) = (
            camera.x - width / 2. * camera.blocks_per_pixel,
            camera.z - height / 2. * camera.blocks_per_pixel,
        );
        let (max_x, max_z) = (
            camera.x + width / 2. * camera.blocks_per_pixel,
            camera.z + height / 2. * camera.blocks_per_pixel,
        );
        let line = 1. / dpr.max(1.);
        let mut lines = |step: f64, shade: f32| {
            let first = (min_x / step).ceil() as i64;
            let last = (max_x / step).floor() as i64;
            if last - first <= LINES {
                for k in first..=last {
                    let x = device(
                        (k as f64 * step - camera.x) / camera.blocks_per_pixel + width / 2.,
                        dpr,
                    );
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.origin.x + px(x as f32), bounds.origin.y),
                            gpui::size(px(line as f32), bounds.size.height),
                        ),
                        gpui::hsla(0., 0., 0., shade),
                    ));
                }
            }
            let first = (min_z / step).ceil() as i64;
            let last = (max_z / step).floor() as i64;
            if last - first <= LINES {
                for k in first..=last {
                    let y = device(
                        (k as f64 * step - camera.z) / camera.blocks_per_pixel + height / 2.,
                        dpr,
                    );
                    window.paint_quad(fill(
                        Bounds::new(
                            point(bounds.origin.x, bounds.origin.y + px(y as f32)),
                            gpui::size(bounds.size.width, px(line as f32)),
                        ),
                        gpui::hsla(0., 0., 0., shade),
                    ));
                }
            }
        };
        if frame.grid.chunks && camera.blocks_per_pixel <= 4. {
            lines(16., 0.35);
        }
        if frame.grid.regions {
            lines(512., 0.65);
        }
    }

    /// Drops the images nothing drew lately, from GPUI's atlas too.
    fn evict(&mut self, window: &mut Window) {
        fn prune<K: Clone + Eq + std::hash::Hash>(
            map: &mut HashMap<K, (Arc<RenderImage>, u64)>,
            keep: usize,
            window: &mut Window,
        ) {
            if map.len() <= keep {
                return;
            }
            let mut order: Vec<(u64, K)> = map
                .iter()
                .map(|(key, (_, used))| (*used, key.clone()))
                .collect();
            order.sort_by_key(|(used, _)| *used);
            for (_, key) in order.into_iter().take(map.len() - keep) {
                if let Some((image, _)) = map.remove(&key) {
                    let _ = window.drop_image(image);
                }
            }
        }
        prune(&mut self.tiles, TILES_KEPT, window);
        prune(&mut self.sprites, SPRITES_KEPT, window);
    }

    /// Returns every image to GPUI when the view goes away.
    pub fn release(&mut self, window: &mut Window) {
        for (image, _) in self.tiles.drain().map(|(_, value)| value) {
            let _ = window.drop_image(image);
        }
        for (image, _) in self.sprites.drain().map(|(_, value)| value) {
            let _ = window.drop_image(image);
        }
    }
}
