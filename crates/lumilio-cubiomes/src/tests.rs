#[test]
fn native_generator_links_and_runs() {
    assert!(super::probe() >= 0);
}

#[test]
fn native_c_goldens_match_all_dimensions_and_scales() {
    use super::{Dimension, Range, Version, biomes};
    let mut rows = include_str!("../tests/biomes.txt").lines();
    for name in ["1.16.5", "1.18.2", "1.21.4"] {
        for dimension in [Dimension::Overworld, Dimension::Nether, Dimension::End] {
            for scale in [1, 4, 16, 64, 256] {
                let expected: Vec<i32> = rows
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .map(|n| n.parse().unwrap())
                    .collect();
                assert_eq!(
                    biomes(
                        Version::from_name(name).unwrap(),
                        262,
                        dimension,
                        Range {
                            scale,
                            x: -2,
                            z: -2,
                            width: 4,
                            height: 4
                        }
                    )
                    .unwrap(),
                    expected,
                    "{name} {dimension:?} {scale}"
                );
            }
        }
    }
    assert!(rows.next().is_none());
}

#[test]
fn rejects_unknown_versions_and_dangerous_ranges() {
    use super::*;
    for name in ["26.3", "1.21.5", "1.21", "snapshot", "1.18.1"] {
        assert_eq!(Version::from_name(name), Err(Error::Unsupported));
    }
    assert_eq!(Version::from_data_version(9999), Err(Error::Unsupported));
    let v = Version::from_name("1.21.4").unwrap();
    for range in [
        Range {
            scale: 2,
            x: 0,
            z: 0,
            width: 1,
            height: 1,
        },
        Range {
            scale: 1,
            x: i32::MAX,
            z: 0,
            width: 1,
            height: 1,
        },
        Range {
            scale: 1,
            x: 0,
            z: 0,
            width: 0,
            height: 1,
        },
        Range {
            scale: 256,
            x: i32::MIN,
            z: 0,
            width: 256,
            height: 256,
        },
    ] {
        assert_eq!(
            biomes(v, 0, Dimension::Overworld, range),
            Err(Error::InvalidRange)
        );
    }
}

#[test]
fn banded_generation_equals_row_by_row_and_stops_when_cancelled() {
    use super::{Dimension, Error, Range, Version, biomes, biomes_until};
    for name in ["1.16.5", "1.21.4"] {
        let version = Version::from_name(name).unwrap();
        let whole = Range {
            scale: 4,
            x: -20,
            z: -37,
            width: 24,
            height: 40,
        };
        let banded = biomes(version, 262, Dimension::Overworld, whole).unwrap();
        let rows: Vec<i32> = (0..whole.height)
            .flat_map(|row| {
                biomes(
                    version,
                    262,
                    Dimension::Overworld,
                    Range {
                        z: whole.z + row,
                        height: 1,
                        ..whole
                    },
                )
                .unwrap()
            })
            .collect();
        assert_eq!(banded, rows, "{name}");
    }
    let mut polls = 0;
    let stopped = biomes_until(
        Version::from_name("1.21.4").unwrap(),
        262,
        Dimension::Overworld,
        Range {
            scale: 4,
            x: 0,
            z: 0,
            width: 64,
            height: 64,
        },
        || {
            polls += 1;
            polls > 1
        },
    );
    assert_eq!(stopped, Err(Error::Cancelled));
    assert_eq!(polls, 2, "polled once per band, stopped at the first yes");
}

const GOLDEN_VERSIONS: [&str; 3] = ["1.16.5", "1.18.2", "1.21.4"];
const GOLDEN_SEEDS: [i64; 2] = [262, 9_876_543_210];
const GOLDEN_DIMENSIONS: [super::Dimension; 3] = [
    super::Dimension::Overworld,
    super::Dimension::Nether,
    super::Dimension::End,
];

fn positions(fields: std::str::SplitWhitespace<'_>) -> Vec<super::Position> {
    fields
        .map(|pair| {
            let (x, z) = pair.split_once(',').unwrap();
            [x.parse().unwrap(), z.parse().unwrap()]
        })
        .collect()
}

