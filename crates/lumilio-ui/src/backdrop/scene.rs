use std::time::Duration;

use gpui::prelude::*;
use gpui::{Hsla, IntoElement, Pixels, SharedString, Size, div, px};
use gpui_component::v_flex;

use super::glyphs::{
    cave_glyph, firefly_glyph, lake_water_glyph, moon_glyph, ramp_glyph, star_glyph, torch_glyph,
    tree_glyph, watcher_glyph,
};

pub(super) const CELL_WIDTH: f32 = 8.5;
pub(super) const CELL_HEIGHT: f32 = 15.;
pub(super) const MIN_COLUMNS: usize = 48;
pub(super) const MAX_COLUMNS: usize = 160;
pub(super) const MIN_ROWS: usize = 18;
pub(super) const MAX_ROWS: usize = 48;
pub(super) const TAU: f32 = std::f32::consts::TAU;
pub(super) const GLYPH_RAMP: &[u8] = b" .,:;*+#%@";

/// Colors needed by the backdrop without coupling it to the global theme.
///
/// Every color arrives with normal alpha. The scene controls contrast with
/// hue, lightness, and glyph density rather than a translucent safety mask.
#[derive(Clone)]
pub struct BackdropColors {
    pub background: Hsla,
    pub sky: Hsla,
    pub haze: Hsla,
    pub terrain_far: Hsla,
    pub terrain_mid: Hsla,
    pub terrain_near: Hsla,
    pub grass: Hsla,
    pub dirt: Hsla,
    pub stone: Hsla,
    pub foliage: Hsla,
    pub water: Hsla,
    pub coal_ore: Hsla,
    pub iron_ore: Hsla,
    pub lapis_ore: Hsla,
    pub diamond_ore: Hsla,
    pub warm: Hsla,
    pub moon: Hsla,
    pub mono_font_family: SharedString,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SceneTint {
    Sky,
    Haze,
    TerrainFar,
    TerrainMid,
    TerrainNear,
    Grass,
    Dirt,
    Stone,
    Snow,
    Foliage,
    Water,
    CoalOre,
    IronOre,
    LapisOre,
    DiamondOre,
    Lava,
    Timber,
    Mob,
    Warm,
    Moon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TerrainLayer {
    Far,
    Mid,
    Near,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BiomeZone {
    Forest,
    Meadow,
    Lake,
    Plains,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SurfaceKind {
    Land,
    LakeBed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TerrainSample {
    pub(super) layer: TerrainLayer,
    pub(super) depth: usize,
    pub(super) noise: f32,
    pub(super) column: usize,
    pub(super) row: usize,
    pub(super) columns: usize,
    pub(super) rows: usize,
    pub(super) surface_kind: SurfaceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SceneHighlight {
    pub(super) index: usize,
    pub(super) tint: SceneTint,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SceneLine {
    pub(super) text: String,
    pub(super) highlights: Vec<SceneHighlight>,
    pub(super) tint: SceneTint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SceneFrame {
    pub(super) columns: usize,
    pub(super) rows: usize,
    pub(super) elapsed_seconds: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SurfaceRows {
    pub(super) far: usize,
    pub(super) mid: usize,
    pub(super) near: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LakeRows {
    pub(super) waterline: usize,
    pub(super) bed: usize,
}

impl SceneFrame {
    pub(super) fn for_viewport(viewport: Size<Pixels>, elapsed: Duration) -> Self {
        let columns = (f32::from(viewport.width) / CELL_WIDTH) as usize;
        let rows = (f32::from(viewport.height) / CELL_HEIGHT) as usize;
        Self {
            columns: columns.clamp(MIN_COLUMNS, MAX_COLUMNS),
            rows: rows.clamp(MIN_ROWS, MAX_ROWS),
            elapsed_seconds: elapsed.as_secs_f32(),
        }
    }
}

/// Render the full-window Home backdrop. It is intentionally just one layer;
/// Home content and navigation are painted later by the shell and remain
/// independent foreground elements.
pub fn render(
    viewport: Size<Pixels>,
    elapsed: Duration,
    colors: BackdropColors,
) -> impl IntoElement {
    let frame = SceneFrame::for_viewport(viewport, elapsed);
    let lines = generate_scene(frame);
    let background = colors.background;
    let font_family = colors.mono_font_family.clone();
    let scene_width = px(frame.columns as f32 * CELL_WIDTH);
    let scene_height = px(frame.rows as f32 * CELL_HEIGHT);

    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .bg(background)
        .child(
            v_flex().size_full().items_center().justify_center().child(
                v_flex()
                    .w(scene_width)
                    .h(scene_height)
                    .overflow_hidden()
                    .font_family(font_family)
                    .text_size(crate::theme::font_px(13.))
                    .line_height(px(CELL_HEIGHT))
                    .whitespace_nowrap()
                    .children(lines.into_iter().map(move |line| {
                        let base_color = scene_color(line.tint, &colors);
                        let highlights = line.highlights.into_iter().map(|highlight| {
                            (
                                highlight.index..highlight.index + 1,
                                gpui::HighlightStyle {
                                    color: Some(scene_color(highlight.tint, &colors)),
                                    ..gpui::HighlightStyle::default()
                                },
                            )
                        });
                        div()
                            .text_color(base_color)
                            .child(gpui::StyledText::new(line.text).with_highlights(highlights))
                    })),
            ),
        )
}

pub(super) fn scene_color(tint: SceneTint, colors: &BackdropColors) -> Hsla {
    match tint {
        SceneTint::Sky => colors.sky,
        SceneTint::Haze => colors.haze,
        SceneTint::TerrainFar => colors.terrain_far,
        SceneTint::TerrainMid => colors.terrain_mid,
        SceneTint::TerrainNear => colors.terrain_near,
        SceneTint::Grass => colors.grass,
        SceneTint::Dirt => colors.dirt,
        SceneTint::Stone => colors.stone,
        SceneTint::Snow => colors.moon,
        SceneTint::Foliage => colors.foliage,
        SceneTint::Water => colors.water,
        SceneTint::CoalOre => colors.coal_ore,
        SceneTint::IronOre => colors.iron_ore,
        SceneTint::LapisOre => colors.lapis_ore,
        SceneTint::DiamondOre => colors.diamond_ore,
        SceneTint::Lava => colors.warm,
        SceneTint::Timber => colors.dirt,
        SceneTint::Mob => colors.foliage,
        SceneTint::Warm => colors.warm,
        SceneTint::Moon => colors.moon,
    }
}

pub(super) fn generate_scene(frame: SceneFrame) -> Vec<SceneLine> {
    let mut lines = Vec::with_capacity(frame.rows);
    let last_row = frame.rows.saturating_sub(1).max(1) as f32;
    let last_column = frame.columns.saturating_sub(1).max(1) as f32;
    let surfaces: Vec<_> = (0..frame.columns)
        .map(|column| surface_rows_at(column as f32 / last_column, frame.columns, frame.rows))
        .collect();
    let watcher_surface = surface_rows_at(0.88, frame.columns, frame.rows).mid;

    for row in 0..frame.rows {
        let normalized_y = row as f32 / last_row;
        let base_tint = line_tint(normalized_y);
        let mut text = String::with_capacity(frame.columns);
        let mut highlights = Vec::new();

        for column in 0..frame.columns {
            let normalized_x = column as f32 / last_column;
            let noise = hash01(column as u32, row as u32, 17);
            let surface_rows = surfaces[column];

            let mut glyph = ' ';
            let mut tint = base_tint;

            if let Some(moon) = moon_glyph(column, row, frame.columns, frame.rows) {
                glyph = moon;
                tint = SceneTint::Moon;
            } else if let Some(star) = star_glyph(
                normalized_x,
                normalized_y,
                column,
                row,
                frame.elapsed_seconds,
            ) {
                glyph = star;
                tint = SceneTint::Moon;
            } else if let Some((watcher, watcher_tint)) =
                watcher_glyph(column, row, watcher_surface, frame.columns, frame.rows)
            {
                glyph = watcher;
                tint = watcher_tint;
            } else if let Some((tree, tree_tint)) =
                tree_glyph(column, row, &surfaces, frame.columns, frame.rows)
            {
                glyph = tree;
                tint = tree_tint;
            } else if let Some((torch, torch_tint)) = torch_glyph(
                column,
                row,
                surface_rows.near,
                frame.columns,
                frame.elapsed_seconds,
            ) {
                glyph = torch;
                tint = torch_tint;
            } else if let Some((water, water_tint)) =
                lake_water_glyph(normalized_x, row, column, frame.rows, frame.elapsed_seconds)
            {
                glyph = water;
                tint = water_tint;
            } else {
                let (layer, surface_row) = if row >= surface_rows.near {
                    (TerrainLayer::Near, surface_rows.near)
                } else if row >= surface_rows.mid {
                    (TerrainLayer::Mid, surface_rows.mid)
                } else if row >= surface_rows.far {
                    (TerrainLayer::Far, surface_rows.far)
                } else {
                    (TerrainLayer::Far, usize::MAX)
                };

                if surface_row != usize::MAX {
                    let depth = row.saturating_sub(surface_row);
                    let surface_kind = if layer == TerrainLayer::Near
                        && biome_zone(normalized_x) == BiomeZone::Lake
                    {
                        SurfaceKind::LakeBed
                    } else {
                        SurfaceKind::Land
                    };
                    if layer == TerrainLayer::Near {
                        let cave = cave_glyph(
                            normalized_x,
                            normalized_y,
                            depth,
                            column,
                            row,
                            frame.elapsed_seconds,
                        );
                        if let Some((cave_glyph, cave_tint)) = cave {
                            glyph = cave_glyph;
                            tint = cave_tint;
                        } else {
                            let terrain = terrain_glyph(TerrainSample {
                                layer,
                                depth,
                                noise,
                                column,
                                row,
                                columns: frame.columns,
                                rows: frame.rows,
                                surface_kind,
                            });
                            glyph = terrain.0;
                            tint = terrain.1;
                        }
                    } else {
                        let terrain = terrain_glyph(TerrainSample {
                            layer,
                            depth,
                            noise,
                            column,
                            row,
                            columns: frame.columns,
                            rows: frame.rows,
                            surface_kind,
                        });
                        glyph = terrain.0;
                        tint = terrain.1;
                    }
                } else if let Some(firefly) = firefly_glyph(
                    normalized_x,
                    normalized_y,
                    column,
                    row,
                    frame.elapsed_seconds,
                ) {
                    glyph = firefly;
                    tint = SceneTint::Warm;
                }
            }

            if tint != base_tint && glyph != ' ' {
                highlights.push(SceneHighlight {
                    index: text.len(),
                    tint,
                });
            }
            text.push(glyph);
        }

        lines.push(SceneLine {
            text,
            highlights,
            tint: base_tint,
        });
    }

    lines
}

pub(super) fn line_tint(y: f32) -> SceneTint {
    if y < 0.32 {
        SceneTint::Sky
    } else if y < 0.48 {
        SceneTint::Haze
    } else if y < 0.61 {
        SceneTint::TerrainFar
    } else if y < 0.74 {
        SceneTint::TerrainMid
    } else {
        SceneTint::TerrainNear
    }
}

pub(super) fn surface_rows_at(x: f32, columns: usize, rows: usize) -> SurfaceRows {
    let stepped_x =
        ((x * (columns / 4).max(12) as f32).floor() / (columns / 4).max(12) as f32).clamp(0., 1.);
    let far_peak_left = triangular_peak(stepped_x, 0.18, 0.22, 0.20);
    let far_peak_right = triangular_peak(stepped_x, 0.78, 0.28, 0.16);
    let far = 0.52 - far_peak_left - far_peak_right + (stepped_x * 15. + 0.4).sin().abs() * 0.018;
    let mid = 0.64
        - triangular_peak(stepped_x, 0.08, 0.18, 0.055)
        - triangular_peak(stepped_x, 0.76, 0.30, 0.045)
        + (stepped_x * 9. + 1.7).sin() * 0.018;
    let near = match biome_zone(x) {
        BiomeZone::Forest => quantized_row(
            0.73 + (stepped_x * 9. + 2.6).sin() * 0.025 + (stepped_x * 21. - 0.8).sin() * 0.010,
            rows,
        ),
        BiomeZone::Meadow => quantized_row(0.76 + (stepped_x * 8. + 1.4).sin() * 0.012, rows),
        BiomeZone::Lake => lake_rows_at(x, rows)
            .map(|lake| lake.bed)
            .unwrap_or_else(|| quantized_row(0.80, rows)),
        BiomeZone::Plains => quantized_row(0.75 + (stepped_x * 5. + 0.6).sin() * 0.008, rows),
    };

    SurfaceRows {
        far: quantized_row(far, rows),
        mid: quantized_row(mid, rows),
        near,
    }
}

pub(super) fn biome_zone(x: f32) -> BiomeZone {
    if x < 0.34 {
        BiomeZone::Forest
    } else if x < 0.42 {
        BiomeZone::Meadow
    } else if x < 0.64 {
        BiomeZone::Lake
    } else {
        BiomeZone::Plains
    }
}

pub(super) fn lake_rows_at(x: f32, rows: usize) -> Option<LakeRows> {
    if biome_zone(x) != BiomeZone::Lake {
        return None;
    }
    let waterline = (rows as f32 * 0.75).round() as usize;
    let center_depth = (rows / 9).clamp(2, 5);
    let bowl = (1. - (x - 0.53).abs() / 0.11).clamp(0., 1.);
    let depth = 1 + (center_depth as f32 * bowl).round() as usize;
    Some(LakeRows {
        waterline,
        bed: (waterline + depth).min(rows.saturating_sub(1)),
    })
}

pub(super) fn triangular_peak(x: f32, center: f32, half_width: f32, height: f32) -> f32 {
    (1. - (x - center).abs() / half_width).clamp(0., 1.) * height
}

pub(super) fn quantized_row(y: f32, rows: usize) -> usize {
    let row = (y.clamp(0.22, 0.84) * rows as f32).round() as usize;
    row.saturating_sub(row % 2)
}

pub(super) fn terrain_glyph(sample: TerrainSample) -> (char, SceneTint) {
    let TerrainSample {
        layer,
        depth,
        noise,
        column,
        row,
        columns,
        rows,
        surface_kind,
    } = sample;
    let chunk_width = (columns / 10).max(6);

    match layer {
        TerrainLayer::Far => match depth {
            0 if row < rows * 9 / 20 && column.is_multiple_of(chunk_width) => {
                ('+', SceneTint::Snow)
            }
            0 if row < rows * 9 / 20 => (if noise > 0.52 { '^' } else { '_' }, SceneTint::Snow),
            0 if column.is_multiple_of(chunk_width) => ('+', SceneTint::TerrainFar),
            0 if noise > 0.56 => ('^', SceneTint::TerrainFar),
            0 => ('_', SceneTint::TerrainFar),
            1 if row < rows * 9 / 20 && noise > 0.30 => ('.', SceneTint::Snow),
            1 if noise > 0.46 => (':', SceneTint::Stone),
            2 if row.is_multiple_of(4) && noise > 0.40 => ('-', SceneTint::Stone),
            _ if depth < 6 && noise > 0.66 => ('.', SceneTint::Stone),
            _ => (' ', SceneTint::TerrainFar),
        },
        TerrainLayer::Mid => match depth {
            0 if column.is_multiple_of(chunk_width) => ('+', SceneTint::Grass),
            0 if noise > 0.70 => ('#', SceneTint::Grass),
            0 if noise > 0.34 => ('^', SceneTint::Grass),
            0 => ('_', SceneTint::Grass),
            1 if column.is_multiple_of(chunk_width) => ('|', SceneTint::Dirt),
            1 if noise > 0.92 => ('o', SceneTint::Warm),
            1 if noise > 0.22 => (':', SceneTint::Dirt),
            1 => ('.', SceneTint::Dirt),
            2 if row.is_multiple_of(4) && noise > 0.36 => ('-', SceneTint::Stone),
            2 if noise > 0.76 => ('.', SceneTint::Stone),
            _ if depth < 6 && noise > 0.50 => (ramp_glyph(0.24 + noise * 0.30), SceneTint::Stone),
            _ => (' ', SceneTint::TerrainMid),
        },
        TerrainLayer::Near if surface_kind == SurfaceKind::LakeBed => match depth {
            0 if column.is_multiple_of(chunk_width) => ('+', SceneTint::Dirt),
            0 if noise > 0.52 => ('=', SceneTint::Dirt),
            0 => ('.', SceneTint::Dirt),
            1 if noise > 0.62 => (':', SceneTint::Dirt),
            1 => ('.', SceneTint::Stone),
            2 if row.is_multiple_of(4) && noise > 0.28 => ('-', SceneTint::Stone),
            _ if noise > 0.42 => (ramp_glyph(0.18 + noise * 0.30), SceneTint::Stone),
            _ => (' ', SceneTint::Stone),
        },
        TerrainLayer::Near => match depth {
            0 if column.is_multiple_of(chunk_width) => ('+', SceneTint::Grass),
            0 if noise > 0.72 => ('#', SceneTint::Grass),
            0 if noise > 0.42 => ('^', SceneTint::Grass),
            0 => ('_', SceneTint::Grass),
            1 if column.is_multiple_of(chunk_width) => ('|', SceneTint::Dirt),
            1 if noise > 0.93 => ('o', SceneTint::Warm),
            1 => (if noise > 0.64 { ':' } else { '.' }, SceneTint::Dirt),
            2 if row.is_multiple_of(4) && noise > 0.28 => ('-', SceneTint::Stone),
            2 if noise > 0.78 => (':', SceneTint::Stone),
            _ if depth < 7 && noise > 0.34 => (ramp_glyph(0.18 + noise * 0.32), SceneTint::Stone),
            _ if noise > 0.58 => (ramp_glyph(0.14 + noise * 0.24), SceneTint::Stone),
            _ => (' ', SceneTint::Stone),
        },
    }
}

pub(super) fn hash01(x: u32, y: u32, seed: u32) -> f32 {
    let mut value = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_147_483_647));
    value = (value ^ (value >> 13)).wrapping_mul(1_274_126_177);
    ((value ^ (value >> 16)) as f32) / u32::MAX as f32
}
