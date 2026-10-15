use super::*;
use crate::meshing::{turned_at, EntityInstance, EntityModel, MeshLayer, MeshOutput, MODEL_FEET};
use crate::rendering::camera::{look_at, mat4_inverse, mat4_mul, ortho, perspective_reversed};

/// Eye at the origin looking down -Z, 90° field of view, reversed depth.
fn looking_down_minus_z() -> [[f32; 4]; 4] {
    let view = look_at([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]);
    mat4_mul(perspective_reversed(90f32.to_radians(), 1.0, 0.05), view)
}

#[test]
fn spheres_in_front_are_seen_and_those_behind_or_beside_are_not() {
    let frustum = Frustum::from_view_proj(&looking_down_minus_z());
    // The infinite far plane is dropped; four sides and the near plane remain.
    assert_eq!(frustum.planes.len(), 5);
    assert!(frustum.sees([0.0, 0.0, -10.0], 0.5));
    assert!(frustum.sees([0.0, 0.0, -10_000.0], 0.5), "no far limit");
    assert!(!frustum.sees([0.0, 0.0, 10.0], 0.5), "behind the eye");
    // 90°: at depth 10 the edge is x = 10.
    assert!(!frustum.sees([12.0, 0.0, -10.0], 1.0));
    // A sphere across the edge is kept.
    assert!(frustum.sees([10.5, 0.0, -10.0], 1.0));
    assert!(!frustum.sees([0.0, -12.0, -10.0], 1.0));
}

#[test]
fn an_orthographic_view_also_has_a_far_plane() {
    let view = look_at([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]);
    // Reversed: near and far swapped, as the renderer builds it.
    let frustum =
        Frustum::from_view_proj(&mat4_mul(ortho(-5.0, 5.0, -5.0, 5.0, 100.0, 0.01), view));
    assert_eq!(frustum.planes.len(), 6);
    assert!(frustum.sees([4.0, 4.0, -50.0], 0.1));
    assert!(!frustum.sees([0.0, 0.0, -150.0], 1.0));
    assert!(!frustum.sees([6.0, 0.0, -50.0], 0.5));
}

#[test]
fn the_distance_limit_counts_from_the_eye_to_the_sphere_surface() {
    let frustum = Frustum::from_view_proj(&looking_down_minus_z());
    let culling = Culling {
        frustum: &frustum,
        eye: [0.0; 3],
        max_distance: Some(64.0),
        back: [0.0, 0.0, 1.0],
    };
    assert!(culling.keeps([0.0, 0.0, -64.5], 1.0));
    assert!(!culling.keeps([0.0, 0.0, -66.0], 1.0));
    let unlimited = Culling {
        max_distance: None,
        ..culling
    };
    assert!(unlimited.keeps([0.0, 0.0, -6600.0], 1.0));
}

fn model() -> EntityModel {
    EntityModel {
        name: "entity:armor_stand".into(),
        mesh: MeshOutput {
            opaque: MeshLayer::new(),
            cutout: MeshLayer::new(),
            transparent: MeshLayer::new(),
            atlas: schematic_mesher::TextureAtlas::empty(),
            greedy_materials: Vec::new(),
            animated_textures: Vec::new(),
            bounds: schematic_mesher::BoundingBox::new([0.0; 3], [0.0; 3]),
            chunk_coord: None,
            lod_level: 0,
        },
        center: [0.0, 0.5, 0.0],
        radius: 1.0,
    }
}

/// Kept instances are grouped by model, farthest first; the rest are gone.
#[test]
fn a_frame_keeps_visible_instances_per_model_farthest_first() {
    let at = |x: f32, z: f32, model: usize| EntityInstance {
        model,
        transform: turned_at([x, 0.0, z], 0.0, MODEL_FEET),
        billboard: None,
    };
    let entities = EntityInstances {
        models: vec![model(), model()],
        instances: vec![
            at(0.0, -5.0, 0),
            at(0.0, 20.0, 0), // behind
            at(0.0, -40.0, 0),
            at(1.0, -200.0, 1), // too far
            at(1.0, -8.0, 1),
        ],
        undrawable: Vec::new(),
    };
    let frustum = Frustum::from_view_proj(&looking_down_minus_z());
    let kept = visible(
        &spheres(&entities),
        &entities.instances,
        &Culling {
            frustum: &frustum,
            eye: [0.0; 3],
            max_distance: Some(96.0),
            back: [0.0, 0.0, 1.0],
        },
    );
    let z = |t: &Transform| t[3][2];
    assert_eq!(kept[0].iter().map(z).collect::<Vec<_>>(), [-40.0, -5.0]);
    assert_eq!(kept[1].iter().map(z).collect::<Vec<_>>(), [-8.0]);
}

#[test]
fn the_eye_is_the_first_person_position_or_the_near_plane_centre() {
    let m = looking_down_minus_z();
    assert_eq!(
        eye(Some([1.0, 2.0, 3.0]), &mat4_inverse(m), true),
        [1.0, 2.0, 3.0]
    );
    let near = eye(None, &mat4_inverse(m), true);
    let distance = near.iter().map(|v| v * v).sum::<f32>().sqrt();
    assert!(distance < 0.06, "{near:?}");
}
