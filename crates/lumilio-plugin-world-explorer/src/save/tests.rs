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

mod drawing {
    use super::super::{BASE, render, sources, tile};
    use super::*;
    use lumilio_plugin_api::map::{
        Dimension, TileKey, TileReply, TileRequest, WorldContext, WorldId,
    };
    use lumilio_plugin_api::{
        DirEntry, DirPage, FetchResponse, FileStat, HostContext, PluginError, SettingValue,
    };
    use std::collections::BTreeMap;

    /// A game directory in memory, read the way the host lets the plugin.
    struct Files(BTreeMap<String, Vec<u8>>);

    impl HostContext for Files {
        fn setting(&self, _: &str) -> Option<SettingValue> {
            None
        }
        fn read_file(&self, _: &str) -> Result<Vec<u8>, PluginError> {
            panic!("regions are read by range")
        }
        fn list_files(&self, _: &str) -> Result<Vec<String>, PluginError> {
            panic!("regions are listed a page at a time")
        }
        fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
            panic!("the save map never uses the network")
        }
        fn file_stat(&self, path: &str) -> Result<Option<FileStat>, PluginError> {
            Ok(self.0.get(path).map(|bytes| FileStat {
                len: bytes.len() as u64,
                modified_ms: 1,
            }))
        }
        fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>, PluginError> {
            let bytes = self
                .0
                .get(path)
                .ok_or_else(|| PluginError::Unavailable("missing".into()))?;
            let start = (offset as usize).min(bytes.len());
            Ok(bytes[start..(start + len).min(bytes.len())].to_vec())
        }
        fn list_dir(
            &self,
            dir: &str,
            after: Option<&str>,
            limit: usize,
        ) -> Result<DirPage, PluginError> {
            let names: Vec<String> = self
                .0
                .keys()
                .filter_map(|path| path.strip_prefix(&format!("{dir}/")))
                .filter(|name| !name.contains('/') && after.is_none_or(|after| *name > after))
                .map(str::to_owned)
                .collect();
            let next = (names.len() > limit).then(|| names[limit - 1].clone());
            Ok(DirPage {
                entries: names
                    .into_iter()
                    .take(limit)
                    .map(|name| DirEntry {
                        name,
                        is_dir: false,
                    })
                    .collect(),
                next,
            })
        }
    }

    fn sample(version: &str, name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../lumilio-anvil/tests/data")
                .join(version)
                .join(name),
        )
        .unwrap()
    }

    fn request(
        data_version: i32,
        dimension: Dimension,
        level: u8,
        tx: i32,
        tz: i32,
    ) -> TileRequest {
        let world = WorldId::Save {
            instance: "i".into(),
            folder: "World".into(),
        };
        TileRequest {
            context: WorldContext {
                world: world.clone(),
                version: None,
                data_version: Some(data_version),
                seed: None,
                dimension: dimension.clone(),
                sources: vec![],
            },
            key: TileKey {
                provider: crate::ID.into(),
                base_map: BASE.into(),
                world,
                dimension,
                level,
                tx,
                tz,
            },
            pixels: 256,
        }
    }

    /// Which 16×16 chunk cells of a tile are covered.
    fn covered_chunks(reply: &TileReply) -> Vec<(usize, usize)> {
        let TileReply::Partial { coverage, .. } = reply else {
            panic!("expected a partial tile, got {reply:?}");
        };
        let mut chunks = Vec::new();
        for cz in 0..16 {
            for cx in 0..16 {
                let cells: Vec<u8> = (0..256)
                    .map(|at| coverage[(cz * 16 + at / 16) * 256 + cx * 16 + at % 16])
                    .collect();
                assert!(
                    cells.iter().all(|cell| *cell == cells[0]),
                    "a chunk is covered whole or not at all"
                );
                if cells[0] == 255 {
                    chunks.push((cx, cz));
                }
            }
        }
        chunks
    }

    #[test]
    fn a_26_3_world_draws_its_finished_chunks_and_leaves_the_rest_uncovered() {
        let files = Files(BTreeMap::from([(
            "saves/World/dimensions/minecraft/overworld/region/r.0.0.mca".to_owned(),
            sample("26.3", "r.0.0.mca"),
        )]));
        let reply = tile(&files, &request(5023, Dimension::Overworld, 0, 0, 0)).unwrap();
        let chunks = covered_chunks(&reply);
        // The sample keeps chunks x 5–8, z 0–3: x 5 is `full`, x 6 has its
        // features placed (`initialize_light`), x 7 stopped at `terrain` and
        // x 8 at `biomes`.
        let expected: Vec<(usize, usize)> =
            (0..4).flat_map(|z| (5..7).map(move |x| (x, z))).collect();
        assert_eq!(chunks, expected);
        let TileReply::Partial { image, unknown, .. } = &reply else {
            unreachable!()
        };
        assert!(
            unknown.is_empty(),
            "every 26.3 block is in the 26.3 table: {unknown:?}"
        );
        // Chunk (5, 0) column 0 is grass at 88 in a forest: a green, not grey.
        let [r, g, b] = [0, 1, 2].map(|at| image.rgba[(5 * 16) * 4 + at]);
        assert!(g > r && g > b, "{r} {g} {b}");
        // Nothing for the other quarters of the region, or another region.
        for (tx, tz) in [(1, 0), (0, 1), (2, 0), (-1, 0)] {
            assert_eq!(
                tile(&files, &request(5023, Dimension::Overworld, 0, tx, tz)).unwrap(),
                TileReply::Empty,
                "{tx},{tz}"
            );
        }
    }

    #[test]
    fn an_old_world_is_found_in_the_old_layout_and_drawn_with_the_oldest_table() {
        let files = Files(BTreeMap::from([(
            "saves/World/region/r.0.-1.mca".to_owned(),
            sample("1.16.5", "r.0.-1.mca"),
        )]));
        // Chunks x 8–11, z 16–19 of region (0, -1): tile (0, -1), chunk cells x 8–11, z 0–3.
        let reply = tile(&files, &request(2586, Dimension::Overworld, 0, 0, -1)).unwrap();
        let chunks = covered_chunks(&reply);
        assert_eq!(chunks.len(), 16);
        assert!(chunks.iter().all(|(x, z)| (8..12).contains(x) && *z < 4));
        let TileReply::Partial { unknown, .. } = &reply else {
            unreachable!()
        };
        assert!(
            unknown.is_empty(),
            "1.16.5's `grass` resolves to the 1.21.4 table's `short_grass`: {unknown:?}"
        );
    }

    #[test]
    fn blocks_no_table_knows_are_drawn_violet_and_named() {
        let files = Files(BTreeMap::from([(
            "saves/World/region/r.0.0.mca".to_owned(),
            sample("1.18.2", "r.0.0.mca"),
        )]));
        // A table that knows only stone: everything else on the surface is unknown.
        let only_stone = Table::parse(
            r#"{"version":"1.0","data_version":1,"blocks":{"minecraft:stone":[1,2,3,0]}}"#,
        )
        .unwrap();
        let region = lumilio_anvil::Region::open(
            super::super::HostRegion::new(&files, "saves/World/region/r.0.0.mca".into()),
            0,
            0,
        )
        .unwrap();
        let reply =
            render::draw(&region, (0, 0), &only_stone, render::View::Open, &|| false).unwrap();
        let TileReply::Partial { image, unknown, .. } = &reply else {
            panic!("{reply:?}")
        };
        assert!(
            unknown.contains(&"minecraft:grass_block".to_owned()),
            "{unknown:?}"
        );
        assert!(
            unknown.contains(&"minecraft:oak_leaves".to_owned()),
            "{unknown:?}"
        );
        assert!(
            unknown.windows(2).all(|pair| pair[0] < pair[1]),
            "sorted, no repeats"
        );
        // Pixel (0, 0) is in the shade-free top row: the unknown colour × 220/255.
        let expected = colors::UNKNOWN.map(|channel| (u16::from(channel) * 220 / 255) as u8);
        assert_eq!(image.rgba[..3], expected);
    }

    #[test]
    fn a_broken_chunk_blanks_only_itself() {
        let mut bytes = sample("1.18.2", "r.0.0.mca");
        // Chunk (2, 1): its compression byte becomes one the game never writes.
        let entry = (32 + 2) * 4;
        let sector = u32::from_be_bytes(bytes[entry..entry + 4].try_into().unwrap()) >> 8;
        bytes[sector as usize * 4096 + 4] = 9;
        let files = Files(BTreeMap::from([(
            "saves/World/region/r.0.0.mca".to_owned(),
            bytes,
        )]));
        let reply = tile(&files, &request(2975, Dimension::Overworld, 0, 0, 0)).unwrap();
        let chunks = covered_chunks(&reply);
        assert_eq!(chunks.len(), 15);
        assert!(!chunks.contains(&(2, 1)));
    }

    #[test]
    fn sources_name_the_region_files_a_tile_is_drawn_from() {
        let files = Files(BTreeMap::from([
            ("saves/World/region/r.0.0.mca".to_owned(), vec![]),
            ("saves/World/region/r.1.0.mca".to_owned(), vec![]),
            ("saves/World/region/r.-1.0.mca".to_owned(), vec![]),
            ("saves/World/region/r.5.5.mca".to_owned(), vec![]),
            ("saves/World/region/c.0.0.mcc".to_owned(), vec![]),
            ("saves/World/DIM-1/region/r.0.0.mca".to_owned(), vec![]),
        ]));
        let named = |level, tx, tz| {
            sources(&files, &request(1, Dimension::Overworld, level, tx, tz))
                .unwrap()
                .unwrap()
        };
        // Level 0 names its region in both layouts, whether or not it exists.
        assert_eq!(
            named(0, 1, 1),
            [
                "saves/World/dimensions/minecraft/overworld/region/r.0.0.mca",
                "saves/World/region/r.0.0.mca"
            ]
        );
        assert_eq!(named(0, -1, 0)[1], "saves/World/region/r.-1.0.mca");
        // Level 1 (1024 blocks) covers regions 0–1 × 0–1 and names those that exist.
        assert_eq!(
            named(1, 0, 0),
            [
                "saves/World/region/r.0.0.mca",
                "saves/World/region/r.1.0.mca"
            ]
        );
        assert!(named(1, 5, 5).is_empty());
        assert_eq!(named(2, 0, 0).len(), 3, "regions 0–7 × 0–7");
        assert!(named(4, -1, -1).is_empty(), "regions -128–-1 × -128–-1");
        assert_eq!(named(4, -1, 0), ["saves/World/region/r.-1.0.mca"]);
        // Only a single-player save has region files.
        let mut seed = request(1, Dimension::Overworld, 0, 0, 0);
        seed.context.world = WorldId::Seed {
            seed: 1,
            version: "1.21.4".into(),
        };
        seed.key.world = seed.context.world.clone();
        assert_eq!(sources(&files, &seed).unwrap(), Some(vec![]));
        assert_eq!(tile(&files, &seed).unwrap(), TileReply::Empty);
        assert!(tile(&files, &request(1, Dimension::Overworld, 1, 0, 0)).is_err());
    }
}
