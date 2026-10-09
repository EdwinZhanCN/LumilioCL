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
    assert_eq!(WorldExplorer.overlays().len(), 18);
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
