use super::*;
use lumilio_plugin_api::map::{TileKey, WorldContext, WorldId};
use lumilio_plugin_api::{FetchResponse, SettingValue};
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
    unsupported.context.version = Some("26.3".into());
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
    assert_eq!(end, ["structure.end-city"]);
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
                &overlay_request("", Dimension::Overworld, "26.3", [0., 0., 1., 1.]).context
            )
            .is_empty()
    );
    // 18 structure layers, spawn, slime chunks, save positions and Xaero waypoints.
    assert_eq!(WorldExplorer.overlays().len(), 22);
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
                "26.3",
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
    assert!(!spawn("1.21.4"));
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
    assert!(!found[0].approximate);
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
