use std::collections::BTreeMap;
use std::io::Write;

use lumilio_nbt::Tag;
use lumilio_plugin_api::{
    ActionId, Effect, FetchResponse, GameFacts, HostContext, InstanceTab, ModFact, PluginError,
    SettingValue, TabState, View,
};

use crate::format::{self, pack};
use crate::{FOLDER, Litematica};

struct Files(BTreeMap<String, Vec<u8>>);

impl HostContext for Files {
    fn setting(&self, _: &str) -> Option<SettingValue> {
        None
    }
    fn read_file(&self, path: &str) -> Result<Vec<u8>, PluginError> {
        self.0
            .get(path)
            .cloned()
            .ok_or(PluginError::Unavailable("missing".into()))
    }
    fn list_files(&self, dir: &str) -> Result<Vec<String>, PluginError> {
        Ok(self
            .0
            .keys()
            .filter(|path| path.starts_with(&format!("{dir}/")))
            .cloned()
            .collect())
    }
    fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
        Err(PluginError::PermissionDenied)
    }
}

fn compound(entries: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}

fn vec3(x: i32, y: i32, z: i32) -> Tag {
    compound(vec![
        ("x", Tag::Int(x)),
        ("y", Tag::Int(y)),
        ("z", Tag::Int(z)),
    ])
}

