//! Picking (plan schematic-studio T10): the voxel walk, the camera ray, and the scene's pick and
//! transparent export on a GPU.

use lumilio_block_assets::EntityLayers;
use nucleation::UniversalSchematic;

use super::tiny_pack;
use crate::{Angle, Grid, Ray, Scene, SceneError, View, march};

const GRID: Grid = Grid {
    min: [0, 0, 0],
    max: [4, 4, 4],
};

fn along_x(y: f32, z: f32) -> Ray {
    Ray {
        origin: [-3.0, y, z],
        direction: [1.0, 0.0, 0.0],
    }
}

#[test]
fn the_walk_stops_at_the_first_block() {
    let hit = march(along_x(2.5, 2.5), GRID, |cell| cell[0] >= 1);
    assert_eq!(hit, Some([1, 2, 2]));
    // A diagonal ray that enters through a corner region.
    let diagonal = Ray {
        origin: [-1.0, -1.0, -1.0],
        direction: [1.0, 1.0, 1.0],
    };
    assert_eq!(
        march(diagonal, GRID, |cell| cell == [3, 3, 3]),
        Some([3, 3, 3])
    );
}

#[test]
fn the_walk_passes_through_air() {
    let mut visited = Vec::new();
    let hit = march(along_x(0.5, 4.5), GRID, |cell| {
        visited.push(cell);
        cell == [4, 0, 4]
    });
    assert_eq!(hit, Some([4, 0, 4]));
    assert_eq!(
        visited,
        [[0, 0, 4], [1, 0, 4], [2, 0, 4], [3, 0, 4], [4, 0, 4]],
        "every air cell on the way is visited once, in order"
    );
    assert_eq!(march(along_x(0.5, 0.5), GRID, |_| false), None);
}

#[test]
fn a_ray_outside_the_grid_finds_nothing() {
    // Passes above the grid.
    assert_eq!(march(along_x(7.0, 2.0), GRID, |_| true), None);
    // Points away from it.
    let away = Ray {
        origin: [-3.0, 2.0, 2.0],
        direction: [-1.0, 0.0, 0.0],
    };
    assert_eq!(march(away, GRID, |_| true), None);
    // A grid with no cells.
    let empty = Grid {
        min: [0, 0, 0],
        max: [-1, 0, 0],
    };
    assert_eq!(march(along_x(0.5, 0.5), empty, |_| true), None);
    // Cells beyond the grid are never asked about.
    let hit = march(along_x(2.5, 2.5), GRID, |cell| {
        assert!(cell[0] <= 4, "asked about {cell:?}");
        false
    });
    assert_eq!(hit, None);
}

#[test]
fn the_middle_of_the_picture_looks_at_the_middle_of_the_build() {
    use nucleation::rendering::CameraConfig;
    // A 4 × 4 × 4 block mesh (blocks 0..=3 at -0.5..=3.5).
    let bounds = ([-0.5; 3], [3.5; 3]);
    for angle in Angle::ALL {
        let view = View::new().at(angle);
        let camera = CameraConfig {
            yaw_deg: view.yaw_deg,
            pitch_deg: view.pitch_deg,
            sphere_fit: true,
            ..CameraConfig::default()
        };
        let ray = crate::camera_ray(&camera, bounds, 64, 48, 32.0, 24.0).unwrap();
        let grid = Grid {
            min: [0; 3],
            max: [3; 3],
        };
        let mut first = None;
        march(ray, grid, |cell| {
            first.get_or_insert(cell);
            true
        });
        let first = first.expect("the ray enters the build");
        match angle {
            Angle::North => assert_eq!(first[2], 0, "north looks at the -Z face"),
            Angle::South => assert_eq!(first[2], 3, "south looks at the +Z face"),
            Angle::East => assert_eq!(first[0], 3, "east looks at the +X face"),
            Angle::West => assert_eq!(first[0], 0, "west looks at the -X face"),
            Angle::Top => assert_eq!(first[1], 3, "top looks down on the roof"),
            Angle::Isometric => assert_eq!(first, [3, 3, 3], "the near top corner"),
        }
    }
}

fn gpu_scene(schematic: UniversalSchematic) -> Option<Scene> {
    match Scene::from_schematic(schematic, &tiny_pack(), EntityLayers::new()) {
        Ok(scene) => Some(scene),
        Err(SceneError::NoGpu) => {
            eprintln!("skipped: no graphics adapter on this machine");
            None
        }
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn a_click_picks_the_block_and_its_state() {
    let mut schematic = UniversalSchematic::new("pick".into());
    schematic.fill_cuboid_str((0, 0, 0), (3, 0, 3), "minecraft:stone");
    schematic.set_block_str(1, 1, 1, "minecraft:stone[snowy=true]");
    let Some(scene) = gpu_scene(schematic) else {
        return;
    };
    let top = View::new().at(Angle::Top);
    // Looking down, the middle of the picture is the middle of the 4 × 4 floor: block (1 or 2,
    // 0, 1 or 2), or the raised block at (1, 1, 1).
    let picked = scene.pick(&top, 64, 64, 32.0, 32.0).expect("a block");
    assert!((1..=2).contains(&picked.position[0]), "{picked:?}");
    assert!(picked.state.starts_with("minecraft:stone"), "{picked:?}");
    // The corner of the picture is empty space.
    assert_eq!(scene.pick(&top, 64, 64, 0.5, 0.5), None);
}

#[test]
fn a_transparent_export_is_the_requested_size_with_clear_empty_space() {
    let mut schematic = UniversalSchematic::new("export".into());
    schematic.fill_cuboid_str((0, 0, 0), (1, 1, 1), "minecraft:stone");
    let Some(mut scene) = gpu_scene(schematic) else {
        return;
    };
    let view = View {
        background: Some([0.2, 0.3, 0.4, 0.0]),
        ..View::new().at(Angle::Isometric)
    };
    let frame = scene.render(&view, 300, 200).unwrap();
    assert_eq!((frame.width, frame.height), (300, 200));
    assert_eq!(frame.bgra.len(), 300 * 200 * 4);
    assert_eq!(frame.bgra[3], 0, "the corner is transparent");
    let centre = (100 * 300 + 150) * 4;
    assert_eq!(frame.bgra[centre + 3], 255, "the block is opaque");
}
