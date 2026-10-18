//! Structure chests against the unopened chests original servers saved
//! (rows `seed chunk_x chunk_z x y z loot_seed`).
use crate::{Version, ffi};
use std::collections::BTreeMap;

macro_rules! chest_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!(
                "../../tests/golden/",
                $version,
                "-pyramid-chests.txt"
            )),
        )
    };
}
/// Releases with loot data (1.21+, data-driven enchantments).
const PYRAMID_CHESTS: &[(&str, &str)] = &[
    chest_golden!("1.21.1"),
    chest_golden!("1.21.3"),
    chest_golden!("1.21.4"),
    chest_golden!("1.21.5"),
    chest_golden!("1.21.6"),
    chest_golden!("1.21.7"),
    chest_golden!("1.21.8"),
    chest_golden!("1.21.9"),
    chest_golden!("1.21.10"),
    chest_golden!("1.21.11"),
    chest_golden!("26.1"),
    chest_golden!("26.1.1"),
    chest_golden!("26.1.2"),
    chest_golden!("26.2"),
    chest_golden!("26.3"),
];

/// Each loot release's goldens: (release, apples per filled chest, everything
/// the filled chests hold, the chests' loot seeds). The chests were filled by
/// the original server from their loot seeds (`save_golden.py --chests`).
macro_rules! filled_golden {
    ($version:literal) => {
        (
            $version,
            include_str!(concat!(
                "../../tests/golden/",
                $version,
                "-pyramid-loot.txt"
            )),
            include_str!(concat!(
                "../../tests/golden/",
                $version,
                "-pyramid-items.txt"
            )),
            include_str!(concat!(
                "../../tests/golden/",
                $version,
                "-pyramid-chests.txt"
            )),
        )
    };
}
const FILLED: &[(&str, &str, &str, &str)] = &[
    filled_golden!("1.21.1"),
    filled_golden!("1.21.3"),
    filled_golden!("1.21.4"),
    filled_golden!("1.21.5"),
    filled_golden!("1.21.6"),
    filled_golden!("1.21.7"),
    filled_golden!("1.21.8"),
    filled_golden!("1.21.9"),
    filled_golden!("1.21.10"),
    filled_golden!("1.21.11"),
    filled_golden!("26.1"),
    filled_golden!("26.1.1"),
    filled_golden!("26.1.2"),
    filled_golden!("26.2"),
    filled_golden!("26.3"),
];

#[test]
fn desert_pyramid_chests_hold_the_apples_original_servers_fill_them_with() {
    let mut apples = 0;
    for &(name, rows, ..) in FILLED {
        let version = Version::from_name(name).unwrap();
        let mut chests = 0;
        for line in rows.lines() {
            let f: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, x, _y, z, want] = f.as_slice() else {
                panic!("invalid loot row")
            };
            let (x, z) = (*x as i32, *z as i32);
            // A chest lies 8 to 12 blocks into its pyramid's start chunk.
            let start = [x.div_euclid(16) * 16, z.div_euclid(16) * 16];
            let chest = ffi::pyramid_chests(version, *seed, start)
                .into_iter()
                .find(|(at, ..)| *at == [x, z])
                .unwrap_or_else(|| panic!("{name} {line}: no predicted chest"));
            assert_eq!(i64::from(chest.2), *want, "{name} {line}");
            chests += 1;
            apples += want;
        }
        assert!(chests >= 32, "{name}: {chests} chests");
    }
    // Apples are rare (2 in 152 weight per roll); the sample must hold some.
    assert!(apples >= 10, "{apples} apples in every filled chest");
}

/// The other pools of the pyramid's table, whose items the simulated first
/// pool cannot account for (1.21–26.3).
const OTHER_POOLS: [&str; 6] = [
    "minecraft:bone",
    "minecraft:rotten_flesh",
    "minecraft:gunpowder",
    "minecraft:string",
    "minecraft:sand",
    "minecraft:dune_armor_trim_smithing_template",
];

#[test]
fn desert_pyramid_first_pools_match_what_original_servers_filled() {
    type Chest = (i64, i32, i32, i32);
    for &(name, _, items, chests) in FILLED {
        let version = Version::from_name(name).unwrap();
        let mut saved: BTreeMap<Chest, Vec<(String, i64)>> = BTreeMap::new();
        for line in items.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            let [seed, x, y, z, key, value] = f.as_slice() else {
                panic!("invalid item row")
            };
            let chest = (
                seed.parse().unwrap(),
                x.parse().unwrap(),
                y.parse().unwrap(),
                z.parse().unwrap(),
            );
            if !OTHER_POOLS.contains(key) {
                saved
                    .entry(chest)
                    .or_default()
                    .push(((*key).to_owned(), value.parse().unwrap()));
            }
        }
        let mut compared = 0;
        for line in chests.lines() {
            let f: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, _, _, x, y, z, loot] = f.as_slice() else {
                panic!("invalid chest row")
            };
            let chest = (*seed, *x as i32, *y as i32, *z as i32);
            let mut want = saved.remove(&chest).unwrap_or_default();
            let mut totals: BTreeMap<String, i64> = BTreeMap::new();
            let mut got = vec![];
            for item in ffi::pyramid_items(version, *loot).lines() {
                let f: Vec<&str> = item.split_whitespace().collect();
                match f.as_slice() {
                    [book, enchantment, level] => {
                        got.push((format!("{book}:{enchantment}"), level.parse().unwrap()));
                    }
                    [id, count] if !OTHER_POOLS.contains(id) => {
                        *totals.entry((*id).to_owned()).or_insert(0) +=
                            count.parse::<i64>().unwrap();
                    }
                    _ => {}
                }
            }
            got.extend(totals);
            got.sort();
            want.sort();
            assert_eq!(got, want, "{name} chest {chest:?} seed {loot}");
            compared += 1;
        }
        assert!(saved.is_empty(), "{name}: items of unrecorded chests");
        assert!(compared >= 32, "{name}: {compared} chests");
    }
}

/// A pyramid (seed, start chunk) and its chests' columns and loot seeds.
type Pyramids = BTreeMap<(i64, i32, i32), Vec<([i32; 2], i64)>>;

#[test]
fn desert_pyramid_chests_get_the_loot_seeds_original_servers_saved() {
    for &(name, rows) in PYRAMID_CHESTS {
        let version = Version::from_name(name).unwrap();
        let mut pyramids = Pyramids::new();
        for line in rows.lines() {
            let f: Vec<i64> = line
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            let [seed, cx, cz, x, _y, z, loot] = f.as_slice() else {
                panic!("invalid chest row")
            };
            pyramids
                .entry((*seed, *cx as i32, *cz as i32))
                .or_default()
                .push(([*x as i32, *z as i32], *loot));
        }
        assert!(pyramids.len() >= 8, "{name}: {} pyramids", pyramids.len());
        for ((seed, cx, cz), mut saved) in pyramids {
            saved.sort();
            let mut got: Vec<([i32; 2], i64)> =
                ffi::pyramid_chests(version, seed, [cx * 16, cz * 16])
                    .into_iter()
                    .map(|(at, loot, _)| (at, loot))
                    .collect();
            got.sort();
            assert_eq!(got, saved, "{name} {seed} {cx} {cz}");
        }
    }
}