fn palette(names: &[&str]) -> Tag {
    Tag::List(
        names
            .iter()
            .map(|name| compound(vec![("Name", Tag::String((*name).into()))]))
            .collect(),
    )
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

fn litematic(name: &str, size: (i32, i32, i32), names: &[&str], values: &[u64]) -> Vec<u8> {
    let bits = if names.len() <= 4 {
        2
    } else {
        usize::BITS - (names.len() - 1).leading_zeros()
    };
    let region = compound(vec![
        ("Size", vec3(size.0, size.1, size.2)),
        ("BlockStatePalette", palette(names)),
        ("BlockStates", Tag::LongArray(pack(values, bits))),
    ]);
    let root = compound(vec![
        (
            "Metadata",
            compound(vec![
                ("Name", Tag::String(name.into())),
                ("Author", Tag::String("Alex".into())),
                ("EnclosingSize", vec3(size.0, size.1, size.2)),
                ("TotalBlocks", Tag::Int(values.len() as i32)),
                ("RegionCount", Tag::Int(1)),
                ("TimeModified", Tag::Long(1_700_000_000_000)),
            ]),
        ),
        ("Regions", compound(vec![("main", region)])),
    ]);
    gzip(&lumilio_nbt::to_bytes(&root))
}

fn files(entries: Vec<(&str, Vec<u8>)>) -> Files {
    Files(
        entries
            .into_iter()
            .map(|(name, bytes)| (format!("{FOLDER}/{name}"), bytes))
            .collect(),
    )
}

#[test]
fn materials_count_every_block_and_skip_air_sorted_by_the_view() {
    // 2x2x2 = 8 blocks: 3 stone, 4 air, 1 oak log.
    let bytes = litematic(
        "House",
        (2, 2, 2),
        &["minecraft:air", "minecraft:stone", "minecraft:oak_log"],
        &[1, 1, 0, 0, 2, 0, 1, 0],
    );
    let counts = format::parse(&bytes).unwrap().materials().unwrap();
    assert_eq!(counts["minecraft:stone"], 3);
    assert_eq!(counts["minecraft:oak_log"], 1);
    assert!(!counts.contains_key("minecraft:air"));
}

#[test]
fn indices_that_straddle_two_longs_decode() {
    // 5 palette bits (17 entries) do not divide 64, so values straddle longs.
    let names: Vec<String> = (0..17).map(|i| format!("minecraft:b{i}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let values: Vec<u64> = (0..26).map(|i| i % 17).collect();
    let bytes = litematic("Wide", (26, 1, 1), &names, &values);
    let counts = format::parse(&bytes).unwrap().materials().unwrap();
    let total: u64 = counts.values().sum();
    // b0 is not air here, so every one of the 26 blocks is counted.
    assert_eq!(total, 26);
    assert_eq!(counts["minecraft:b16"], 1);
    assert_eq!(counts["minecraft:b0"], 2);
}

#[test]
fn a_palette_index_outside_the_palette_is_an_error_not_a_panic() {
    let bytes = litematic(
        "Bad",
        (1, 1, 2),
        &["minecraft:air", "minecraft:stone"],
        &[3, 1],
    );
    assert!(format::parse(&bytes).unwrap().materials().is_err());
}

#[test]
fn short_block_state_arrays_are_rejected() {
    let region = compound(vec![
        ("Size", vec3(100, 100, 100)),
        ("BlockStatePalette", palette(&["minecraft:stone"])),
        ("BlockStates", Tag::LongArray(vec![0])),
    ]);
    let root = compound(vec![
        ("Metadata", compound(vec![])),
        ("Regions", compound(vec![("r", region)])),
    ]);
    let parsed = format::parse(&lumilio_nbt::to_bytes(&root)).unwrap();
    assert!(parsed.materials().is_err());
}

#[test]
fn the_list_shows_metadata_and_marks_broken_files_without_failing() {
    let ctx = files(vec![
        (
            "house.litematic",
            litematic("House", (2, 1, 1), &["minecraft:stone"], &[0, 0]),
        ),
        ("broken.litematic", b"not nbt".to_vec()),
        ("notes.txt", b"ignored".to_vec()),
    ]);
    let View::List { items } = Litematica.view(&ctx, &TabState::Null).unwrap() else {
        panic!("list");
    };
    assert_eq!(items.len(), 2);
    let broken = &items[0];
    assert_eq!(broken.title, "broken");
    assert_eq!(broken.subtitle.as_deref(), Some("读不了"));
    assert!(broken.open.is_none());
    let house = &items[1];
    assert_eq!(
        house.title, "house",
        "the file name, not the schematic's own name"
    );
    assert_eq!(house.subtitle.as_deref(), Some("Alex · 2×1×1"));
    assert_eq!(house.value.as_deref(), Some("2 个方块"));
    assert_eq!(
        house.open,
        Some(ActionId::new("open:schematics/house.litematic"))
    );
}

#[test]
fn an_empty_folder_says_so() {
    let ctx = files(vec![("notes.txt", b"x".to_vec())]);
    assert!(matches!(
        Litematica.view(&ctx, &TabState::Null).unwrap(),
        View::Empty { .. }
    ));
}

#[test]
fn detail_has_facts_a_descending_material_table_and_actions() {
    let ctx = files(vec![(
        "house.litematic",
        litematic(
            "House",
            (2, 2, 1),
            &["minecraft:air", "minecraft:stone", "minecraft:glass"],
            &[1, 2, 2, 2],
        ),
    )]);
    let (state, effects) = Litematica
        .update(
            &ctx,
            TabState::Null,
            ActionId::new("open:schematics/house.litematic"),
        )
        .unwrap();
    assert!(effects.is_empty());
    let View::Detail {
        title,
        facts,
        children,
        ..
    } = Litematica.view(&ctx, &state).unwrap()
    else {
        panic!("detail");
    };
    assert_eq!(title, "house");
    assert!(facts.contains(&("投影名称".to_owned(), "House".to_owned())));
    assert!(facts.contains(&("尺寸".to_owned(), "2 × 2 × 1".to_owned())));
    assert!(facts.contains(&("修改时间".to_owned(), "2023-11-14 22:13".to_owned())));
    assert_eq!(
        children[0],
        View::Model {
            file: "schematics/house.litematic".into()
        },
        "the 3D preview comes first, then the materials"
    );
    let View::Section {
        children: material, ..
    } = &children[1]
    else {
        panic!("materials second");
    };
    let View::Table { rows, .. } = &material[0] else {
        panic!("table");
    };
    assert_eq!(
        rows,
        &vec![
            vec!["minecraft:glass".to_owned(), "3".to_owned()],
            vec!["minecraft:stone".to_owned(), "1".to_owned()],
        ]
    );
}

#[test]
fn reveal_and_export_become_effects_and_back_closes_the_detail() {
    let ctx = files(vec![(
        "house.litematic",
        litematic("House", (2, 1, 1), &["minecraft:stone"], &[0, 0]),
    )]);
    let open = serde_json::json!({"open": "schematics/house.litematic"});
    let (_, effects) = Litematica
        .update(&ctx, open.clone(), ActionId::new("reveal"))
        .unwrap();
    assert_eq!(
        effects,
        vec![Effect::RevealGameFile {
            path: "schematics/house.litematic".into()
        }]
    );
    let (_, effects) = Litematica
        .update(&ctx, open.clone(), ActionId::new("export"))
        .unwrap();
    let [
        Effect::SaveAs {
            suggested_name,
            bytes,
        },
    ] = effects.as_slice()
    else {
        panic!("save");
    };
    assert_eq!(suggested_name, "house-材料清单.csv");
    assert_eq!(
        String::from_utf8(bytes.clone()).unwrap(),
        "\u{feff}方块,数量\r\nminecraft:stone,2\r\n"
    );
    let (state, _) = Litematica
        .update(&ctx, open, ActionId::new("back"))
        .unwrap();
    assert_eq!(state, TabState::Null);
    assert!(
        Litematica
            .update(&ctx, TabState::Null, ActionId::new("open:options.txt"))
            .is_err()
    );
}

#[test]
fn the_tab_appears_with_the_mod_or_with_existing_schematics() {
    let empty = files(vec![]);
    let with_files = files(vec![("a.litematic", Vec::new())]);
    let mut game = GameFacts::default();
    assert!(!Litematica.appears(&game, &empty));
    assert!(Litematica.appears(&game, &with_files));
    game.mods.push(ModFact {
        id: "litematica".into(),
        version: None,
        file: "litematica.jar".into(),
    });
    assert!(Litematica.appears(&game, &empty));
}

#[test]
fn dates_are_formatted_in_utc() {
    assert_eq!(crate::tab::date_for_tests(0), "1970-01-01 00:00");
    assert_eq!(
        crate::tab::date_for_tests(1_700_000_000_000),
        "2023-11-14 22:13"
    );
}

#[test]
fn subfolders_show_as_a_tag_and_unnamed_schematics_keep_their_file_name() {
    let ctx = files(vec![(
        "farms/iron.litematic",
        litematic("Unnamed", (1, 1, 1), &["minecraft:stone"], &[0]),
    )]);
    let View::List { items } = Litematica.view(&ctx, &TabState::Null).unwrap() else {
        panic!("list");
    };
    assert_eq!(items[0].title, "iron");
    assert_eq!(items[0].tags, ["farms"]);
}

struct Spec<'a> {
    name: &'a str,
    position: (i32, i32, i32),
    size: (i32, i32, i32),
    palette: &'a [&'a str],
    values: &'a [u64],
    tile_entities: Vec<(i32, i32, i32)>,
}

fn regions_file(specs: &[Spec]) -> Vec<u8> {
    let regions = specs
        .iter()
        .map(|spec| {
            let bits = crate::format::bit_width(spec.palette.len());
            let tiles = Tag::List(
                spec.tile_entities
                    .iter()
                    .map(|(x, y, z)| {
                        compound(vec![
                            ("x", Tag::Int(*x)),
                            ("y", Tag::Int(*y)),
                            ("z", Tag::Int(*z)),
                            ("id", Tag::String("minecraft:chest".into())),
                        ])
                    })
                    .collect(),
            );
            (
                spec.name,
                compound(vec![
                    (
                        "Position",
                        vec3(spec.position.0, spec.position.1, spec.position.2),
                    ),
                    ("Size", vec3(spec.size.0, spec.size.1, spec.size.2)),
                    ("BlockStatePalette", palette(spec.palette)),
                    ("BlockStates", Tag::LongArray(pack(spec.values, bits))),
                    ("TileEntities", tiles),
                ]),
            )
        })
        .collect();
    let root = compound(vec![
        ("MinecraftDataVersion", Tag::Int(4189)),
        (
            "Metadata",
            compound(vec![
                ("Name", Tag::String("multi".into())),
                ("RegionCount", Tag::Int(specs.len() as i32)),
            ]),
        ),
        ("Regions", compound(regions)),
    ]);
    gzip(&lumilio_nbt::to_bytes(&root))
}

/// The block name at a coordinate of the single merged region.
fn block_at(bytes: &[u8], x: u64, y: u64, z: u64) -> String {
    let root = lumilio_nbt::parse_maybe_gzip(bytes).unwrap();
    let Some(Tag::Compound(regions)) = root.get("Regions") else {
        panic!("regions");
    };
    assert_eq!(regions.len(), 1, "one region");
    let region = regions.values().next().unwrap();
    let size = |axis: &str| region.at(&["Size", axis]).and_then(Tag::as_i64).unwrap() as u64;
    let (sx, sz) = (size("x"), size("z"));
    let Some(Tag::List(entries)) = region.get("BlockStatePalette") else {
        panic!("palette");
    };
    let Some(Tag::LongArray(words)) = region.get("BlockStates") else {
        panic!("states");
    };
    let bits = crate::format::bit_width(entries.len());
    let index = crate::format::unpack(words, (y * sz + z) * sx + x, bits);
    entries[index as usize]
        .get("Name")
        .and_then(Tag::as_str)
        .unwrap()
        .to_owned()
}

fn spec<'a>(
    name: &'a str,
    position: (i32, i32, i32),
    size: (i32, i32, i32),
    palette: &'a [&'a str],
    values: &'a [u64],
) -> Spec<'a> {
    Spec {
        name,
        position,
        size,
        palette,
        values,
        tile_entities: Vec::new(),
    }
}

