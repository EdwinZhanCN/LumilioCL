use super::colors::{self, Block, SEE_THROUGH, Table};

fn table(version: &str, data_version: i32) -> Table {
    Table::parse(&format!(
        r#"{{"version":"{version}","data_version":{data_version},
            "blocks":{{"minecraft:stone":[126,126,126,0],
                       "minecraft:grass_block":[147,147,147,2],
                       "minecraft:oak_leaves":[144,144,144,4],
                       "minecraft:water":[177,177,177,8],
                       "minecraft:short_grass":[146,145,146,3],
                       "minecraft:bad":[1,2]}},
            "biomes":{{"minecraft:plains":[9551193,7842607,4159204],
                       "minecraft:windswept_hills":[9020288,7182193,4159204]}}}}"#
    ))
    .unwrap()
}

#[test]
fn a_world_uses_the_newest_table_not_newer_than_it() {
    let tables = [table("1.21.4", 4189), table("26.3", 5023)];
    let pick = |data_version, version| colors::pick(&tables, data_version, version).version.clone();
    assert_eq!(pick(Some(4189), None), "1.21.4", "exactly the table's game");
    assert_eq!(pick(Some(4500), None), "1.21.4", "between two tables");
    assert_eq!(pick(Some(5023), None), "26.3");
    assert_eq!(pick(Some(9000), None), "26.3", "newer than every table");
    assert_eq!(pick(Some(2586), None), "1.21.4", "older than every table");
    // Without a data version the release name decides.
    assert_eq!(pick(None, Some("1.21.10")), "1.21.4");
    assert_eq!(pick(None, Some("26.1")), "1.21.4");
    assert_eq!(pick(None, Some("26.3")), "26.3");
    assert_eq!(pick(None, Some("1.16.5")), "1.21.4");
    assert_eq!(
        pick(None, Some("26.4-snapshot-3")),
        "26.3",
        "unreadable name"
    );
    assert_eq!(pick(None, None), "26.3");
}

#[test]
fn the_committed_tables_load_in_game_order() {
    let tables = colors::tables();
    assert_eq!(
        tables
            .iter()
            .map(|table| (table.version.as_str(), table.data_version))
            .collect::<Vec<_>>(),
        [("1.21.4", 4189), ("26.3", 5023)]
    );
    for table in tables {
        for name in [
            "minecraft:stone",
            "minecraft:grass_block",
            "minecraft:water",
        ] {
            assert!(table.block(name).is_some(), "{} {name}", table.version);
        }
        assert!(table.block("minecraft:air").is_none());
    }
}

#[test]
fn tinted_blocks_take_their_biome_colour_and_old_names_still_resolve() {
    let table = table("1.21.4", 4189);
    let grass = table.block("minecraft:grass_block").unwrap();
    // 147 × 0x91 / 255, 147 × 0xbd / 255, 147 × 0x59 / 255.
    assert_eq!(table.color(grass, Some("minecraft:plains")), [83, 108, 51]);
    assert_eq!(
        table.color(grass, Some("minecraft:mountains")),
        table.color(grass, Some("minecraft:windswept_hills")),
        "a pre-1.18 biome takes its renamed biome's colour"
    );
    assert_eq!(
        table.color(grass, Some("somemod:odd_biome")),
        table.color(grass, None),
        "an unknown biome is plains"
    );
    let water = table.block("minecraft:water").unwrap();
    assert_eq!(table.color(water, Some("minecraft:plains")), [43, 81, 158]);
    let stone = table.block("minecraft:stone").unwrap();
    assert_eq!(table.color(stone, Some("minecraft:plains")), [126; 3]);
    assert_eq!(
        table.block("minecraft:grass"),
        Some(Block {
            rgb: [146, 145, 146],
            flags: 2 | SEE_THROUGH
        }),
        "renamed to short_grass in 1.20.3"
    );
    assert_eq!(
        table.block("minecraft:bad"),
        None,
        "a malformed entry is left out"
    );
    assert_eq!(table.block("somemod:machine"), None);
}
