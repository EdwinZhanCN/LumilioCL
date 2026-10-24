use lumilio_plugin_api::map::{TileKey, WorldContext, WorldId};
use lumilio_plugin_api::{FetchResponse, SettingValue};

use super::*;

#[test]
fn new_release_worlds_and_data_version_only_saves_reach_the_provider() {
    for name in ["1.21.5", "1.21.11", "26.1.2", "26.2", "26.3"] {
        let world = overlay_request("", Dimension::Overworld, name, [0.; 4]).context;
        let offered = WorldExplorer.overlays_for(&world);
        assert!(
            offered.iter().any(|l| l.id == "structure.village"),
            "{name}"
        );
        assert_eq!(
            offered.iter().any(|l| l.id == "structure.abandoned-camp"),
            name == "26.3"
        );
        for camp in offered
            .iter()
            .filter(|l| l.id == "structure.abandoned-camp")
        {
            assert!(camp.approximate);
        }
    }
    let mut world = overlay_request("", Dimension::Overworld, "26.3", [0.; 4]).context;
    world.version = None;
    world.data_version = Some(5023);
    assert!(
        WorldExplorer
            .overlays_for(&world)
            .iter()
            .any(|l| l.id == "structure.abandoned-camp")
    );
    world.data_version = Some(5024);
    assert!(WorldExplorer.overlays_for(&world).is_empty());
}

struct Context {
    cancelled: bool,
}
impl HostContext for Context {
    fn cancelled(&self) -> bool {
        self.cancelled
    }
    fn setting(&self, _: &str) -> Option<SettingValue> {
        None
    }
    fn read_file(&self, _: &str) -> Result<Vec<u8>, PluginError> {
        panic!("seed tiles do not read files")
    }
    fn list_files(&self, _: &str) -> Result<Vec<String>, PluginError> {
        panic!("seed tiles do not list files")
    }
    fn fetch(&self, _: &str) -> Result<FetchResponse, PluginError> {
        panic!("seed tiles never use network")
    }
}
fn request() -> TileRequest {
    let world = WorldId::Seed {
        seed: 262,
        version: "1.21.4".into(),
    };
    TileRequest {
        context: WorldContext {
            world: world.clone(),
            version: Some("1.21.4".into()),
            data_version: None,
            seed: Some(262),
            dimension: Dimension::Overworld,
            sources: vec![],
        },
        key: TileKey {
            provider: ID.into(),
            base_map: "seed".into(),
            world,
            dimension: Dimension::Overworld,
            level: 2,
            tx: -1,
            tz: 0,
        },
        pixels: 256,
    }
}
#[test]
fn seed_provider_generates_valid_tiles_and_rejects_new_versions() {
    let context = Context { cancelled: false };
    assert!(WorldExplorer.tile(&context, &request()).unwrap().is_valid());
    let mut unsupported = request();
    unsupported.context.version = Some("26.4".into());
    assert_eq!(
        WorldExplorer.tile(&context, &unsupported),
        Err(PluginError::Unavailable("map-version-unsupported".into()))
    );
    assert!(
        WorldExplorer
            .tile(&Context { cancelled: true }, &request())
            .is_err()
    );
    assert!(WorldExplorer.manifest().default_enabled);
}

fn overlay_request(
    overlay: &str,
    dimension: Dimension,
    version: &str,
    area: [f64; 4],
) -> OverlayRequest {
    OverlayRequest {
        context: WorldContext {
            world: WorldId::Seed {
                seed: 262,
                version: version.into(),
            },
            version: Some(version.into()),
            data_version: None,
            seed: Some(262),
            dimension,
            sources: vec![],
        },
        overlay: overlay.into(),
        bounds: lumilio_plugin_api::map::MapBounds {
            min: lumilio_plugin_api::map::MapPoint {
                x: area[0],
                z: area[1],
            },
            max: lumilio_plugin_api::map::MapPoint {
                x: area[2],
                z: area[3],
            },
        },
        level: 0,
    }
}

fn spots(objects: &[lumilio_plugin_api::map::MapObject]) -> Vec<[i32; 2]> {
    objects
        .iter()
        .map(|object| match &object.kind {
            lumilio_plugin_api::map::MapObjectKind::Icon { at, .. } => [at.x as i32, at.z as i32],
            other => panic!("structures are icons, got {other:?}"),
        })
        .collect()
}