#[test]
fn regions_with_clashing_palettes_each_keep_their_own_blocks() {
    // Index 2 is soul sand in one region and glass in the other: the mix-up
    // the viewer made.
    let bytes = regions_file(&[
        spec(
            "a",
            (0, 0, 0),
            (2, 1, 1),
            &[
                "minecraft:air",
                "minecraft:packed_ice",
                "minecraft:soul_sand",
            ],
            &[2, 1],
        ),
        spec(
            "b",
            (2, 0, 0),
            (2, 1, 1),
            &[
                "minecraft:air",
                "minecraft:packed_ice",
                "minecraft:white_stained_glass",
            ],
            &[2, 2],
        ),
    ]);
    let merged = crate::merge::single_region(&bytes).unwrap();
    let blocks: Vec<_> = (0..4).map(|x| block_at(&merged, x, 0, 0)).collect();
    assert_eq!(
        blocks,
        [
            "minecraft:soul_sand",
            "minecraft:packed_ice",
            "minecraft:white_stained_glass",
            "minecraft:white_stained_glass"
        ]
    );
    // The material list agrees before and after.
    let counts = format::parse(&merged).unwrap().materials().unwrap();
    assert_eq!(counts["minecraft:white_stained_glass"], 2);
    assert_eq!(counts["minecraft:soul_sand"], 1);
    assert_eq!(counts["minecraft:packed_ice"], 1);
    let original = format::parse(&bytes).unwrap().materials().unwrap();
    assert_eq!(counts, original);
}

