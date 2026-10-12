//! Decoration candidates against the original servers' own placement
//! modifiers (`Probe.java`, imported by `cargo xtask worldgen-import`; rows
//! `seed kind chunk_x chunk_z x y z` in placement order). The probe passes the
//! filters that read the world, so the rows are the raw random sequence.
use crate::{Dimension, Version, ffi};
use std::collections::BTreeMap;

macro_rules! decoration_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!("../../tests/golden/", $version, "-decorations.txt")),
        )
    };
}
const DECORATIONS: &[(&str, &str)] = &[
    decoration_golden!("1.19.4"),
    decoration_golden!("1.20.6"),
    decoration_golden!("1.21.1"),
    decoration_golden!("1.21.3"),
    decoration_golden!("1.21.4"),
    decoration_golden!("1.21.5"),
    decoration_golden!("1.21.6"),
    decoration_golden!("1.21.7"),
    decoration_golden!("1.21.8"),
    decoration_golden!("1.21.9"),
    decoration_golden!("1.21.10"),
    decoration_golden!("1.21.11"),
    decoration_golden!("26.1"),
    decoration_golden!("26.1.1"),
    decoration_golden!("26.1.2"),
    decoration_golden!("26.2"),
    decoration_golden!("26.3"),
];

/// The fork's `DecorationType` order: (dimension, counted per chunk).
const KINDS: [(Dimension, bool); 11] = [
    (Dimension::Overworld, false), // fossil_upper
    (Dimension::Overworld, false), // fossil_lower
    (Dimension::Overworld, false), // lake_lava_surface
    (Dimension::Overworld, false), // lake_lava_underground
    (Dimension::Overworld, true),  // monster_room
    (Dimension::Overworld, true),  // monster_room_deep
    (Dimension::Overworld, true),  // rooted_sulfur_spring
    (Dimension::Overworld, false), // amethyst_geode
    (Dimension::Overworld, false), // desert_well
    (Dimension::End, false),       // end_island_decorated
    (Dimension::End, false),       // end_gateway_return
];

#[test]
fn decoration_candidates_follow_the_original_placement_modifiers() {
    for &(name, rows) in DECORATIONS {
        let version = Version::from_name(name).unwrap();
        let mut expected: BTreeMap<(i64, i32, i32, i32), Vec<[i32; 3]>> = BTreeMap::new();
        for line in rows.lines() {
            let f: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, kind, cx, cz, x, y, z] = f.as_slice() else {
                panic!("invalid decoration row")
            };
            expected
                .entry((*seed, *kind as i32, *cx as i32, *cz as i32))
                .or_default()
                .push([*x as i32, *y as i32, *z as i32]);
        }
        let mut compared = 0;
        for (kind, (dimension, counted)) in KINDS.into_iter().enumerate() {
            let kind = kind as i32;
            let edge = if counted { 4 } else { 16 };
            for seed in [262_i64, 9_876_543_210, -123_456_789_012_345] {
                for cz in -edge / 2..edge / 2 {
                    for cx in -edge / 2..edge / 2 {
                        let want = expected.remove(&(seed, kind, cx, cz)).unwrap_or_default();
                        match ffi::decoration_raw(version, dimension, kind, seed, [cx, cz]) {
                            Some(got) => {
                                assert_eq!(got, want, "{name} kind {kind} {seed} {cx} {cz}")
                            }
                            None => assert!(want.is_empty(), "{name} lacks kind {kind}"),
                        }
                        compared += want.len();
                    }
                }
            }
        }
        assert!(expected.is_empty(), "{name}: rows outside the grid");
        assert!(compared > 500, "{name}: {compared} candidates");
    }
}
