//! Structure start variants against the structure starts that original
//! servers saved (`crates/lumilio-xtask/src/worldgen/save_golden.py`, read by
//! `cargo xtask worldgen-save-read`; rows `seed dimension kind x z variant`).
use crate::{Bastion, Dimension, Structure, Variant, Version};

macro_rules! variant_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!("../../tests/golden/", $version, "-variants.txt")),
        )
    };
}
const VARIANTS: &[(&str, &str)] = &[
    variant_golden!("1.18.2"),
    variant_golden!("1.19.2"),
    variant_golden!("1.19.4"),
    variant_golden!("1.20.6"),
    variant_golden!("1.21.1"),
    variant_golden!("1.21.3"),
    variant_golden!("1.21.4"),
    variant_golden!("1.21.5"),
    variant_golden!("1.21.6"),
    variant_golden!("1.21.7"),
    variant_golden!("1.21.8"),
    variant_golden!("1.21.9"),
    variant_golden!("1.21.10"),
    variant_golden!("1.21.11"),
    variant_golden!("26.1"),
    variant_golden!("26.1.1"),
    variant_golden!("26.1.2"),
    variant_golden!("26.2"),
    variant_golden!("26.3"),
];

fn code(variant: Option<Variant>) -> Option<i64> {
    match variant? {
        Variant::Bastion(start) => Some(match start {
            Bastion::Housing => 0,
            Bastion::HoglinStable => 1,
            Bastion::Treasure => 2,
            Bastion::Bridge => 3,
        }),
        Variant::Igloo { basement } => Some(i64::from(basement)),
        Variant::EndCity { ship } => Some(i64::from(ship)),
    }
}

#[test]
fn start_variants_match_the_structure_starts_original_servers_saved() {
    for &(name, rows) in VARIANTS {
        let version = Version::from_name(name).unwrap();
        let mut kinds = std::collections::BTreeSet::new();
        for line in rows.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, dim, kind, x, z, expected] = fields.as_slice() else {
                panic!("invalid structure start row")
            };
            let dimension = match dim {
                -1 => Dimension::Nether,
                0 => Dimension::Overworld,
                1 => Dimension::End,
                _ => panic!("invalid dimension"),
            };
            let kind = Structure::ALL[*kind as usize];
            let at = [*x as i32, *z as i32];
            // The saved start is one the map shows: its own region query finds it.
            let found = crate::structures(
                version,
                *seed,
                dimension,
                kind,
                [at[0], at[1], at[0] + 16, at[1] + 16],
                || false,
            )
            .unwrap();
            assert!(found.contains(&at), "{name} {line}: {found:?}");
            assert_eq!(
                code(kind.variant(version, *seed, at)),
                Some(*expected),
                "{name} {line}"
            );
            kinds.insert(kind);
        }
        assert_eq!(
            kinds.len(),
            3,
            "{name}: bastions, igloos and End Cities are all sampled"
        );
    }
}

macro_rules! fossil_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!(
                "../../tests/golden/",
                $version,
                "-nether-fossils.txt"
            )),
        )
    };
}
/// Saved nether fossil pieces, rows `seed x z y`: the template position the
/// start chose, located from spread-out origins.
const NETHER_FOSSILS: &[(&str, &str)] = &[
    fossil_golden!("1.18.2"),
    fossil_golden!("1.19.2"),
    fossil_golden!("1.19.4"),
    fossil_golden!("1.20.6"),
    fossil_golden!("1.21.1"),
    fossil_golden!("1.21.3"),
    fossil_golden!("1.21.4"),
    fossil_golden!("1.21.5"),
    fossil_golden!("1.21.6"),
    fossil_golden!("1.21.7"),
    fossil_golden!("1.21.8"),
    fossil_golden!("1.21.9"),
    fossil_golden!("1.21.10"),
    fossil_golden!("1.21.11"),
    fossil_golden!("26.1"),
    fossil_golden!("26.1.1"),
    fossil_golden!("26.1.2"),
    fossil_golden!("26.2"),
    fossil_golden!("26.3"),
];

#[test]
fn nether_fossil_columns_match_the_fossils_original_servers_saved() {
    for &(name, rows) in NETHER_FOSSILS {
        let version = Version::from_name(name).unwrap();
        let mut count = 0;
        for line in rows.lines() {
            let fields: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, x, z, y] = fields.as_slice() else {
                panic!("invalid nether fossil row")
            };
            let at = [*x as i32, *z as i32];
            let found = crate::structures(
                version,
                *seed,
                Dimension::Nether,
                Structure::NetherFossil,
                [at[0] - 32, at[1] - 32, at[0] + 32, at[1] + 32],
                || false,
            )
            .unwrap();
            assert!(found.contains(&at), "{name} {line}: {found:?}");
            // The Nether's noise terrain gives the height the search stops at.
            assert_eq!(
                crate::ffi::nether_fossil_y(version, *seed, at),
                Some(*y as i32),
                "{name} {line}"
            );
            count += 1;
        }
        assert!(count >= 6, "{name}: {count} saved fossils");
    }
}

#[test]
fn only_bastions_igloos_and_end_cities_have_start_variants() {
    let version = Version::from_name("26.3").unwrap();
    for kind in Structure::ALL {
        let has = kind.variant(version, 262, [0, 0]).is_some();
        assert_eq!(
            has,
            matches!(
                kind,
                Structure::Bastion | Structure::Igloo | Structure::EndCity
            ),
            "{kind:?}"
        );
    }
    // Bastions began in 1.16; there is no start to tell apart before.
    let old = Version::from_name("1.15.2").unwrap();
    assert_eq!(Structure::Bastion.variant(old, 262, [0, 0]), None);
}