#[test]
fn a_negative_size_grows_a_region_towards_lower_coordinates() {
    let bytes = regions_file(&[
        spec(
            "a",
            (0, 0, 0),
            (2, 1, 1),
            &["minecraft:air", "minecraft:stone"],
            &[1, 1],
        ),
        // Spans x = 2 and 3: the corner at 3, growing back by two.
        spec(
            "b",
            (3, 0, 0),
            (-2, 1, 1),
            &["minecraft:air", "minecraft:glass", "minecraft:dirt"],
            &[1, 2],
        ),
    ]);
    let merged = crate::merge::single_region(&bytes).unwrap();
    let blocks: Vec<_> = (0..4).map(|x| block_at(&merged, x, 0, 0)).collect();
    assert_eq!(
        blocks,
        [
            "minecraft:stone",
            "minecraft:stone",
            "minecraft:glass",
            "minecraft:dirt"
        ]
    );
}

#[test]
fn air_never_erases_a_block_and_states_with_different_properties_stay_apart() {
    let palette_chest = |kind: &'static str| {
        // Same block, told apart by a property.
        compound(vec![
            ("Name", Tag::String("minecraft:chest".into())),
            (
                "Properties",
                compound(vec![("type", Tag::String(kind.into()))]),
            ),
        ])
    };
    let build = |position: (i32, i32, i32), kinds: &[&'static str]| {
        let entries: Vec<Tag> = std::iter::once(compound(vec![(
            "Name",
            Tag::String("minecraft:cave_air".into()),
        )]))
        .chain(kinds.iter().map(|kind| palette_chest(kind)))
        .collect();
        let values: Vec<u64> = (1..=kinds.len() as u64).collect();
        let bits = crate::format::bit_width(entries.len());
        compound(vec![
            ("Position", vec3(position.0, position.1, position.2)),
            ("Size", vec3(kinds.len() as i32, 1, 1)),
            ("BlockStatePalette", Tag::List(entries)),
            ("BlockStates", Tag::LongArray(pack(&values, bits))),
        ])
    };
    let root = compound(vec![(
        "Regions",
        compound(vec![
            ("a", build((0, 0, 0), &["left", "right"])),
            ("b", build((1, 0, 0), &["single", "left"])),
        ]),
    )]);
    let merged = crate::merge::single_region(&gzip(&lumilio_nbt::to_bytes(&root))).unwrap();
    let parsed = lumilio_nbt::parse_maybe_gzip(&merged).unwrap();
    let Some(Tag::List(entries)) = parsed.at(&["Regions", "merged", "BlockStatePalette"]) else {
        panic!("palette");
    };
    let types: Vec<_> = entries
        .iter()
        .filter_map(|entry| entry.at(&["Properties", "type"]).and_then(Tag::as_str))
        .collect();
    assert_eq!(types, ["left", "right", "single"], "left appears once");
    assert_eq!(
        entries[0].get("Name").and_then(Tag::as_str),
        Some("minecraft:air")
    );
}

