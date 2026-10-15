use std::collections::BTreeMap;
use std::io::Write;

use lumilio_nbt::Tag;
use lumilio_plugin_api::{
    ActionId, Effect, FetchResponse, GameFacts, HostContext, InstanceTab, ModFact, Plugin,
    PluginError, SettingValue, TabState, View,
};

use crate::format::{self, pack};
use crate::{FOLDER, Litematica};

#[test]
fn placement_overlay_uses_world_config_and_schematic_region_bounds() {
    use lumilio_plugin_api::map::{
        Dimension, MapBounds, MapObjectKind, MapPoint, OverlayProvider, OverlayRequest,
        WorldContext, WorldId,
    };
    let world = WorldId::Save {
        instance: "i".into(),
        folder: "New World".into(),
    };
    let context = WorldContext {
        world: world.clone(),
        version: None,
        data_version: None,
        seed: None,
        dimension: Dimension::Overworld,
        sources: vec![],
    };
    let config = serde_json::json!({
        "placements": { "placements": [
            {"name":"House", "enabled":true, "origin":[100,64,-20],
             "rotation":"NONE", "mirror":"NONE", "bb_color":-65281,
             "schematic":"/game/schematics/house.litematic",
             "placements":[{"name":"main", "placement":{"pos":[2,0,3],
                 "rotation":"NONE", "mirror":"NONE", "enabled":true}}]},
            {"name":"Hidden", "enabled":false, "origin":[0,0,0],
             "schematic":"/game/schematics/house.litematic"}
        ]}
    });
    let ctx = Files(BTreeMap::from([
        (
            "config/litematica/litematica_New World_dim_minecraft_overworld.json".into(),
            serde_json::to_vec(&config).unwrap(),
        ),
        (
            "schematics/house.litematic".into(),
            litematic("House", (2, 1, 3), &["minecraft:stone"], &[0; 6]),
        ),
    ]));
    let request = OverlayRequest {
        context,
        overlay: "litematica.placements".into(),
        bounds: MapBounds {
            min: MapPoint { x: 0., z: -100. },
            max: MapPoint { x: 200., z: 100. },
        },
        level: 0,
    };
    let objects = Litematica.objects(&ctx, &request).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].label.as_deref(), Some("House"));
    assert_eq!(objects[0].color, Some([255, 0, 255]));
    assert_eq!(
        objects[0].kind,
        MapObjectKind::Polyline(vec![
            MapPoint { x: 102., z: -17. },
            MapPoint { x: 104., z: -17. },
            MapPoint { x: 104., z: -14. },
            MapPoint { x: 102., z: -14. },
            MapPoint { x: 102., z: -17. },
        ])
    );
    let mut outside = request;
    outside.bounds = MapBounds {
        min: MapPoint { x: -200., z: -200. },
        max: MapPoint { x: -100., z: -100. },
    };
    assert!(Litematica.objects(&ctx, &outside).unwrap().is_empty());
    assert!(Litematica.manifest().permissions.iter().any(|permission|
        matches!(permission, lumilio_plugin_api::Permission::ReadGameFiles { under } if under == "config/litematica")));
}

