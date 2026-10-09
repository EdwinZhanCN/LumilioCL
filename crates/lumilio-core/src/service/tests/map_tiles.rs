//! Base maps drawn from a world's files are kept per instance and rebuilt
//! only where a file changed (plan W6, T30).
use super::Scripted;
use crate::LauncherService;
use crate::instance::Loader;
use lumilio_plugin_api::map::{
    BaseMapInfo, BaseMapProvider, Dimension, TileKey, TileReply, TileRequest, WorldContext, WorldId,
};
use lumilio_plugin_api::{
    API_VERSION, HostContext, ImageData, MAX_PAGE, Manifest, Permission, Plugin, PluginError, Words,
};
use std::sync::{Arc, Mutex};

/// A provider whose level-0 tile `(tx, tz)` is drawn from region
/// `r.<tx div 2>.<tz div 2>.mca`, and which records every tile it draws.
#[derive(Default)]
struct Regions {
    drawn: Mutex<Vec<(i32, i32)>>,
}

const DIR: &str = "saves/World/region";

impl Plugin for Regions {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.regions".into(),
            name: Words::new("R", "R"),
            description: Words::default(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![Permission::ReadGameFiles {
                under: "saves".into(),
            }],
            settings: vec![],
        }
    }
    fn base_map_provider(&self) -> Option<&dyn BaseMapProvider> {
        Some(self)
    }
}

impl BaseMapProvider for Regions {
    fn base_maps(&self) -> Vec<BaseMapInfo> {
        vec![BaseMapInfo {
            id: "files".into(),
            kind_id: "map-base-save".into(),
            dimensions: vec![Dimension::Overworld],
            levels: vec![0],
        }]
    }
    fn sources(
        &self,
        ctx: &dyn HostContext,
        request: &TileRequest,
    ) -> Result<Option<Vec<String>>, PluginError> {
        let key = &request.key;
        if key.level == 0 {
            return Ok(Some(vec![format!(
                "{DIR}/r.{}.{}.mca",
                key.tx.div_euclid(2),
                key.tz.div_euclid(2)
            )]));
        }
        // Regions per side of a tile: 2 at level 1, 8 at level 2, …
        let side = 2 * 4_i32.pow(u32::from(key.level) - 1);
        let page = ctx.list_dir(DIR, None, MAX_PAGE)?;
        Ok(Some(
            page.entries
                .into_iter()
                .filter(|entry| {
                    let mut parts = entry.name.split('.').skip(1);
                    let (Some(Ok(x)), Some(Ok(z))) = (
                        parts.next().map(str::parse::<i32>),
                        parts.next().map(str::parse::<i32>),
                    ) else {
                        return false;
                    };
                    x.div_euclid(side) == key.tx && z.div_euclid(side) == key.tz
                })
                .map(|entry| format!("{DIR}/{}", entry.name))
                .collect(),
        ))
    }
    fn tile(&self, _: &dyn HostContext, request: &TileRequest) -> Result<TileReply, PluginError> {
        assert_eq!(request.key.level, 0, "the host builds coarser levels");
        self.drawn
            .lock()
            .unwrap()
            .push((request.key.tx, request.key.tz));
        Ok(TileReply::Image(ImageData {
            width: 256,
            height: 256,
            rgba: [90, 120, 60, 255].repeat(256 * 256),
        }))
    }
}

fn request(instance: &str, level: u8, tx: i32, tz: i32) -> TileRequest {
    let world = WorldId::Save {
        instance: instance.into(),
        folder: "World".into(),
    };
    TileRequest {
        context: WorldContext {
            world: world.clone(),
            version: None,
            data_version: None,
            seed: None,
            dimension: Dimension::Overworld,
            sources: vec![],
        },
        key: TileKey {
            provider: "test.regions".into(),
            base_map: "files".into(),
            world,
            dimension: Dimension::Overworld,
            level,
            tx,
            tz,
        },
        pixels: 256,
    }
}

fn drawn(plugin: &Regions) -> Vec<(i32, i32)> {
    let mut drawn = std::mem::take(&mut *plugin.drawn.lock().unwrap());
    drawn.sort();
    drawn
}

#[tokio::test]
async fn a_changed_region_rebuilds_only_its_tiles_and_their_parents() {
    let dir = tempfile::tempdir().unwrap();
    let plugin = Arc::new(Regions::default());
    let service = LauncherService::open(
        dir.path().join("launcher"),
        Scripted::default(),
        vec![plugin.clone() as Arc<dyn Plugin>],
    )
    .unwrap();
    let id = service
        .create_instance("Map", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap()
        .id;
    let regions = service.layout.game(&id).join(DIR);
    std::fs::create_dir_all(&regions).unwrap();
    for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1), (5, 5)] {
        std::fs::write(regions.join(format!("r.{x}.{z}.mca")), b"region").unwrap();
    }
    let tile = |level, tx, tz| {
        let (service, id) = (&service, &id);
        async move {
            service
                .map_tile(
                    id,
                    request(id, level, tx, tz),
                    crate::CancellationToken::new(),
                )
                .await
                .unwrap()
        }
    };

    // Level 1 tile (0, 0) covers regions 0–1 × 0–1: all 16 level-0 tiles drawn.
    assert!(matches!(tile(1, 0, 0).await, TileReply::Image(_)));
    let first = drawn(&plugin);
    assert_eq!(first.len(), 16);
    assert!(
        first
            .iter()
            .all(|(x, z)| (0..4).contains(x) && (0..4).contains(z))
    );
    // Level 2 above it covers regions 0–7: the kept level-1 tile, region
    // (5, 5) drawn for the first time, and empty siblings.
    assert!(matches!(tile(2, 0, 0).await, TileReply::Partial { .. }));
    assert_eq!(drawn(&plugin), [(10, 10), (10, 11), (11, 10), (11, 11)]);
    assert!(matches!(tile(1, 0, 0).await, TileReply::Image(_)));
    assert_eq!(drawn(&plugin), [], "unchanged files: kept");
    // Nothing there: answered without asking the provider to draw.
    assert_eq!(tile(0, 40, 40).await, TileReply::Empty);
    assert_eq!(tile(1, 7, 7).await, TileReply::Empty);
    assert_eq!(drawn(&plugin), []);

    // The game writes region (1, 0): only its four tiles are drawn again, for
    // its parent at level 1 and that parent's parent at level 2.
    std::fs::write(regions.join("r.1.0.mca"), b"region, grown").unwrap();
    assert!(matches!(tile(2, 0, 0).await, TileReply::Partial { .. }));
    assert_eq!(drawn(&plugin), [(2, 0), (2, 1), (3, 0), (3, 1)]);
    assert!(matches!(tile(1, 0, 0).await, TileReply::Image(_)));
    assert!(matches!(tile(0, 3, 1).await, TileReply::Image(_)));
    assert_eq!(drawn(&plugin), [], "the rebuilt tiles are kept again");
    // Region (5, 5), which the change did not touch, is still kept.
    tile(0, 10, 10).await;
    tile(1, 2, 2).await;
    assert_eq!(drawn(&plugin), []);

    // A region that appears where there was none counts as a change too.
    std::fs::write(regions.join("r.2.0.mca"), b"new").unwrap();
    assert!(matches!(tile(1, 1, 0).await, TileReply::Partial { .. }));
    assert_eq!(drawn(&plugin), [(4, 0), (4, 1), (5, 0), (5, 1)]);

    // Clearing the map cache forgets the save tiles as well.
    service.clear_map_cache().await.unwrap();
    tile(1, 0, 0).await;
    assert_eq!(drawn(&plugin).len(), 16);
}
