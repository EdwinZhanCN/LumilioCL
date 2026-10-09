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
fn retained_tiles_finish_once_and_departed_tiles_are_cancelled() {
    let key = request().key;
    let mut schedule = MapSchedule::default();
    let generation = schedule.update(std::slice::from_ref(&key));
    let (_, cancel) = schedule.begin(&key).unwrap();
    assert!(schedule.begin(&key).is_none());
    schedule.update(std::slice::from_ref(&key));
    assert!(!cancel.is_cancelled());
    assert!(schedule.begin(&key).is_none());
    assert!(schedule.accept(generation, &key));
    assert!(!schedule.accept(generation, &key));
    let (old_generation, cancel) = schedule.begin(&key).unwrap();
    let mut next = key.clone();
    next.tx += 1;
    schedule.update(std::slice::from_ref(&next));
    assert!(cancel.is_cancelled());
    assert!(!schedule.accept(old_generation, &key));
    schedule.update(std::slice::from_ref(&key));
    let (new_generation, _) = schedule.begin(&key).unwrap();
    assert!(!schedule.accept(old_generation, &key));
    assert!(schedule.accept(new_generation, &key));
}

#[test]
fn worker_cancellation_does_not_hide_current_tile_error() {
    let key = request().key;
    let mut schedule = MapSchedule::default();
    schedule.update(std::slice::from_ref(&key));
    let (generation, cancel) = schedule.begin(&key).unwrap();
    // The host cancels the worker after timeout or provider failure, before
    // delivering its error to the viewport. That error still needs rendering.
    cancel.cancel();
    assert!(schedule.accept(generation, &key));
    assert!(!schedule.accept(generation, &key));
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

mod xaero {
    use super::super::WorldMapContext;
    use super::super::xaero::{attach, minimap_dirs};
    use lumilio_plugin_api::map::{Dimension, SourceLink, WorldContext, WorldId};
    use std::collections::BTreeMap;

    fn save(folder: &str) -> WorldMapContext {
        WorldMapContext {
            name: folder.into(),
            spawn: None,
            suggested_xaero: None,
            context: WorldContext {
                world: WorldId::Save {
                    instance: "i".into(),
                    folder: folder.into(),
                },
                version: Some("1.21.4".into()),
                data_version: None,
                seed: Some(1),
                dimension: Dimension::Overworld,
                sources: vec![SourceLink::Save(folder.into())],
            },
        }
    }

    fn strings(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_same_named_directory_is_only_suggested_until_the_person_links_it() {
        let dirs = strings(&["Multiplayer_play.example.org", "Survival", "Unrelated"]);
        let mut worlds = vec![save("Survival"), save("Creative")];
        attach(&mut worlds, "i", &dirs, &BTreeMap::new());
        assert_eq!(worlds[0].suggested_xaero.as_deref(), Some("Survival"));
        assert_eq!(worlds[1].suggested_xaero, None);
        assert!(
            worlds[0]
                .context
                .sources
                .iter()
                .all(|source| !matches!(source, SourceLink::XaeroMinimap(_))),
            "a suggestion attaches nothing"
        );
        // Confirmed: the directory becomes a source and stops being a suggestion.
        let links = BTreeMap::from([("Creative".to_owned(), "Unrelated".to_owned())]);
        let mut worlds = vec![save("Survival"), save("Creative")];
        attach(&mut worlds, "i", &dirs, &links);
        assert!(
            worlds[1]
                .context
                .sources
                .contains(&SourceLink::XaeroMinimap("Unrelated".into()))
        );
        assert_eq!(worlds[1].suggested_xaero, None);
        assert_eq!(worlds[0].suggested_xaero.as_deref(), Some("Survival"));
        // A link to a directory that has since gone is ignored.
        let stale = BTreeMap::from([("Creative".to_owned(), "Deleted".to_owned())]);
        let mut worlds = vec![save("Creative")];
        attach(&mut worlds, "i", &dirs, &stale);
        assert!(worlds[0].context.sources.len() == 1);
    }

    #[test]
    fn multiplayer_directories_become_server_worlds() {
        let dirs = strings(&["Multiplayer_play.example.org", "Survival"]);
        let mut worlds = vec![save("Survival")];
        attach(&mut worlds, "i", &dirs, &BTreeMap::new());
        let server = worlds.last().unwrap();
        assert_eq!(server.name, "play.example.org");
        assert_eq!(
            server.context.world,
            WorldId::Server {
                instance: "i".into(),
                address: "play.example.org".into()
            }
        );
        assert_eq!(server.context.seed, None);
        assert_eq!(
            server.context.sources,
            [SourceLink::XaeroMinimap(
                "Multiplayer_play.example.org".into()
            )]
        );
        // A multiplayer directory linked to a save is that save, not a server.
        let links = BTreeMap::from([(
            "Survival".to_owned(),
            "Multiplayer_play.example.org".to_owned(),
        )]);
        let mut worlds = vec![save("Survival")];
        attach(&mut worlds, "i", &dirs, &links);
        assert_eq!(worlds.len(), 1);
    }

    #[test]
    fn minimap_directories_skip_files_backups_hidden_entries_and_links() {
        let game = tempfile::tempdir().unwrap();
        assert!(minimap_dirs(game.path()).is_empty());
        let root = game.path().join("xaero/minimap");
        for dir in ["Survival", "Multiplayer_a", "backup", "backup--", ".hidden"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        assert_eq!(minimap_dirs(game.path()), ["Multiplayer_a", "Survival"]);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("Looped")).unwrap();
            assert_eq!(minimap_dirs(game.path()), ["Multiplayer_a", "Survival"]);
        }
    }
}
