use super::*;

#[test]
fn rotation_and_signed_size_follow_litematica_corner_transform() {
    let global = Transform {
        mirror: "NONE",
        rotation: "CLOCKWISE_90",
    };
    let local = Transform {
        mirror: "LEFT_RIGHT",
        rotation: "NONE",
    };
    let points = footprint([100, 64, -20], [2, 0, 3], (-2, 1, 3), global, local).unwrap();
    // Relative origin (2, 3) rotates to (-3, 2). The signed far corner
    // (-1, 2) rotates to (-2, -1), then the local mirror flips its Z.
    assert_eq!(points[0], MapPoint { x: 95., z: -18. });
    assert_eq!(points[2], MapPoint { x: 98., z: -16. });
}

#[test]
fn malformed_extreme_placement_does_not_overflow() {
    let identity = Transform {
        mirror: "FRONT_BACK",
        rotation: "CLOCKWISE_180",
    };
    assert!(footprint([i64::MIN, 0, 0], [0; 3], (1, 1, 1), identity, identity).is_none());
    assert!(footprint([0; 3], [0; 3], (i64::MIN, 1, 1), identity, identity).is_none());
}
