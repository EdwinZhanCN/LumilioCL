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