#[test]
fn structure_layers_follow_the_worlds_version_and_dimension() {
    let ids = |request: &OverlayRequest| -> Vec<String> {
        WorldExplorer
            .overlays_for(&request.context)
            .into_iter()
            .map(|layer| layer.id)
            .collect()
    };
    let has = |ids: &[String], name: &str| ids.iter().any(|id| id == &format!("structure.{name}"));
    let old = ids(&overlay_request(
        "",
        Dimension::Overworld,
        "1.16.5",
        [0., 0., 1., 1.],
    ));
    assert!(has(&old, "village") && has(&old, "stronghold"));
    assert!(
        !has(&old, "ancient-city") && !has(&old, "trial-chambers"),
        "{old:?}"
    );
    assert!(
        !has(&old, "fortress"),
        "nether kinds stay out of the Overworld"
    );
    let new = ids(&overlay_request(
        "",
        Dimension::Overworld,
        "1.21.4",
        [0., 0., 1., 1.],
    ));
    assert!(has(&new, "ancient-city") && has(&new, "trial-chambers") && has(&new, "trail-ruins"));
    let nether = ids(&overlay_request(
        "",
        Dimension::Nether,
        "1.21.4",
        [0., 0., 1., 1.],
    ));
    assert!(has(&nether, "fortress") && has(&nether, "bastion") && has(&nether, "ruined-portal"));
    assert!(!has(&nether, "village") && !has(&nether, "stronghold"));
    let end = ids(&overlay_request(
        "",
        Dimension::End,
        "1.21.4",
        [0., 0., 1., 1.],
    ));
    assert_eq!(end, ["structure.end-city", "structure.end-gateway"]);
    // Height-bound kinds are marked estimated from 1.18 on, and only those.
    let flags = |version: &str| -> Vec<(String, bool)> {
        WorldExplorer
            .overlays_for(
                &overlay_request("", Dimension::Overworld, version, [0., 0., 1., 1.]).context,
            )
            .into_iter()
            .filter(|layer| layer.id.starts_with("structure."))
            .map(|layer| (layer.id, layer.approximate))
            .collect()
    };
    assert!(flags("1.16.5").iter().all(|(_, estimated)| !estimated));
    let estimated: Vec<_> = flags("1.21.4")
        .into_iter()
        .filter(|(_, estimated)| *estimated)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(
        estimated,
        [
            "structure.desert-pyramid",
            "structure.jungle-temple",
            "structure.mansion"
        ]
    );
    // No seed or a version cubiomes lacks: nothing is offered.
    let mut seedless = overlay_request("", Dimension::Overworld, "1.21.4", [0., 0., 1., 1.]);
    seedless.context.seed = None;
    assert!(WorldExplorer.overlays_for(&seedless.context).is_empty());
    assert!(
        WorldExplorer
            .overlays_for(
                &overlay_request("", Dimension::Overworld, "26.4", [0., 0., 1., 1.]).context
            )
            .is_empty()
    );
    // 25 structure layers, 3 decoration layers, the apple layer, 3
    // underground layers, spawn, slime chunks, save positions and Xaero
    // waypoints.
    assert_eq!(WorldExplorer.overlays().len(), 36);
}

