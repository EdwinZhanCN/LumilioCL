//! A deterministic, animated ASCII environment for the Home canvas.
//!
//! The scene deliberately stays renderer-independent for now. GPUI 0.2.2 does
//! not expose a stable public glyph-atlas shader API, so the same small scene
//! contract is expressed with normal text elements and can move to a shader
//! later without changing the Home or navigation layers.

use std::time::Duration;

use gpui::{Hsla, IntoElement, Pixels, SharedString, Size, div, prelude::*, px};
use gpui_component::v_flex;

const CELL_WIDTH: f32 = 8.5;
const CELL_HEIGHT: f32 = 15.;
const MIN_COLUMNS: usize = 48;
const MAX_COLUMNS: usize = 160;
const MIN_ROWS: usize = 18;
const MAX_ROWS: usize = 48;
const TAU: f32 = std::f32::consts::TAU;
const GLYPH_RAMP: &[u8] = b" .,:;*+#%@";

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
enum SceneTint {
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
enum TerrainLayer {
    Far,
    Mid,
    Near,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BiomeZone {
    Forest,
    Meadow,
    Lake,
    Plains,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceKind {
    Land,
    LakeBed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerrainSample {
    layer: TerrainLayer,
    depth: usize,
    noise: f32,
    column: usize,
    row: usize,
    columns: usize,
    rows: usize,
    surface_kind: SurfaceKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SceneHighlight {
    index: usize,
    tint: SceneTint,
}

#[derive(Clone, Debug, PartialEq)]
struct SceneLine {
    text: String,
    highlights: Vec<SceneHighlight>,
    tint: SceneTint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SceneFrame {
    columns: usize,
    rows: usize,
    elapsed_seconds: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SurfaceRows {
    far: usize,
    mid: usize,
    near: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LakeRows {
    waterline: usize,
    bed: usize,
}

impl SceneFrame {
    fn for_viewport(viewport: Size<Pixels>, elapsed: Duration) -> Self {
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
                    .text_size(px(13.))
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

fn scene_color(tint: SceneTint, colors: &BackdropColors) -> Hsla {
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

fn generate_scene(frame: SceneFrame) -> Vec<SceneLine> {
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

fn line_tint(y: f32) -> SceneTint {
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

fn surface_rows_at(x: f32, columns: usize, rows: usize) -> SurfaceRows {
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

fn biome_zone(x: f32) -> BiomeZone {
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

fn lake_rows_at(x: f32, rows: usize) -> Option<LakeRows> {
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

fn triangular_peak(x: f32, center: f32, half_width: f32, height: f32) -> f32 {
    (1. - (x - center).abs() / half_width).clamp(0., 1.) * height
}

fn quantized_row(y: f32, rows: usize) -> usize {
    let row = (y.clamp(0.22, 0.84) * rows as f32).round() as usize;
    row.saturating_sub(row % 2)
}

fn terrain_glyph(sample: TerrainSample) -> (char, SceneTint) {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeKind {
    Spruce,
    Oak,
}

fn tree_glyph(
    column: usize,
    row: usize,
    surfaces: &[SurfaceRows],
    columns: usize,
    rows: usize,
) -> Option<(char, SceneTint)> {
    let trees = [
        (0.03, 0.20, TerrainLayer::Near, TreeKind::Spruce),
        (0.10, 0.14, TerrainLayer::Mid, TreeKind::Oak),
        (0.26, 0.16, TerrainLayer::Near, TreeKind::Oak),
        (0.33, 0.13, TerrainLayer::Mid, TreeKind::Spruce),
        (0.72, 0.13, TerrainLayer::Near, TreeKind::Oak),
        (0.96, 0.15, TerrainLayer::Near, TreeKind::Oak),
    ];
    let column = column as isize;

    for (anchor, height_ratio, layer, kind) in trees {
        let anchor_column = (anchor * columns as f32) as isize;
        let anchor_index =
            anchor_column.clamp(0, surfaces.len().saturating_sub(1) as isize) as usize;
        let surface_rows = surfaces[anchor_index];
        let surface_row = match layer {
            TerrainLayer::Far => surface_rows.far,
            TerrainLayer::Mid => surface_rows.mid,
            TerrainLayer::Near => surface_rows.near,
        };
        let height = (rows as f32 * height_ratio).round().max(5.) as isize;
        let level = surface_row as isize - row as isize;
        if !(1..=height).contains(&level) {
            continue;
        }

        let offset = column - anchor_column;
        let glyph = match kind {
            TreeKind::Spruce => spruce_shape(level, height, offset),
            TreeKind::Oak => oak_shape(level, height, offset),
        };
        let Some(glyph) = glyph else {
            continue;
        };
        let tint = if layer == TerrainLayer::Far {
            SceneTint::TerrainFar
        } else if glyph == '|' {
            SceneTint::Timber
        } else {
            SceneTint::Foliage
        };
        return Some((glyph, tint));
    }

    None
}

fn spruce_shape(level: isize, height: isize, offset: isize) -> Option<char> {
    let distance = offset.abs();
    if level <= 2 && distance == 0 {
        return Some('|');
    }
    if level < 2 {
        return None;
    }
    let width = 1 + (height - level) / 2;
    if distance > width {
        return None;
    }
    Some(if distance == width || level == height {
        '#'
    } else {
        '*'
    })
}

fn oak_shape(level: isize, height: isize, offset: isize) -> Option<char> {
    let distance = offset.abs();
    let trunk_height = (height / 2).max(2);
    if level <= trunk_height && distance == 0 {
        return Some('|');
    }
    if level < trunk_height {
        return None;
    }
    let crown_level = level - trunk_height;
    let crown_height = height - trunk_height;
    let width = if crown_level == 0 || crown_level == crown_height {
        3
    } else {
        4
    };
    if distance > width {
        return None;
    }
    Some(if distance == width || level == height {
        '#'
    } else {
        '*'
    })
}

fn watcher_glyph(
    column: usize,
    row: usize,
    terrain_row: usize,
    columns: usize,
    rows: usize,
) -> Option<(char, SceneTint)> {
    let anchor = (columns as f32 * 0.88).round() as isize;
    let height = (rows as f32 * 0.12).round().max(4.) as isize;
    let level = terrain_row as isize - row as isize;
    let offset = column as isize - anchor;
    if !(1..=height).contains(&level) || offset.abs() > 1 {
        return None;
    }

    if level >= height - 1 {
        if level == height - 1 && offset != 0 {
            return Some((':', SceneTint::Warm));
        }
        return Some(('#', SceneTint::Mob));
    }
    if level == 1 {
        return (offset != 0).then_some(('#', SceneTint::Mob));
    }
    if offset == 0 || level == 2 {
        Some(('#', SceneTint::Mob))
    } else {
        None
    }
}

fn cave_glyph(
    x: f32,
    y: f32,
    depth: usize,
    column: usize,
    row: usize,
    elapsed_seconds: f32,
) -> Option<(char, SceneTint)> {
    if depth < 1 || y < 0.78 {
        return None;
    }

    if let Some(ore) = strata_ore_glyph(x, y, column, row) {
        return Some(ore);
    }

    let access_tunnel = ((x - 0.64) / 0.17).powi(2) + ((y - 0.87) / 0.065).powi(2);
    let magma_chamber = ((x - 0.81) / 0.21).powi(2) + ((y - 0.89) / 0.14).powi(2);
    let cavern = access_tunnel.min(magma_chamber);

    if y >= 0.91 && (0.67..=0.93).contains(&x) && magma_chamber < 0.96 {
        let wave = (elapsed_seconds * 2.6 + column as f32 * 0.37 + row as f32 * 0.8).sin();
        return Some((if wave > 0. { '~' } else { '=' }, SceneTint::Lava));
    }

    let lava_center = 0.80 + (y * 22.).sin() * 0.004;
    if (0.79..0.91).contains(&y) && (x - lava_center).abs() < 0.013 && magma_chamber < 1.02 {
        let glow = (elapsed_seconds * 3.1 + row as f32 * 0.9).sin();
        return Some((if glow > -0.25 { '!' } else { '|' }, SceneTint::Lava));
    }

    if cavern > 1.12 {
        return None;
    }
    if cavern > 0.86 {
        return Some((
            if row.is_multiple_of(2) { '#' } else { ':' },
            SceneTint::Stone,
        ));
    }
    if row.is_multiple_of(7) && hash01(column as u32, row as u32, 223) > 0.72 {
        return Some((if y < 0.90 { 'v' } else { '^' }, SceneTint::Stone));
    }
    Some((' ', SceneTint::TerrainNear))
}

fn strata_ore_glyph(x: f32, y: f32, column: usize, row: usize) -> Option<(char, SceneTint)> {
    if x >= 0.56 {
        return None;
    }
    let veins = [
        (0.10, 0.81, 0.055, 0.025, SceneTint::CoalOre),
        (0.29, 0.83, 0.050, 0.025, SceneTint::CoalOre),
        (0.18, 0.87, 0.055, 0.028, SceneTint::IronOre),
        (0.43, 0.88, 0.050, 0.026, SceneTint::IronOre),
        (0.09, 0.92, 0.045, 0.025, SceneTint::LapisOre),
        (0.33, 0.93, 0.055, 0.028, SceneTint::LapisOre),
        (0.23, 0.97, 0.050, 0.025, SceneTint::DiamondOre),
        (0.49, 0.965, 0.042, 0.026, SceneTint::DiamondOre),
    ];
    for (index, (ore_x, ore_y, radius_x, radius_y, tint)) in veins.into_iter().enumerate() {
        let in_vein = ((x - ore_x) / radius_x).powi(2) + ((y - ore_y) / radius_y).powi(2) < 1.;
        let noise = hash01(column as u32, row as u32, 241 + index as u32);
        if in_vein && noise > 0.28 {
            return Some((if noise > 0.68 { '*' } else { 'o' }, tint));
        }
    }
    None
}

fn torch_glyph(
    column: usize,
    row: usize,
    terrain_row: usize,
    columns: usize,
    elapsed_seconds: f32,
) -> Option<(char, SceneTint)> {
    let anchors = [0.39, 0.63];
    for anchor in anchors {
        let torch_column = (anchor * columns as f32) as isize;
        if column as isize != torch_column {
            continue;
        }
        if row + 1 == terrain_row {
            return Some(('!', SceneTint::Warm));
        }
        if row + 2 == terrain_row {
            let flicker = (elapsed_seconds * 7. + anchor * TAU).sin();
            return Some((if flicker > -0.2 { '*' } else { '+' }, SceneTint::Warm));
        }
    }
    None
}

fn moon_glyph(column: usize, row: usize, columns: usize, rows: usize) -> Option<char> {
    let center_column = (columns as f32 * 0.80).round() as isize;
    let center_row = (rows as f32 * 0.16).round() as isize;
    let half_width = (columns / 36).clamp(2, 5) as isize;
    let half_height = (rows / 18).clamp(1, 3) as isize;
    let dx = column as isize - center_column;
    let dy = row as isize - center_row;

    if dx.abs() == half_width + 1 && dy.abs() <= half_height
        || dy.abs() == half_height + 1 && dx.abs() <= half_width
    {
        return Some('.');
    }
    if dx.abs() > half_width || dy.abs() > half_height {
        return None;
    }
    if dy == -half_height || dy == half_height {
        return Some('=');
    }
    if dx == -half_width || dx == half_width {
        return Some('|');
    }
    if dx == half_width / 2 && dy == 0 {
        Some('o')
    } else if dx == -(half_width / 2) && dy == -half_height / 2 {
        Some('@')
    } else {
        Some('#')
    }
}

fn star_glyph(x: f32, y: f32, column: usize, row: usize, elapsed_seconds: f32) -> Option<char> {
    if y > 0.48 {
        return None;
    }
    let density = hash01(column as u32, row as u32, 23);
    let phase = hash01(column as u32, row as u32, 29);
    let twinkle = star_twinkle(phase, elapsed_seconds);
    let threshold = if (0.32..0.68).contains(&x) && y > 0.12 {
        0.989
    } else {
        0.975
    };
    if density > threshold && twinkle > 0.18 {
        Some(if twinkle > 0.72 { '*' } else { '.' })
    } else {
        None
    }
}

fn star_twinkle(seed: f32, elapsed_seconds: f32) -> f32 {
    (elapsed_seconds * 0.9 + seed * TAU).sin() * 0.5 + 0.5
}

fn lake_water_glyph(
    x: f32,
    row: usize,
    column: usize,
    rows: usize,
    elapsed_seconds: f32,
) -> Option<(char, SceneTint)> {
    let lake = lake_rows_at(x, rows)?;
    if row < lake.waterline || row >= lake.bed {
        return None;
    }
    let depth = row - lake.waterline;
    let wave = (elapsed_seconds * 1.7 + column as f32 * 0.42).sin();
    let noise = hash01(column as u32, depth as u32, 101);
    let glyph = if depth == 0 {
        if wave > 0. { '~' } else { '=' }
    } else if noise > 0.66 {
        '~'
    } else if noise > 0.28 {
        '-'
    } else {
        '.'
    };
    Some((glyph, SceneTint::Water))
}

fn firefly_glyph(x: f32, y: f32, column: usize, row: usize, elapsed_seconds: f32) -> Option<char> {
    if !(0.58..0.88).contains(&y) {
        return None;
    }
    let seed = hash01(column as u32, row as u32, 71);
    let pulse = (elapsed_seconds * 1.4 + seed * TAU).sin() * 0.5 + 0.5;
    if seed > 0.991 && pulse > 0.36 && (x * 11. + elapsed_seconds * 0.02).sin() > -0.8 {
        Some(if pulse > 0.75 { '*' } else { '+' })
    } else {
        None
    }
}

fn ramp_glyph(value: f32) -> char {
    let index = (value.clamp(0., 1.) * (GLYPH_RAMP.len() - 1) as f32).round() as usize;
    GLYPH_RAMP[index] as char
}

fn hash01(x: u32, y: u32, seed: u32) -> f32 {
    let mut value = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_147_483_647));
    value = (value ^ (value >> 13)).wrapping_mul(1_274_126_177);
    ((value ^ (value >> 16)) as f32) / u32::MAX as f32
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gpui::{Pixels, Size, px, size};

    use super::{
        BiomeZone, SceneFrame, SceneTint, SurfaceKind, TerrainLayer, TerrainSample, biome_zone,
        cave_glyph, generate_scene, hash01, lake_rows_at, lake_water_glyph, line_tint, moon_glyph,
        oak_shape, spruce_shape, star_glyph, star_twinkle, surface_rows_at, terrain_glyph,
        tree_glyph, watcher_glyph,
    };

    #[test]
    fn scene_generation_is_deterministic_and_bounded() {
        let frame = SceneFrame {
            columns: 48,
            rows: 18,
            elapsed_seconds: 0.,
        };
        let first = generate_scene(frame);
        let second = generate_scene(frame);

        assert_eq!(first, second);
        assert_eq!(first.len(), 18);
        assert!(first.iter().all(|line| line.text.len() == 48));
        assert!(
            first
                .iter()
                .flat_map(|line| line.text.chars())
                .all(|ch| ch.is_ascii())
        );
    }

    #[test]
    fn ambient_effects_change_without_moving_the_terrain() {
        let first = generate_scene(SceneFrame {
            columns: 64,
            rows: 24,
            elapsed_seconds: 0.,
        });
        let later = generate_scene(SceneFrame {
            columns: 64,
            rows: 24,
            elapsed_seconds: 8.,
        });

        assert_ne!(first, later);
        assert!(later.iter().all(|line| line.text.len() == 64));

        let terrain_rows = first
            .iter()
            .zip(later.iter())
            .enumerate()
            .filter(|(row, _)| *row > 12)
            .all(|(_, (before, after))| {
                before
                    .text
                    .chars()
                    .zip(after.text.chars())
                    .filter(|(left, right)| *left != ' ' && *right != ' ')
                    .count()
                    > 0
            });
        assert!(terrain_rows);
    }

    #[test]
    fn viewport_time_is_clamped_to_the_scene_grid() {
        let frame = SceneFrame::for_viewport(size(px(2_000.), px(2_000.)), Duration::from_secs(3));
        assert_eq!(frame.columns, 160);
        assert_eq!(frame.rows, 48);

        let small = SceneFrame::for_viewport(
            Size {
                width: Pixels::ZERO,
                height: Pixels::ZERO,
            },
            Duration::ZERO,
        );
        assert_eq!(small.columns, 48);
        assert_eq!(small.rows, 18);
    }

    #[test]
    fn depth_bands_have_stable_roles() {
        assert_eq!(line_tint(0.20), SceneTint::Sky);
        assert_eq!(line_tint(0.40), SceneTint::Haze);
        assert_eq!(line_tint(0.55), SceneTint::TerrainFar);
        assert_eq!(line_tint(0.68), SceneTint::TerrainMid);
        assert_eq!(line_tint(0.90), SceneTint::TerrainNear);
    }

    #[test]
    fn voxel_motifs_keep_strata_and_tree_silhouettes_distinct() {
        let grass = terrain_glyph(TerrainSample {
            layer: TerrainLayer::Near,
            depth: 0,
            noise: 0.70,
            column: 3,
            row: 18,
            columns: 64,
            rows: 24,
            surface_kind: SurfaceKind::Land,
        });
        let dirt = terrain_glyph(TerrainSample {
            layer: TerrainLayer::Near,
            depth: 1,
            noise: 0.60,
            column: 3,
            row: 19,
            columns: 64,
            rows: 24,
            surface_kind: SurfaceKind::Land,
        });
        let stone = terrain_glyph(TerrainSample {
            layer: TerrainLayer::Near,
            depth: 2,
            noise: 0.60,
            column: 3,
            row: 20,
            columns: 64,
            rows: 24,
            surface_kind: SurfaceKind::Land,
        });

        assert_eq!(grass.1, SceneTint::Grass);
        assert_eq!(dirt.1, SceneTint::Dirt);
        assert_eq!(stone.1, SceneTint::Stone);
        assert_eq!(spruce_shape(6, 6, 0), Some('#'));
        assert_eq!(spruce_shape(1, 6, 0), Some('|'));
        assert_eq!(oak_shape(1, 6, 0), Some('|'));
        assert!(oak_shape(4, 6, 1).is_some());
    }

    #[test]
    fn former_navigation_band_contains_texture_without_being_solid() {
        let frame = SceneFrame {
            columns: 64,
            rows: 24,
            elapsed_seconds: 2.5,
        };
        let scene = generate_scene(frame);
        let center_start = frame.columns / 3;
        let center_end = frame.columns * 2 / 3;
        let lower_center: Vec<_> = scene
            .iter()
            .skip(frame.rows * 4 / 5)
            .flat_map(|line| line.text[center_start..center_end].chars())
            .collect();
        let filled = lower_center.iter().filter(|ch| **ch != ' ').count();
        assert!(filled > 0);
        assert!(filled < lower_center.len());
    }

    #[test]
    fn stars_twinkle_in_place() {
        let changed = (0..64)
            .flat_map(|column| (0..24).map(move |row| (column, row)))
            .any(|(column, row)| {
                star_glyph(0.20, 0.20, column, row, 0.) != star_glyph(0.20, 0.20, column, row, 4.)
            });
        assert!(changed);
        assert_ne!(star_twinkle(0.4, 0.), star_twinkle(0.4, 1.));
    }

    #[test]
    fn moon_and_watcher_have_distinct_ascii_silhouettes() {
        let columns = 120;
        let rows = 40;
        let moon: Vec<_> = (0..rows)
            .flat_map(|row| {
                (0..columns).filter_map(move |column| moon_glyph(column, row, columns, rows))
            })
            .collect();
        assert!(moon.contains(&'='));
        assert!(moon.contains(&'|'));
        assert!(moon.contains(&'#'));
        assert!(moon.contains(&'o'));

        let watcher_surface = surface_rows_at(0.88, columns, rows).mid;
        let watcher: Vec<_> = (0..rows)
            .flat_map(|row| {
                (0..columns).filter_map(move |column| {
                    watcher_glyph(column, row, watcher_surface, columns, rows)
                })
            })
            .collect();
        assert!(watcher.iter().any(|(_, tint)| *tint == SceneTint::Mob));
        assert!(watcher.iter().any(|(_, tint)| *tint == SceneTint::Warm));
    }

    #[test]
    fn block_canopies_separate_the_forest_from_the_plains() {
        let columns = 120;
        let rows = 40;
        let surfaces: Vec<_> = (0..columns)
            .map(|column| surface_rows_at(column as f32 / (columns - 1) as f32, columns, rows))
            .collect();
        let mut forest_glyphs = 0;
        let mut plains_glyphs = 0;
        for column in 0..columns {
            let x = column as f32 / (columns - 1) as f32;
            for row in 0..rows {
                if tree_glyph(column, row, &surfaces, columns, rows).is_some() {
                    match biome_zone(x) {
                        BiomeZone::Forest => forest_glyphs += 1,
                        BiomeZone::Plains => plains_glyphs += 1,
                        BiomeZone::Meadow | BiomeZone::Lake => {}
                    }
                }
            }
        }
        assert!(forest_glyphs > plains_glyphs);

        let tree_glyphs: Vec<_> =
            (1..=8)
                .flat_map(|level| (-6..=6).filter_map(move |offset| spruce_shape(level, 8, offset)))
                .chain((1..=8).flat_map(|level| {
                    (-6..=6).filter_map(move |offset| oak_shape(level, 8, offset))
                }))
                .collect();
        assert!(tree_glyphs.contains(&'#'));
        assert!(tree_glyphs.contains(&'*'));
        assert!(tree_glyphs.contains(&'|'));
        assert!(!tree_glyphs.iter().any(|glyph| matches!(glyph, '/' | '\\')));
    }

    #[test]
    fn lake_water_interrupts_the_grass_cap_and_owns_the_full_basin() {
        let rows = 40;
        let x = 0.53;
        let lake = lake_rows_at(x, rows).expect("lake center should own a basin");
        assert!(lake.bed > lake.waterline + 1);
        for row in lake.waterline..lake.bed {
            let water = lake_water_glyph(x, row, 64, rows, 2.0);
            assert!(water.is_some_and(|(_, tint)| tint == SceneTint::Water));
        }
        assert_eq!(lake_water_glyph(x, lake.bed, 64, rows, 2.0), None);
        assert_eq!(surface_rows_at(x, 120, rows).near, lake.bed);

        let lakebed = terrain_glyph(TerrainSample {
            layer: TerrainLayer::Near,
            depth: 0,
            noise: 0.70,
            column: 64,
            row: lake.bed,
            columns: 120,
            rows,
            surface_kind: SurfaceKind::LakeBed,
        });
        assert_ne!(lakebed.1, SceneTint::Grass);
    }

    #[test]
    fn ore_strata_descend_from_coal_to_diamond_beside_the_lava_cave() {
        let columns = 120;
        let rows = 40;

        let mut has_void = false;
        let mut coal_rows = Vec::new();
        let mut iron_rows = Vec::new();
        let mut lapis_rows = Vec::new();
        let mut diamond_rows = Vec::new();
        let mut fall_rows = Vec::new();
        let mut lake_rows = Vec::new();
        for row in 0..rows {
            let y = row as f32 / (rows - 1) as f32;
            for column in 0..columns {
                let x = column as f32 / (columns - 1) as f32;
                let surface = surface_rows_at(x, columns, rows).near;
                let depth = row.saturating_sub(surface);
                if let Some((glyph, tint)) = cave_glyph(x, y, depth, column, row, 2.0) {
                    has_void |= glyph == ' ';
                    match tint {
                        SceneTint::CoalOre => coal_rows.push(row),
                        SceneTint::IronOre => iron_rows.push(row),
                        SceneTint::LapisOre => lapis_rows.push(row),
                        SceneTint::DiamondOre => diamond_rows.push(row),
                        SceneTint::Lava if y < 0.91 => fall_rows.push(row),
                        SceneTint::Lava => lake_rows.push(row),
                        _ => {}
                    }
                }
            }
        }
        assert!(has_void);
        assert!(!coal_rows.is_empty());
        assert!(!iron_rows.is_empty());
        assert!(!lapis_rows.is_empty());
        assert!(!diamond_rows.is_empty());
        assert!(coal_rows.iter().min() < iron_rows.iter().min());
        assert!(iron_rows.iter().min() < lapis_rows.iter().min());
        assert!(lapis_rows.iter().min() < diamond_rows.iter().min());
        assert!(!fall_rows.is_empty());
        assert!(!lake_rows.is_empty());
        fall_rows.sort_unstable();
        fall_rows.dedup();
        lake_rows.sort_unstable();
        lake_rows.dedup();
        assert!(lake_rows.len() >= 2);
        assert!(fall_rows.last().copied().unwrap() + 1 >= lake_rows[0]);
    }

    #[test]
    fn stepped_profiles_are_layered_and_repeat_across_neighboring_columns() {
        let columns = 120;
        let rows = 40;
        let profiles: Vec<_> = (0..columns)
            .map(|column| surface_rows_at(column as f32 / (columns - 1) as f32, columns, rows))
            .collect();
        assert!(
            profiles
                .iter()
                .all(|surface| surface.far < surface.mid && surface.mid < surface.near)
        );
        let repeated_steps = profiles
            .windows(2)
            .filter(|pair| pair[0].far == pair[1].far)
            .count();
        assert!(repeated_steps > columns / 2);
    }

    #[test]
    fn central_sky_keeps_breathing_room_for_home_content() {
        let frame = SceneFrame {
            columns: 120,
            rows: 40,
            elapsed_seconds: 3.5,
        };
        let scene = generate_scene(frame);
        let glyphs: Vec<_> = scene
            .iter()
            .take(frame.rows * 12 / 20)
            .skip(frame.rows / 8)
            .flat_map(|line| line.text[frame.columns / 3..frame.columns * 2 / 3].chars())
            .collect();
        let filled = glyphs.iter().filter(|glyph| **glyph != ' ').count();
        assert!(filled * 5 < glyphs.len());
    }

    #[test]
    fn hash_is_stable_for_fixed_coordinates() {
        assert_eq!(hash01(4, 9, 17), hash01(4, 9, 17));
        assert_ne!(hash01(4, 9, 17), hash01(5, 9, 17));
    }
}
