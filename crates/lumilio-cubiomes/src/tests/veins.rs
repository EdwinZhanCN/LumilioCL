//! Ore veins against original servers: every raw ore block a saved world
//! holds in its fully generated areas (rows `seed x y z kind`, 1 copper,
//! 2 iron; only veins place these blocks) must be the vein rule's raw block.
use crate::{Version, ffi};

macro_rules! ore_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!("../../tests/golden/", $version, "-ores.txt")),
        )
    };
}
const ORES: &[(&str, &str)] = &[
    ore_golden!("1.18.2"),
    ore_golden!("1.19.2"),
    ore_golden!("1.19.4"),
    ore_golden!("1.20.6"),
    ore_golden!("1.21.1"),
    ore_golden!("1.21.3"),
    ore_golden!("1.21.4"),
    ore_golden!("1.21.5"),
    ore_golden!("1.21.6"),
    ore_golden!("1.21.7"),
    ore_golden!("1.21.8"),
    ore_golden!("1.21.9"),
    ore_golden!("1.21.10"),
    ore_golden!("1.21.11"),
    ore_golden!("26.1"),
    ore_golden!("26.1.1"),
    ore_golden!("26.1.2"),
    ore_golden!("26.2"),
    ore_golden!("26.3"),
];

/// The fork's `VB_RAW_COPPER` and `VB_RAW_IRON`.
const RAW: [u8; 2] = [3, 6];

#[test]
fn saved_raw_ore_blocks_are_where_the_vein_rule_puts_them() {
    for &(name, rows) in ORES {
        let version = Version::from_name(name).unwrap();
        let mut count = 0;
        for line in rows.lines() {
            let f: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, x, y, z, kind] = f.as_slice() else {
                panic!("invalid ore row")
            };
            let (x, y, z) = (*x as i32, *y as i32, *z as i32);
            let got = ffi::vein_column(version, *seed, [x, z], y, y)[0];
            assert_eq!(got, RAW[*kind as usize - 1], "{name} {line}");
            count += 1;
        }
        assert!(count >= 10, "{name}: {count} raw ore blocks");
    }
}
