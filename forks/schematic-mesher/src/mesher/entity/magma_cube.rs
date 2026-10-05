use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Magma cube model — texture `entity/slime/magmacube`, 64x64.
/// From MagmaCubeModel.java (MC 26.2): eight 8x1x8 body segments stacked from
/// y 16 to 24 — the accordion the squish animation plays on — plus a 4x4x4
/// core. Segment UVs walk the sheet exactly as createBodyLayer lays them out:
/// rows 0..3 down the left edge, rows 4..7 down from (32, 0).
pub(super) fn magma_cube_model() -> EntityModelDef {
    let mut children: Vec<EntityPart> = (0..8)
        .map(|i| {
            let (u, v) = match i {
                0 => (0, 0),
                1..=3 => (0, 9 * i as u32),
                _ => (32, 9 * i as u32 - 36),
            };
            EntityPart {
                cubes: vec![EntityCube {
                    origin: [-4.0, 16.0 + i as f32, -4.0],
                    dimensions: [8.0, 1.0, 8.0],
                    tex_offset: [u, v],
                    inflate: 0.0,
                    mirror: false,
                    skip_faces: vec![],
                }],
                pose: Default::default(),
                children: vec![],
            }
        })
        .collect();

    children.push(EntityPart {
        cubes: vec![EntityCube {
            origin: [-2.0, 18.0, -2.0],
            dimensions: [4.0, 4.0, 4.0],
            tex_offset: [24, 40],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    });

    // Y-down -> Y-up root wrapper.
    let root = EntityPart {
        cubes: vec![],
        pose: EntityPartPose {
            position: [8.0, 24.0, 8.0],
            rotation: [std::f32::consts::PI, 0.0, 0.0],
            ..Default::default()
        },
        children,
    };

    EntityModelDef {
        texture_path: "entity/slime/magmacube".to_string(),
        texture_size: [64, 64],
        parts: vec![root],
        is_opaque: true,
    }
}