#[test]
fn structure_strongholds_spawn_and_slime_goldens_match_the_c_probe() {
    use super::{Structure, Version, slime_chunks, spawn, strongholds, structures};
    let mut listed = std::collections::BTreeSet::new();
    let mut checked = [0; 4];
    for line in include_str!("../tests/structures.txt").lines() {
        let mut fields = line.split_whitespace();
        let tag = fields.next().unwrap();
        let mut index = || fields.next().unwrap().parse::<usize>().unwrap();
        match tag {
            "S" => {
                let (s, v, d, k) = (index(), index(), index(), index());
                let (version, seed) = (
                    Version::from_name(GOLDEN_VERSIONS[v]).unwrap(),
                    GOLDEN_SEEDS[s],
                );
                let (dimension, kind) = (GOLDEN_DIMENSIONS[d], Structure::ALL[k]);
                listed.insert((s, v, d, k));
                assert_eq!(
                    structures(
                        version,
                        seed,
                        dimension,
                        kind,
                        [-1536, -1536, 1536, 1536],
                        || false
                    ),
                    Ok(positions(fields)),
                    "{} {seed} {dimension:?} {kind:?}",
                    GOLDEN_VERSIONS[v]
                );
                checked[0] += 1;
            }
            "H" => {
                let (s, v) = (index(), index());
                let version = Version::from_name(GOLDEN_VERSIONS[v]).unwrap();
                let expected = positions(fields);
                assert_eq!(
                    strongholds(version, GOLDEN_SEEDS[s], expected.len(), || false),
                    Ok(expected)
                );
                checked[1] += 1;
            }
            "P" => {
                let (s, v) = (index(), index());
                let version = Version::from_name(GOLDEN_VERSIONS[v]).unwrap();
                assert_eq!(
                    [spawn(version, GOLDEN_SEEDS[s]).at],
                    positions(fields).as_slice()
                );
                checked[2] += 1;
            }
            "L" => {
                let (s, _v) = (index(), index());
                let expected: Vec<bool> =
                    fields.next().unwrap().chars().map(|c| c == '1').collect();
                assert_eq!(slime_chunks(GOLDEN_SEEDS[s], -4, -4, 8, 8), Ok(expected));
                checked[3] += 1;
            }
            other => panic!("unknown golden row {other}"),
        }
    }
    assert_eq!(checked[1..], [6, 6, 6]);
    // A kind absent from the probe output is one cubiomes refuses, and the
    // Rust side must agree about every pair, not only the listed ones.
    for s in 0..2 {
        for (v, name) in GOLDEN_VERSIONS.iter().enumerate() {
            for (d, dimension) in GOLDEN_DIMENSIONS.iter().enumerate() {
                for (k, kind) in Structure::ALL.iter().enumerate() {
                    assert_eq!(
                        kind.available(Version::from_name(name).unwrap(), *dimension),
                        listed.contains(&(s, v, d, k)),
                        "{name} {dimension:?} {kind:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn structure_queries_reject_bad_areas_unavailable_kinds_and_stop_when_cancelled() {
    use super::{Dimension, Error, MAX_AREA, Structure, Version, slime_chunks, structures};
    let v = Version::from_name("1.21.4").unwrap();
    let ok = [0, 0, 512, 512];
    assert_eq!(
        structures(v, 1, Dimension::Nether, Structure::Village, ok, || false),
        Err(Error::Unsupported)
    );
    for area in [
        [0, 0, 0, 10],
        [10, 0, 0, 10],
        [0, 0, MAX_AREA + 1, 10],
        [0, 0, 10, MAX_AREA + 1],
        [-40_000_000, 0, -39_999_000, 10],
        [0, 0, 40_000_000, 10],
    ] {
        assert_eq!(
            structures(v, 1, Dimension::Overworld, Structure::Village, area, || {
                false
            }),
            Err(Error::InvalidRange),
            "{area:?}"
        );
    }
    assert_eq!(
        structures(
            v,
            1,
            Dimension::Overworld,
            Structure::Village,
            [0, 0, 8192, 8192],
            || true
        ),
        Err(Error::Cancelled)
    );
    // The widest accepted area fits its buffer for the densest kind.
    structures(
        v,
        262,
        Dimension::Overworld,
        Structure::Village,
        [-MAX_AREA / 2, -MAX_AREA / 2, MAX_AREA / 2, MAX_AREA / 2],
        || false,
    )
    .unwrap();
    assert_eq!(slime_chunks(1, 0, 0, 0, 4), Err(Error::InvalidRange));
    assert_eq!(slime_chunks(1, 0, 0, 1025, 4), Err(Error::InvalidRange));
}

#[test]
fn estimated_flags_follow_the_height_dependent_kinds_since_1_18() {
    use super::{Structure, Version, spawn};
    let old = Version::from_name("1.16.5").unwrap();
    let new = Version::from_name("1.18.2").unwrap();
    for kind in Structure::ALL {
        let height_bound = matches!(
            kind,
            Structure::DesertPyramid | Structure::JungleTemple | Structure::Mansion
        );
        assert!(!kind.estimated(old), "{kind:?}");
        assert_eq!(kind.estimated(new), height_bound, "{kind:?}");
    }
    assert!(spawn(old, 262).estimated);
    assert!(!spawn(new, 262).estimated);
}