#[test]
fn vein_zones_cover_the_raw_ore_an_original_server_saved() {
    use lumilio_plugin_api::map::MapObjectKind;
    let context = Context { cancelled: false };
    // Raw blocks the original 26.3 server saved for seed 262
    // (crates/lumilio-cubiomes/tests/golden/26.3-ores.txt): (x, z, iron).
    let saved = [
        (7, 45, true),
        (21, 20, true),
        (41, -2, true),
        (34, 18, true),
        (-981, 3133, true),
        (-965, 3119, true),
        (-915, 3218, false),
        (-916, 3221, false),
    ];
    let heat = |overlay: &str, area: [f64; 4]| {
        let objects = WorldExplorer
            .objects(
                &context,
                &overlay_request(overlay, Dimension::Overworld, "26.3", area),
            )
            .unwrap();
        objects
            .into_iter()
            .flat_map(|object| match object.kind {
                MapObjectKind::Heat { cell, values } => {
                    assert_eq!(cell, 16.);
                    assert!(object.color.is_some() && !object.approximate);
                    values
                }
                other => panic!("veins are heat, got {other:?}"),
            })
            .collect::<Vec<_>>()
    };
    for (overlay, iron) in [
        ("underground.iron-vein", true),
        ("underground.copper-vein", false),
    ] {
        let squares = heat(overlay, [-1024., -64., 64., 4032.]);
        assert!(squares.iter().all(|(_, v)| (0.35..=1.).contains(v)));
        for &(x, z, _) in saved.iter().filter(|(.., i)| *i == iron) {
            let (x, z) = (f64::from(x), f64::from(z));
            assert!(
                squares
                    .iter()
                    .any(|(at, _)| (at.x..at.x + 16.).contains(&x)
                        && (at.z..at.z + 16.).contains(&z)),
                "{overlay} misses {x} {z}"
            );
        }
    }
    // Large veins began in 1.18, and only the Overworld has them.
    let offered = |dimension: Dimension, version: &str| {
        WorldExplorer
            .overlays_for(&overlay_request("", dimension, version, [0., 0., 1., 1.]).context)
            .into_iter()
            .filter(|layer| layer.id.ends_with("-vein"))
            .count()
    };
    assert_eq!(offered(Dimension::Overworld, "26.3"), 2);
    assert_eq!(offered(Dimension::Overworld, "1.18.2"), 2);
    assert_eq!(offered(Dimension::Overworld, "1.16.5"), 0);
    assert_eq!(offered(Dimension::Nether, "26.3"), 0);
}

#[test]
fn ravines_draw_their_start_and_walk() {
    use lumilio_plugin_api::map::{MapIcon, MapObjectKind};
    let context = Context { cancelled: false };
    let objects = WorldExplorer
        .objects(
            &context,
            &overlay_request(
                "underground.ravine",
                Dimension::Overworld,
                "26.3",
                [-192., -192., 192., 192.],
            ),
        )
        .unwrap();
    // The original 26.3 carver starts canyons for seed 262 in chunks (7, -11)
    // and (-7, 11) (crates/lumilio-cubiomes/tests/golden/26.3-ravines.txt).
    let starts: Vec<[i32; 2]> = objects
        .iter()
        .filter_map(|object| match &object.kind {
            MapObjectKind::Icon { icon, at } => {
                assert!(matches!(icon, MapIcon::Ravine | MapIcon::UnderwaterRavine));
                Some([(at.x as i32).div_euclid(16), (at.z as i32).div_euclid(16)])
            }
            _ => None,
        })
        .collect();
    assert!(
        starts.contains(&[7, -11]) && starts.contains(&[-7, 11]),
        "{starts:?}"
    );
    // Each start comes with its walk: 84 to 111 steps from 112-block reach.
    let walks: Vec<usize> = objects
        .iter()
        .filter_map(|object| match &object.kind {
            MapObjectKind::Polyline(points) => Some(points.len()),
            _ => None,
        })
        .collect();
    assert_eq!(walks.len(), starts.len());
    assert!(walks.iter().all(|n| (84..112).contains(n)), "{walks:?}");
    assert!(objects.iter().all(|o| !o.approximate && o.color.is_some()));
}

#[test]
fn apple_layer_marks_the_pyramid_whose_chest_holds_one() {
    use lumilio_plugin_api::map::MapObjectKind;
    let context = Context { cancelled: false };
    let found = |area: [f64; 4]| -> Vec<[i32; 2]> {
        WorldExplorer
            .objects(
                &context,
                &overlay_request(
                    "feature.enchanted-golden-apple",
                    Dimension::Overworld,
                    "26.3",
                    area,
                ),
            )
            .unwrap()
            .into_iter()
            .map(|object| match object.kind {
                MapObjectKind::Icon { at, .. } => {
                    assert!(object.approximate);
                    [at.x as i32, at.z as i32]
                }
                other => panic!("apples are icons, got {other:?}"),
            })
            .collect()
    };
    // The original 26.3 server filled a chest of seed 262's pyramid at chunk
    // (464, 416) with an apple; the one at (432, -595) holds none
    // (crates/lumilio-cubiomes/tests/golden/26.3-pyramid-loot.txt).
    assert!(found([7168., 6400., 7680., 6912.]).contains(&[7434, 6666]));
    assert!(!found([6656., -9728., 7168., -9216.]).contains(&[6922, -9510]));
}

