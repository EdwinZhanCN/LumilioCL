use super::{
    ChestVariant, DoubleChestSide, EntityCube, EntityModelDef, EntityPart, EntityPartPose,
};

/// Single chest model (64x64 texture).
/// Values from EntityModelJson dump (Minecraft 1.19).
pub(super) fn chest_model(variant: ChestVariant) -> EntityModelDef {
    let texture_path = match variant {
        ChestVariant::Normal => "entity/chest/normal",
        ChestVariant::Trapped => "entity/chest/trapped",
        ChestVariant::Ender => "entity/chest/ender",
        ChestVariant::Christmas => "entity/chest/christmas",
    };

    // Bottom: origin [1,0,1], dims [14,10,14], texOffs(0,19)
    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.0, 0.0, 1.0],
            dimensions: [14.0, 10.0, 14.0],
            tex_offset: [0, 19],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    // Lid: origin [1,0,0], dims [14,5,14], texOffs(0,0), pose position=[0,9,1]
    let lid = EntityPart {
        cubes: vec![EntityCube {
            origin: [1.0, 0.0, 0.0],
            dimensions: [14.0, 5.0, 14.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    // Lock: origin [7,-1,15], dims [2,4,1], texOffs(0,0), pose position=[0,8,0]
    let lock = EntityPart {
        cubes: vec![EntityCube {
            origin: [7.0, -1.0, 15.0],
            dimensions: [2.0, 4.0, 1.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 8.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path: texture_path.to_string(),
        texture_size: [64, 64],
        parts: vec![bottom, lid, lock],
        is_opaque: true,
    }
}

/// Double chest model (uses left/right textures).
pub(super) fn double_chest_model(variant: ChestVariant, side: DoubleChestSide) -> EntityModelDef {
    let base = match variant {
        ChestVariant::Normal => "entity/chest/normal",
        ChestVariant::Trapped => "entity/chest/trapped",
        ChestVariant::Ender => "entity/chest/ender",
        ChestVariant::Christmas => "entity/chest/christmas",
    };
    let suffix = match side {
        DoubleChestSide::Left => "_left",
        DoubleChestSide::Right => "_right",
    };
    let texture_path = format!("{}{}", base, suffix);

    // Double chest values from EntityModelJson dump.
    // Each half is 15 units wide with a 2-unit gap between halves (intentional seam).
    let (bottom_origin, lid_origin, lock_origin, lock_dims) = match side {
        DoubleChestSide::Left => ([0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [0.0, -1.0, 15.0], [1.0, 4.0, 1.0]),
        DoubleChestSide::Right => ([1.0, 0.0, 1.0], [1.0, 0.0, 0.0], [15.0, -1.0, 15.0], [1.0, 4.0, 1.0]),
    };

    let bottom = EntityPart {
        cubes: vec![EntityCube {
            origin: bottom_origin,
            dimensions: [15.0, 10.0, 14.0],
            tex_offset: [0, 19],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: Default::default(),
        children: vec![],
    };

    let lid = EntityPart {
        cubes: vec![EntityCube {
            origin: lid_origin,
            dimensions: [15.0, 5.0, 14.0],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 9.0, 1.0],
            ..Default::default()
        },
        children: vec![],
    };

    let lock = EntityPart {
        cubes: vec![EntityCube {
            origin: lock_origin,
            dimensions: [lock_dims[0], lock_dims[1], lock_dims[2]],
            tex_offset: [0, 0],
            inflate: 0.0,
            mirror: false,
            skip_faces: vec![],
        }],
        pose: EntityPartPose {
            position: [0.0, 8.0, 0.0],
            ..Default::default()
        },
        children: vec![],
    };

    EntityModelDef {
        texture_path,
        texture_size: [64, 64],
        parts: vec![bottom, lid, lock],
        is_opaque: true,
    }
}
