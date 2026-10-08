use super::*;
use lumilio_plugin_api::map::*;

fn request() -> TileRequest {
    let world = WorldId::Seed {
        seed: 262,
        version: "1.21.4".into(),
    };
    TileRequest {
        context: WorldContext {
            world: world.clone(),
            seed: Some(262),
            version: Some("1.21.4".into()),
            data_version: None,
            dimension: Dimension::Overworld,
            sources: vec![],
        },
        key: TileKey {
            provider: "test.seed".into(),
            base_map: "seed".into(),
            world,
            dimension: Dimension::Overworld,
            level: 0,
            tx: 0,
            tz: 0,
        },
        pixels: 256,
    }
}
#[test]
fn stale_generations_and_duplicate_keys_are_rejected() {
    let key = request().key;
    let mut schedule = MapSchedule::default();
    let generation = schedule.update(std::slice::from_ref(&key));
    let (_, cancel) = schedule.begin(&key).unwrap();
    assert!(schedule.begin(&key).is_none());
    schedule.update(std::slice::from_ref(&key));
    assert!(cancel.is_cancelled());
    assert!(!schedule.accept(generation, &key));
    let (generation, _) = schedule.begin(&key).unwrap();
    assert!(schedule.accept(generation, &key));
}
#[test]
fn cache_hits_and_lru_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let cache = TileCache::with_limit(dir.path().to_owned(), 14);
    let a = request();
    let mut b = a.clone();
    b.key.tx = 1;
    let mut c = a.clone();
    c.key.tx = 2;
    cache.put(&a, &TileReply::Empty).unwrap();
    cache.put(&b, &TileReply::Empty).unwrap();
    assert_eq!(cache.get(&a).unwrap(), Some(TileReply::Empty));
    cache.put(&c, &TileReply::Empty).unwrap();
    assert_eq!(cache.get(&a).unwrap(), Some(TileReply::Empty));
    assert_eq!(cache.get(&b).unwrap(), None);
    assert_eq!(cache.get(&c).unwrap(), Some(TileReply::Empty));
}
#[test]
fn visible_tiles_include_negative_prefetch_and_center_first() {
    let viewport = Viewport {
        x: -128.0,
        z: -128.0,
        blocks_per_pixel: 1.0,
        width: 128,
        height: 128,
    };
    let tiles = viewport.visible(&request().key);
    assert_eq!((tiles[0].tx, tiles[0].tz), (-1, -1));
    assert_eq!(tiles.len(), 9);
}

#[test]
fn region_lines_are_exact_for_negative_coordinates() {
    struct NoIo;
    impl lumilio_plugin_api::HostContext for NoIo {
        fn setting(&self, _: &str) -> Option<lumilio_plugin_api::SettingValue> {
            None
        }
        fn read_file(&self, _: &str) -> Result<Vec<u8>, lumilio_plugin_api::PluginError> {
            panic!("no I/O")
        }
        fn list_files(&self, _: &str) -> Result<Vec<String>, lumilio_plugin_api::PluginError> {
            panic!("no I/O")
        }
        fn fetch(
            &self,
            _: &str,
        ) -> Result<lumilio_plugin_api::FetchResponse, lumilio_plugin_api::PluginError> {
            panic!("no network")
        }
    }
    let objects = UtilityOverlay
        .objects(
            &NoIo,
            &OverlayRequest {
                context: request().context,
                overlay: "map-regions".into(),
                bounds: MapBounds {
                    min: MapPoint {
                        x: -513.,
                        z: -1025.,
                    },
                    max: MapPoint { x: -1., z: -1. },
                },
                level: 0,
            },
        )
        .unwrap();
    assert_eq!(objects.len(), 3);
    for object in objects {
        if let MapObjectKind::Polyline(line) = object.kind {
            let coordinate = if line[0].x == line[1].x {
                line[0].x
            } else {
                line[0].z
            };
            assert_eq!(coordinate % 512., 0.);
            assert!(coordinate < 0.);
        } else {
            panic!("expected lines");
        }
    }
}
