use std::time::Duration;

use gpui::{Pixels, Size, px, size};

use super::{
    BiomeZone, SceneFrame, SceneTint, SurfaceKind, TerrainLayer, TerrainSample, biome_zone,
    cave_glyph, generate_scene, hash01, lake_rows_at, lake_water_glyph, line_tint, moon_glyph,
    oak_shape, spruce_shape, star_glyph, star_twinkle, surface_rows_at, terrain_glyph, tree_glyph,
    watcher_glyph,
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

    let tree_glyphs: Vec<_> = (1..=8)
        .flat_map(|level| (-6..=6).filter_map(move |offset| spruce_shape(level, 8, offset)))
        .chain(
            (1..=8)
                .flat_map(|level| (-6..=6).filter_map(move |offset| oak_shape(level, 8, offset))),
        )
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
