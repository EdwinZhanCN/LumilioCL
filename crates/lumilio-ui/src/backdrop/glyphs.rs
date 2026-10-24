use super::{GLYPH_RAMP, SceneTint, SurfaceRows, TAU, TerrainLayer, hash01, lake_rows_at};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TreeKind {
    Spruce,
    Oak,
}

pub(super) fn tree_glyph(
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

pub(super) fn spruce_shape(level: isize, height: isize, offset: isize) -> Option<char> {
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

pub(super) fn oak_shape(level: isize, height: isize, offset: isize) -> Option<char> {
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

pub(super) fn watcher_glyph(
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

pub(super) fn cave_glyph(
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

pub(super) fn strata_ore_glyph(
    x: f32,
    y: f32,
    column: usize,
    row: usize,
) -> Option<(char, SceneTint)> {
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

pub(super) fn torch_glyph(
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

pub(super) fn moon_glyph(column: usize, row: usize, columns: usize, rows: usize) -> Option<char> {
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

pub(super) fn star_glyph(
    x: f32,
    y: f32,
    column: usize,
    row: usize,
    elapsed_seconds: f32,
) -> Option<char> {
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

pub(super) fn star_twinkle(seed: f32, elapsed_seconds: f32) -> f32 {
    (elapsed_seconds * 0.9 + seed * TAU).sin() * 0.5 + 0.5
}

pub(super) fn lake_water_glyph(
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

pub(super) fn firefly_glyph(
    x: f32,
    y: f32,
    column: usize,
    row: usize,
    elapsed_seconds: f32,
) -> Option<char> {
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

pub(super) fn ramp_glyph(value: f32) -> char {
    let index = (value.clamp(0., 1.) * (GLYPH_RAMP.len() - 1) as f32).round() as usize;
    GLYPH_RAMP[index] as char
}
