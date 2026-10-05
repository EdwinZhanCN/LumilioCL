use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Book model for lecterns and enchanting tables (64x32 texture).
/// Cube geometry from BookModel.java (MC 1.21.5); static pose baked from
/// `setupAnim(_, f2, f3, f4=1)` — i.e. the book fully open at rest.
pub(super) fn book_model() -> EntityModelDef {
    // MC setupAnim: f5 = (sin(time*0.02)*0.1 + 1.25) * f4, static f4=1 ⇒ f5≈1.25.
    const F5: f32 = 1.25;
    const SIN_F5: f32 = 0.9489846;
    // Page flip fractions — pick small asymmetric values so flip pages don't
    // coincide with each other or with the page stacks (prevents z-fighting).
    const F2: f32 = 0.1;
    const F3: f32 = 0.9;

    // Left lid: yRot = PI + f5.
    let left_lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [-6.0, -5.0, -0.005],
            dimensions: [6.0, 10.0, 0.005],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, -1.0],
            rotation: [0.0, std::f32::consts::PI + F5, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Right lid: yRot = -f5.
    let right_lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -5.0, -0.005],
            dimensions: [6.0, 10.0, 0.005],
            tex_offset: [16, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 0.0, 1.0],
            rotation: [0.0, -F5, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Seam (spine): has a base PartPose rotation of (0, PI/2, 0) in MC.
    let seam = EntityPart {
        cubes: vec![EntityCube {
            origin: [-1.0, -5.0, 0.0],
            dimensions: [2.0, 10.0, 0.005],
            tex_offset: [12, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            rotation: [0.0, std::f32::consts::FRAC_PI_2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Left pages: yRot = f5, x = sin(f5).
    let left_pages = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, -0.99],
            dimensions: [5.0, 8.0, 1.0],
            tex_offset: [0, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [SIN_F5, 0.0, 0.0],
            rotation: [0.0, F5, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Right pages: yRot = -f5, x = sin(f5).
    let right_pages = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, -0.01],
            dimensions: [5.0, 8.0, 1.0],
            tex_offset: [12, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [SIN_F5, 0.0, 0.0],
            rotation: [0.0, -F5, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Flip pages: yRot = f5 - f5*2*flipAmount, x = sin(f5).
    let flip_page1 = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, 0.0],
            dimensions: [5.0, 8.0, 0.005],
            tex_offset: [24, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [SIN_F5, 0.0, 0.0],
            rotation: [0.0, F5 - F5 * 2.0 * F2, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    let flip_page2 = EntityPart {
        cubes: vec![EntityCube {
            origin: [0.0, -4.0, 0.0],
            dimensions: [5.0, 8.0, 0.005],
            tex_offset: [24, 10],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [SIN_F5, 0.0, 0.0],
            rotation: [0.0, F5 - F5 * 2.0 * F3, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: "entity/enchanting_table_book".to_string(),
        texture_size: [64, 32],
        parts: vec![left_lid, right_lid, seam, left_pages, right_pages, flip_page1, flip_page2],
        is_opaque: false,
    }
}