#[test]
fn a_block_entity_moves_with_its_region() {
    let mut second = spec(
        "b",
        (5, 2, 0),
        (2, 1, 1),
        &["minecraft:air", "minecraft:chest"],
        &[1, 1],
    );
    second.tile_entities = vec![(1, 0, 0)];
    let bytes = regions_file(&[
        spec(
            "a",
            (0, 0, 0),
            (1, 1, 1),
            &["minecraft:air", "minecraft:stone"],
            &[1],
        ),
        second,
    ]);
    let merged = crate::merge::single_region(&bytes).unwrap();
    let parsed = lumilio_nbt::parse_maybe_gzip(&merged).unwrap();
    let Some(Tag::List(tiles)) = parsed.at(&["Regions", "merged", "TileEntities"]) else {
        panic!("tile entities");
    };
    assert_eq!(tiles.len(), 1);
    let at = |key| tiles[0].get(key).and_then(Tag::as_i64).unwrap();
    assert_eq!((at("x"), at("y"), at("z")), (6, 2, 0));
}

#[test]
fn one_region_goes_through_untouched_and_an_oversized_merge_is_refused() {
    let one = litematic("House", (2, 1, 1), &["minecraft:stone"], &[0, 0]);
    assert_eq!(crate::merge::single_region(&one).unwrap(), one);
    let huge = regions_file(&[
        spec("a", (0, 0, 0), (1, 1, 1), &["minecraft:air"], &[0]),
        spec("b", (6000, 6000, 6000), (1, 1, 1), &["minecraft:air"], &[0]),
    ]);
    assert!(crate::merge::single_region(&huge).is_err());
    assert!(crate::merge::single_region(b"not nbt").is_err());
}

#[test]
fn the_viewer_is_given_the_merged_schematic_and_a_bad_file_is_an_error() {
    let bytes = regions_file(&[
        spec(
            "a",
            (0, 0, 0),
            (1, 1, 1),
            &["minecraft:air", "minecraft:stone"],
            &[1],
        ),
        spec(
            "b",
            (1, 0, 0),
            (1, 1, 1),
            &["minecraft:air", "minecraft:glass"],
            &[1],
        ),
    ]);
    let ctx = files(vec![
        ("multi.litematic", bytes),
        ("bad.litematic", b"junk".to_vec()),
    ]);
    let served = Litematica
        .model(&ctx, "schematics/multi.litematic")
        .unwrap();
    assert_eq!(block_at(&served, 1, 0, 0), "minecraft:glass");
    assert!(Litematica.model(&ctx, "schematics/bad.litematic").is_err());
    assert!(
        Litematica
            .model(&ctx, "schematics/missing.litematic")
            .is_err()
    );
}
