use crate::{Dimension, Error, Range, Structure, Version, biomes, ffi};

const RELEASES: &[(&str, i32, &str)] = &[
    (
        "1.21.4",
        4189,
        include_str!("../../tests/golden/1.21.4.txt"),
    ),
    (
        "1.21.5",
        4325,
        include_str!("../../tests/golden/1.21.5.txt"),
    ),
    (
        "1.21.6",
        4435,
        include_str!("../../tests/golden/1.21.6.txt"),
    ),
    (
        "1.21.7",
        4438,
        include_str!("../../tests/golden/1.21.7.txt"),
    ),
    (
        "1.21.8",
        4440,
        include_str!("../../tests/golden/1.21.8.txt"),
    ),
    (
        "1.21.9",
        4554,
        include_str!("../../tests/golden/1.21.9.txt"),
    ),
    (
        "1.21.10",
        4556,
        include_str!("../../tests/golden/1.21.10.txt"),
    ),
    (
        "1.21.11",
        4671,
        include_str!("../../tests/golden/1.21.11.txt"),
    ),
    ("26.1", 4786, include_str!("../../tests/golden/26.1.txt")),
    (
        "26.1.1",
        4788,
        include_str!("../../tests/golden/26.1.1.txt"),
    ),
    (
        "26.1.2",
        4790,
        include_str!("../../tests/golden/26.1.2.txt"),
    ),
    ("26.2", 4903, include_str!("../../tests/golden/26.2.txt")),
    ("26.3", 5023, include_str!("../../tests/golden/26.3.txt")),
];

macro_rules! structure_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!("../../tests/golden/", $version, "-structures.txt")),
            include_str!(concat!("../../tests/golden/", $version, "-rules.txt")),
        )
    };
}
const STRUCTURES: &[(&str, &str, &str)] = &[
    structure_golden!("1.21.5"),
    structure_golden!("1.21.6"),
    structure_golden!("1.21.7"),
    structure_golden!("1.21.8"),
    structure_golden!("1.21.9"),
    structure_golden!("1.21.10"),
    structure_golden!("1.21.11"),
    structure_golden!("26.1"),
    structure_golden!("26.1.1"),
    structure_golden!("26.1.2"),
    structure_golden!("26.2"),
    structure_golden!("26.3"),
];

macro_rules! saved_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!("../../tests/golden/", $version, "-saved.txt")),
            include_str!(concat!("../../tests/golden/", $version, "-locates.txt")),
        )
    };
}
const SAVED: &[(&str, &str, &str)] = &[
    saved_golden!("1.21.4"),
    saved_golden!("1.21.5"),
    saved_golden!("1.21.6"),
    saved_golden!("1.21.7"),
    saved_golden!("1.21.8"),
    saved_golden!("1.21.9"),
    saved_golden!("1.21.10"),
    saved_golden!("1.21.11"),
    saved_golden!("26.1"),
    saved_golden!("26.1.1"),
    saved_golden!("26.1.2"),
    saved_golden!("26.2"),
    saved_golden!("26.3"),
];

#[test]
fn actual_saved_biomes_and_locates_match_with_bounded_spawn_estimates() {
    for &(name, samples, locates) in SAVED {
        let version = Version::from_name(name).unwrap();
        let mut covered = std::collections::BTreeSet::new();
        for line in samples.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, dim, x, y, z, expected] = fields.as_slice() else {
                panic!("invalid saved sample")
            };
            let dimension = match dim {
                -1 => Dimension::Nether,
                0 => Dimension::Overworld,
                1 => Dimension::End,
                _ => panic!("invalid dimension"),
            };
            covered.insert((*seed, *dim));
            assert_eq!(
                ffi::sample(version, *seed, dimension, [*x as i32, *y as i32, *z as i32]),
                *expected as i32,
                "saved {name} {line}"
            );
        }
        assert_eq!(covered.len(), 9, "saved {name}");
        let mut count = 0;
        for line in locates.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, dim, kind, x, z] = fields.as_slice() else {
                panic!("invalid locate")
            };
            let target = [*x as i32, *z as i32];
            if *kind == -2 {
                let spawn = crate::spawn(version, *seed);
                assert!(spawn.estimated, "spawn {name} {line}");
                // Both spawn searches examine chunks around the climate-selected
                // origin; terrain approximations can pick different columns.
                assert!(
                    spawn
                        .at
                        .into_iter()
                        .zip(target)
                        .all(|(a, b)| a.abs_diff(b) <= 176),
                    "spawn {name} {line}: {:?}",
                    spawn.at
                );
            } else if *kind == -1 {
                let candidates = crate::strongholds(version, *seed, 3, || false).unwrap();
                assert!(
                    candidates
                        .iter()
                        .any(|p| p.map(|v| v.div_euclid(16) * 16) == target),
                    "locate {name} {line}: {candidates:?}"
                );
            } else if *kind == 17 {
                // Camp terrain/template viability is explicitly estimated;
                // locate verifies its region placement, not that approximation.
                assert_eq!(
                    ffi::placement(
                        version,
                        *seed,
                        Structure::AbandonedCamp,
                        target.map(|v| v.div_euclid(37 * 16))
                    ),
                    Ok(target),
                    "camp {name} {line}"
                );
            } else {
                let dimension = match dim {
                    -1 => Dimension::Nether,
                    0 => Dimension::Overworld,
                    1 => Dimension::End,
                    _ => panic!("invalid dimension"),
                };
                let candidates = crate::structures(
                    version,
                    *seed,
                    dimension,
                    Structure::ALL[*kind as usize],
                    [target[0], target[1], target[0] + 16, target[1] + 16],
                    || false,
                )
                .unwrap();
                assert!(
                    candidates.contains(&target),
                    "locate {name} {line}: {candidates:?}"
                );
            }
            count += 1;
        }
        assert_eq!(count, if name == "26.3" { 18 } else { 15 });
    }
}

