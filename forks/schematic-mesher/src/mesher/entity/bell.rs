use super::{EntityCube, EntityModelDef, EntityPart, EntityPartPose};

/// Bell model (32x32 texture). Only the bell body — frame is from JSON model.
pub(super) fn bell_model() -> EntityModelDef {
    // Bell body: 6x7x6 at texOffs(0,0)
    let bell_body = EntityPart {
        cubes: vec![EntityCube {
            origin: [-3.0, -7.0, -3.0],
            dimensions: [6.0, 7.0, 6.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [8.0, 12.0, 8.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Bell lip: 8x2x8 at bottom of body
    let bell_lip = EntityPart {
        cubes: vec![EntityCube {
            origin: [-4.0, -9.0, -4.0],
            dimensions: [8.0, 2.0, 8.0],
            tex_offset: [0, 13],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [8.0, 12.0, 8.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: "entity/bell/bell_body".to_string(),
        texture_size: [32, 32],
        parts: vec![bell_body, bell_lip],
        is_opaque: true,
    }
}