#[test]
fn decoration_and_nether_fossil_layers_need_their_tables() {
    let offered = |dimension: Dimension, version: &str| -> Vec<(String, bool, Option<String>)> {
        WorldExplorer
            .overlays_for(&overlay_request("", dimension, version, [0., 0., 1., 1.]).context)
            .into_iter()
            .map(|layer| (layer.id, layer.approximate, layer.group_id))
            .collect()
    };
    let features = |list: &[(String, bool, Option<String>)]| -> Vec<String> {
        list.iter()
            .filter(|(id, ..)| id.starts_with("feature."))
            .map(|(id, ..)| id.clone())
            .collect()
    };
    let new = offered(Dimension::Overworld, "26.3");
    assert_eq!(
        features(&new),
        [
            "feature.fossil",
            "feature.fossil-diamond",
            "feature.lava-lake",
            "feature.enchanted-golden-apple"
        ]
    );
    // Pyramid loot needs data-driven enchantments (1.21+).
    let apples = |version: &str| {
        features(&offered(Dimension::Overworld, version))
            .contains(&"feature.enchanted-golden-apple".to_owned())
    };
    assert!(apples("1.21.1") && !apples("1.20.6"));
    // Terrain decides each of them: estimated, in their own group.
    assert!(
        new.iter()
            .filter(|(id, ..)| id.starts_with("feature."))
            .all(|(_, estimated, group)| *estimated
                && group.as_deref() == Some("map-group-features"))
    );
    // No original-server table before 1.19.4: nothing is offered.
    assert!(features(&offered(Dimension::Overworld, "1.18.2")).is_empty());
    assert!(features(&offered(Dimension::Overworld, "1.16.5")).is_empty());
    let has_fossils = |version: &str| {
        offered(Dimension::Nether, version)
            .iter()
            // Exact: the fork models the Nether's terrain the column needs.
            .any(|(id, estimated, _)| id == "structure.nether-fossil" && !*estimated)
    };
    assert!(has_fossils("26.3") && has_fossils("1.18.2"));
    assert!(!has_fossils("1.16.5"));
}

#[test]
fn structure_objects_show_their_start_variant() {
    use lumilio_plugin_api::map::{MapIcon, MapObjectKind};
    let context = Context { cancelled: false };
    let found = |overlay: &str, dimension: Dimension, area: [f64; 4]| {
        WorldExplorer
            .objects(&context, &overlay_request(overlay, dimension, "26.3", area))
            .unwrap()
            .into_iter()
            .map(|object| match object.kind {
                MapObjectKind::Icon { icon, at } => (
                    [at.x as i32, at.z as i32],
                    icon,
                    object.label_id.unwrap_or_default(),
                ),
                other => panic!("structures are icons, got {other:?}"),
            })
            .collect::<Vec<_>>()
    };
    let at = |list: &[([i32; 2], MapIcon, String)], spot: [i32; 2]| {
        list.iter()
            .find(|(found, ..)| *found == spot)
            .map(|(_, icon, label)| (*icon, label.clone()))
            .unwrap_or_else(|| panic!("no structure at {spot:?} in {list:?}"))
    };
    // Starts saved by the original 26.3 server for seed 262
    // (crates/lumilio-cubiomes/tests/golden/26.3-variants.txt).
    let bastions = found(
        "structure.bastion",
        Dimension::Nether,
        [-2048., -2048., 2048., 2048.],
    );
    for (spot, icon, label) in [
        ([144, 1456], MapIcon::BastionHousing, "housing"),
        ([-1520, 32], MapIcon::BastionHoglinStable, "hoglin-stable"),
        ([256, 48], MapIcon::BastionTreasure, "treasure"),
        ([112, -1648], MapIcon::BastionBridge, "bridge"),
    ] {
        assert_eq!(
            at(&bastions, spot),
            (icon, format!("map-structure-bastion-{label}"))
        );
    }
    let igloos = found(
        "structure.igloo",
        Dimension::Overworld,
        [-2048., -1024., 3072., 1536.],
    );
    assert_eq!(
        at(&igloos, [-384, 800]),
        (
            MapIcon::IglooBasement,
            "map-structure-igloo-basement".into()
        )
    );
    assert_eq!(
        at(&igloos, [2560, -400]),
        (MapIcon::IglooNoBasement, "map-structure-igloo".into())
    );
    let cities = found(
        "structure.end-city",
        Dimension::End,
        [1024., 0., 2048., 2560.],
    );
    assert_eq!(
        at(&cities, [1712, 96]),
        (MapIcon::EndCity, "map-structure-end-city-ship".into())
    );
    assert_eq!(
        at(&cities, [1296, 2272]),
        (MapIcon::EndCity, "map-structure-end-city".into())
    );
    // A ruined portal has its own icon in the Nether, on the layer too.
    let portals = found(
        "structure.ruined-portal",
        Dimension::Nether,
        [-1024., -1024., 1024., 1024.],
    );
    assert!(!portals.is_empty());
    assert!(
        portals
            .iter()
            .all(|(_, icon, _)| *icon == MapIcon::RuinedPortalNether)
    );
    let layer = WorldExplorer
        .overlays_for(&overlay_request("", Dimension::Nether, "26.3", [0., 0., 1., 1.]).context)
        .into_iter()
        .find(|layer| layer.id == "structure.ruined-portal")
        .unwrap();
    assert_eq!(layer.icon, Some(MapIcon::RuinedPortalNether));
}