#[test]
#[ignore = "set LUMILIO_LITEMATICA_GAME to a local game folder with a saved placement"]
fn local_placement_and_schematic_agree_on_subregion_names() {
    use lumilio_plugin_api::map::{
        Dimension, MapBounds, MapPoint, OverlayProvider, OverlayRequest, WorldContext, WorldId,
    };
    let game =
        std::path::PathBuf::from(std::env::var("LUMILIO_LITEMATICA_GAME").expect("game folder"));
    let name = "litematica_paranoid.modrinth.gg_dim_minecraft_overworld.json";
    let mut config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(game.join("config/litematica").join(name)).unwrap())
            .unwrap();
    let placements = config
        .pointer_mut("/placements/placements")
        .unwrap()
        .as_array_mut()
        .unwrap();
    let first = placements.first_mut().unwrap();
    first["enabled"] = serde_json::Value::Bool(true);
    let schematic = first["schematic"]
        .as_str()
        .unwrap()
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap()
        .to_owned();
    let ctx = Files(BTreeMap::from([
        (
            format!("config/litematica/{name}"),
            serde_json::to_vec(&config).unwrap(),
        ),
        (
            format!("schematics/{schematic}"),
            std::fs::read(game.join("schematics").join(schematic)).unwrap(),
        ),
    ]));
    let objects = Litematica
        .objects(
            &ctx,
            &OverlayRequest {
                context: WorldContext {
                    world: WorldId::Server {
                        instance: "test".into(),
                        address: "paranoid.modrinth.gg".into(),
                    },
                    version: None,
                    data_version: None,
                    seed: None,
                    dimension: Dimension::Overworld,
                    sources: vec![],
                },
                overlay: "litematica.placements".into(),
                bounds: MapBounds {
                    min: MapPoint {
                        x: -30_000_000.,
                        z: -30_000_000.,
                    },
                    max: MapPoint {
                        x: 30_000_000.,
                        z: 30_000_000.,
                    },
                },
                level: 0,
            },
        )
        .unwrap();
    assert!(!objects.is_empty());
}

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
fn the_tab_is_the_hosts_studio_over_the_schematics_folder() {
    let ctx = files(vec![("house.litematic", Vec::new())]);
    assert_eq!(
        Litematica.view(&ctx, &TabState::Null).unwrap(),
        View::Studio {
            folder: FOLDER.into()
        }
    );
}

#[test]
fn reveal_and_export_name_one_file_and_become_effects() {
    let ctx = files(vec![
        (
            "farms/house.litematic",
            litematic(
                "House",
                (2, 2, 1),
                &["minecraft:air", "minecraft:stone", "minecraft:glass"],
                &[1, 2, 2, 2],
            ),
        ),
        ("broken.litematic", b"not nbt".to_vec()),
    ]);
    let (_, effects) = Litematica
        .update(
            &ctx,
            TabState::Null,
            ActionId::new("reveal:schematics/farms/house.litematic"),
        )
        .unwrap();
    assert_eq!(
        effects,
        vec![Effect::RevealGameFile {
            path: "schematics/farms/house.litematic".into()
        }]
    );
    let (_, effects) = Litematica
        .update(
            &ctx,
            TabState::Null,
            ActionId::new("export:schematics/farms/house.litematic"),
        )
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
        "\u{feff}方块,数量,组数\r\nminecraft:glass,3,3\r\nminecraft:stone,1,1\r\n",
        "most first, air left out"
    );
    let (_, effects) = Litematica
        .update(
            &ctx,
            TabState::Null,
            ActionId::new("export:schematics/broken.litematic"),
        )
        .unwrap();
    assert!(matches!(effects.as_slice(), [Effect::Toast(_)]));
    for outside in [
        "reveal:options.txt",
        "export:schematics/notes.txt",
        "reveal:schematics/../options.litematic",
        "export:schematicsx/a.litematic",
    ] {
        assert!(
            Litematica
                .update(&ctx, TabState::Null, ActionId::new(outside))
                .is_err(),
            "{outside}"
        );
    }
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
fn only_litematic_files_in_the_schematics_folder_may_be_written() {
    let writes: Vec<_> = Litematica
        .manifest()
        .permissions
        .into_iter()
        .filter_map(|permission| match permission {
            lumilio_plugin_api::Permission::WriteGameFiles { under, names } => Some((under, names)),
            _ => None,
        })
        .collect();
    assert_eq!(writes, [(FOLDER.to_owned(), "*.litematic".to_owned())]);
}

/// The material list's stacks column says what the studio's materials say
/// (plan schematic-studio T25).
#[test]
fn the_material_list_counts_stacks_as_the_studio_does() {
    let zh = crate::text::Text::new("zh-CN");
    let en = crate::text::Text::new("en-US");
    for (count, chinese, english) in [
        (22, "22", "22"),
        (64, "1 组", "1 stack"),
        (192, "3 组", "3 stacks"),
        (86, "1 组 + 22", "1 stack + 22"),
        (150, "2 组 + 22", "2 stacks + 22"),
    ] {
        assert_eq!(zh.stacks(count), chinese, "{count}");
        assert_eq!(en.stacks(count), english, "{count}");
    }
}
