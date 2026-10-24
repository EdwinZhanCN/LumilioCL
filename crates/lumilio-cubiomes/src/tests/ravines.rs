//! Ravines and the noises behind veins against original servers' runtime
//! (`Probe.java`, imported by `cargo xtask worldgen-import`).

use std::collections::BTreeMap;

use crate::{Version, ffi};

/// Rows `seed chunk_x chunk_z blocks checksum`: every canyon starting in the
/// chunks [-12, 12)² of the three seeds, carved by the original 26.x carver
/// into a carving mask for each chunk it can reach.
const RAVINES: &[(&str, &str)] = &[("26.3", include_str!("../../tests/golden/26.3-ravines.txt"))];

#[test]
fn ravines_carve_the_blocks_the_original_carver_does() {
    for &(name, rows) in RAVINES {
        let version = Version::from_name(name).unwrap();
        let mut expected = BTreeMap::new();
        for line in rows.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [seed, sx, sz, count, checksum] = f.as_slice() else {
                panic!("invalid ravine row")
            };
            expected.insert(
                (
                    seed.parse::<i64>().unwrap(),
                    sx.parse::<i32>().unwrap(),
                    sz.parse::<i32>().unwrap(),
                ),
                (
                    count.parse::<i64>().unwrap(),
                    checksum.parse::<u64>().unwrap(),
                ),
            );
        }
        let mut starts = 0;
        for seed in [262_i64, 9_876_543_210, -123_456_789_012_345] {
            for sz in -12..12 {
                for sx in -12..12 {
                    let want = expected.remove(&(seed, sx, sz));
                    assert_eq!(
                        ffi::ravine_check(version, seed, [sx, sz]),
                        want,
                        "{name} {seed} {sx} {sz}"
                    );
                    starts += usize::from(want.is_some());
                }
            }
        }
        assert!(expected.is_empty(), "{name}: starts outside the grid");
        assert!(starts >= 10, "{name}: {starts} ravines");
    }
}

/// Rows `value sin cos`: `Mth`'s table lookups for float inputs. 1.21.4 reads
/// the table with a float product, 26.3 with a double.
const TRIG: &[(&str, &str)] = &[
    ("1.21.4", include_str!("../../tests/golden/1.21.4-trig.txt")),
    ("26.3", include_str!("../../tests/golden/26.3-trig.txt")),
];

#[test]
fn trig_tables_match_each_releases_mth() {
    for &(name, rows) in TRIG {
        let version = Version::from_name(name).unwrap();
        for line in rows.lines() {
            let f: Vec<f32> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [value, sin, cos] = f.as_slice() else {
                panic!("invalid trig row")
            };
            assert_eq!(ffi::trig(version, false, *value), *sin, "{name} sin {line}");
            assert_eq!(ffi::trig(version, true, *value), *cos, "{name} cos {line}");
        }
    }
}

/// Rows `seed noise x y z value`: the four vein noises as their density
/// functions read them.
const VEIN_NOISES: &[(&str, &str)] = &[
    (
        "1.21.4",
        include_str!("../../tests/golden/1.21.4-veins.txt"),
    ),
    ("26.3", include_str!("../../tests/golden/26.3-veins.txt")),
];

#[test]
fn vein_noises_match_the_original_noise_router() {
    for &(name, rows) in VEIN_NOISES {
        let version = Version::from_name(name).unwrap();
        // 26.3 sums its noises in single precision.
        let tolerance = if name == "26.3" { 1e-5 } else { 1e-12 };
        let mut count = 0;
        for line in rows.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [seed, noise, x, y, z, value] = f.as_slice() else {
                panic!("invalid vein noise row")
            };
            let at = [x, y, z].map(|v| v.parse::<i32>().unwrap());
            let got = ffi::vein_noise(version, seed.parse().unwrap(), noise.parse().unwrap(), at);
            let want: f64 = value.parse().unwrap();
            assert!((got - want).abs() <= tolerance, "{name} {line}: {got}");
            count += 1;
        }
        assert_eq!(count, 3 * 4 * 64, "{name}");
    }
}