#[test]
fn structure_objects_match_cubiomes_and_carry_their_identity() {
    let context = Context { cancelled: false };
    let whole = [-1536., -1536., 1536., 1536.];
    let villages = WorldExplorer
        .objects(
            &context,
            &overlay_request("structure.village", Dimension::Overworld, "1.21.4", whole),
        )
        .unwrap();
    // Positions from the C probe (crates/lumilio-cubiomes/tests/structures.txt).
    assert_eq!(
        spots(&villages),
        [
            [-1264, -144],
            [-800, -240],
            [-768, 704],
            [-416, 832],
            [-1520, 1136],
            [-1040, 1232],
            [1488, 1152]
        ]
    );
    let first = &villages[0];
    assert_eq!(first.id, "structure.village:-1264:-144");
    assert_eq!(first.label_id.as_deref(), Some("map-structure-village"));
    assert_eq!(first.dimension, Dimension::Overworld);
    assert!(!first.approximate);
    // A smaller window keeps only what is inside it.
    let corner = WorldExplorer
        .objects(
            &context,
            &overlay_request(
                "structure.village",
                Dimension::Overworld,
                "1.21.4",
                [-900., -300., -700., 0.],
            ),
        )
        .unwrap();
    assert_eq!(spots(&corner), [[-800, -240]]);
    // Strongholds come from the cached generation-order list.
    let strongholds = WorldExplorer
        .objects(
            &context,
            &overlay_request(
                "structure.stronghold",
                Dimension::Overworld,
                "1.21.4",
                [-2000., -2000., 2100., 1300.],
            ),
        )
        .unwrap();
    assert_eq!(
        spots(&strongholds),
        [[-12, -1708], [2036, 1220], [-1772, 836]]
    );
    assert!(strongholds.iter().all(|object| object.priority > 0));
    // A layer the world does not have is empty; an unknown one is an error.
    assert!(
        WorldExplorer
            .objects(
                &context,
                &overlay_request("structure.fortress", Dimension::Overworld, "1.21.4", whole)
            )
            .unwrap()
            .is_empty()
    );
    assert!(
        WorldExplorer
            .objects(
                &context,
                &overlay_request("structure.nope", Dimension::Overworld, "1.21.4", whole)
            )
            .is_err()
    );
}

#[test]
fn structure_objects_reject_bad_bounds_and_stop_when_cancelled() {
    let context = Context { cancelled: false };
    for area in [
        [0., 0., 0., 10.],
        [f64::NAN, 0., 10., 10.],
        [0., 0., 1.0e9, 10.],
        [0., 0., 200_000., 10.],
    ] {
        assert!(
            WorldExplorer
                .objects(
                    &context,
                    &overlay_request("structure.village", Dimension::Overworld, "1.21.4", area)
                )
                .is_err(),
            "{area:?}"
        );
    }
    assert_eq!(
        WorldExplorer.objects(
            &Context { cancelled: true },
            &overlay_request(
                "structure.village",
                Dimension::Overworld,
                "1.21.4",
                [0., 0., 8192., 8192.]
            )
        ),
        Err(PluginError::Transient("map-cancelled".into()))
    );
    assert_eq!(
        WorldExplorer.objects(
            &context,
            &overlay_request(
                "structure.village",
                Dimension::Overworld,
                "26.4",
                [0., 0., 512., 512.]
            )
        ),
        Err(PluginError::Unavailable("map-version-unsupported".into()))
    );
}