#[test]
fn regional_placements_and_changed_biome_tags_match_each_original_release() {
    for &(name, placements, rules) in STRUCTURES {
        let version = Version::from_name(name).unwrap();
        let mut count = 0;
        for line in placements.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, kind, rx, rz, x, z] = fields.as_slice() else {
                panic!("invalid placement")
            };
            let kind = Structure::ALL[*kind as usize];
            assert_eq!(
                ffi::placement(version, *seed, kind, [*rx as i32, *rz as i32]),
                Ok([*x as i32, *z as i32]),
                "{name} {kind:?} {line}"
            );
            count += 1;
        }
        assert_eq!(count, if name == "26.3" { 1350 } else { 1275 });
        for line in rules.lines() {
            let fields: Vec<i32> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [rule, id, expected] = fields.as_slice() else {
                panic!("invalid biome rule")
            };
            assert_eq!(
                ffi::biome_rule(version, *rule, *id),
                *expected != 0,
                "{name} {line}"
            );
        }
    }
}

#[test]
fn every_release_matches_its_own_original_server_samples() {
    let mut newest = std::collections::BTreeSet::new();
    for &(name, data_version, samples) in RELEASES {
        let version = Version::from_name(name).unwrap();
        assert_eq!(Version::from_data_version(data_version), Ok(version));
        let mut covered = std::collections::BTreeSet::new();
        let mut count = 0;
        for line in samples.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            assert_eq!(fields.len(), 6);
            let [seed, dim, x, y, z, expected] = fields.as_slice() else {
                unreachable!()
            };
            let dimension = match dim {
                -1 => Dimension::Nether,
                0 => Dimension::Overworld,
                1 => Dimension::End,
                _ => panic!("bad dimension"),
            };
            covered.insert((*seed, *dim, *y));
            assert_eq!(
                ffi::sample(version, *seed, dimension, [*x as i32, *y as i32, *z as i32]),
                *expected as i32,
                "{name} {line}"
            );
            if *y == 16 {
                // The public map path uses genBiomes and its cached tree leaf,
                // rather than the oracle probe's single getBiomeAt call.
                assert_eq!(
                    biomes(
                        version,
                        *seed,
                        dimension,
                        Range {
                            scale: 4,
                            x: *x as i32,
                            z: *z as i32,
                            width: 1,
                            height: 1,
                        }
                    ),
                    Ok(vec![*expected as i32]),
                    "map {name} {line}"
                );
            }
            if name == "26.3" {
                newest.insert(*expected);
            }
            count += 1;
        }
        assert_eq!(covered.len(), 3 * 3 * 4, "{name}");
        assert_eq!(count, 3456, "{name}");
    }
    for id in [186, 187, 188] {
        assert!(newest.contains(&id), "new biome {id} must be sampled");
    }
    for unknown in [4326, 4904, 5022, 5024] {
        assert_eq!(Version::from_data_version(unknown), Err(Error::Unsupported));
    }
}

#[test]
fn camps_are_available_only_in_the_26_3_overworld_and_always_estimated() {
    let new = Version::from_name("26.3").unwrap();
    let old = Version::from_name("26.2").unwrap();
    assert!(!Structure::AbandonedCamp.available(old, Dimension::Overworld));
    assert!(Structure::AbandonedCamp.available(new, Dimension::Overworld));
    assert!(!Structure::AbandonedCamp.available(new, Dimension::Nether));
    assert!(!Structure::AbandonedCamp.available(new, Dimension::End));
    assert!(Structure::AbandonedCamp.estimated(new));
}