#[test]
fn landmark_layers_are_overworld_only_and_slime_is_a_fine_zoom_layer() {
    let overworld = overlay_request("world.spawn", Dimension::Overworld, "1.21.4", [0.; 4]).context;
    let layers = WorldExplorer.overlays_for(&overworld);
    let ids: Vec<&str> = layers
        .iter()
        .filter(|layer| layer.group_id.as_deref() == Some(landmarks::GROUP))
        .map(|layer| layer.id.as_str())
        .collect();
    assert_eq!(ids, ["world.spawn", "world.slime"]);
    let slime = layers
        .iter()
        .find(|layer| layer.id == "world.slime")
        .unwrap();
    assert_eq!(slime.max_scale, Some(1));
    let spawn = |version: &str| {
        let context =
            overlay_request("world.spawn", Dimension::Overworld, version, [0.; 4]).context;
        WorldExplorer
            .overlays_for(&context)
            .into_iter()
            .find(|layer| layer.id == "world.spawn")
            .unwrap()
            .approximate
    };
    assert!(spawn("1.16.5"), "before 1.18 the spawn is an estimate");
    assert!(spawn("1.21.4"));
    let mut nether = overworld;
    nether.dimension = Dimension::Nether;
    assert!(
        WorldExplorer
            .overlays_for(&nether)
            .iter()
            .all(|layer| layer.group_id.as_deref() != Some(landmarks::GROUP))
    );
}

#[test]
fn spawn_and_slime_objects_match_cubiomes() {
    let context = Context { cancelled: false };
    let spawn =
        lumilio_cubiomes::spawn(lumilio_cubiomes::Version::from_name("1.21.4").unwrap(), 262);
    let around = |overlay: &str, area: [f64; 4]| {
        WorldExplorer
            .objects(
                &context,
                &overlay_request(overlay, Dimension::Overworld, "1.21.4", area),
            )
            .unwrap()
    };
    let (x, z) = (f64::from(spawn.at[0]), f64::from(spawn.at[1]));
    let found = around("world.spawn", [x - 10., z - 10., x + 10., z + 10.]);
    assert_eq!(spots(&found), [spawn.at]);
    assert!(found[0].approximate);
    assert!(around("world.spawn", [x + 20., z + 20., x + 40., z + 40.]).is_empty());

    // 256x256 chunks: the one Heat object lists exactly cubiomes' slime chunks.
    let found = around("world.slime", [-2048., -2048., 2048., 2048.]);
    let [slime] = found.as_slice() else {
        panic!("one object per request, got {}", found.len());
    };
    let lumilio_plugin_api::map::MapObjectKind::Heat { cell, values } = &slime.kind else {
        panic!("slime chunks are heat cells");
    };
    assert_eq!(*cell, 16.);
    let flags = lumilio_cubiomes::slime_chunks(262, -128, -128, 256, 256).unwrap();
    assert_eq!(values.len(), flags.iter().filter(|slime| **slime).count());
    for (at, _) in values {
        assert_eq!(at.x % 16., 0.);
        let (cx, cz) = ((at.x / 16.) as i32 + 128, (at.z / 16.) as i32 + 128);
        assert!(flags[(cz * 256 + cx) as usize], "chunk {cx},{cz}");
    }
    assert!(!values.is_empty());
    // Not a slime layer in the Nether, and a bad box is refused.
    let nether = overlay_request(
        "world.slime",
        Dimension::Nether,
        "1.21.4",
        [0., 0., 64., 64.],
    );
    assert!(WorldExplorer.objects(&context, &nether).unwrap().is_empty());
    let bad = overlay_request(
        "world.slime",
        Dimension::Overworld,
        "1.21.4",
        [0., 0., 0., 64.],
    );
    assert!(WorldExplorer.objects(&context, &bad).is_err());
}
