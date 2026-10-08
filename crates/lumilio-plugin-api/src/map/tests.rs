use super::*;

fn round_trip<T: Serialize + for<'a> Deserialize<'a> + PartialEq + std::fmt::Debug>(value: T) {
    assert_eq!(
        serde_json::from_slice::<T>(&serde_json::to_vec(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn map_contracts_round_trip_without_launcher_types() {
    for world in [
        WorldId::Save {
            instance: "i".into(),
            folder: "w".into(),
        },
        WorldId::Server {
            instance: "i".into(),
            address: "localhost".into(),
        },
        WorldId::Seed {
            seed: i64::MIN,
            version: "1.21.4".into(),
        },
    ] {
        let context = WorldContext {
            world: world.clone(),
            version: Some("1.21.4".into()),
            data_version: Some(4189),
            seed: Some(i64::MIN),
            dimension: Dimension::Nether,
            sources: vec![SourceLink::Seed(SeedSource::Manual)],
        };
        round_trip(TileRequest {
            context: context.clone(),
            key: TileKey {
                provider: "plugin".into(),
                base_map: "seed".into(),
                world,
                dimension: Dimension::Nether,
                level: 4,
                tx: -1,
                tz: -2,
            },
            pixels: TILE_PIXELS,
        });
        round_trip(OverlayRequest {
            context,
            overlay: "points".into(),
            bounds: MapBounds {
                min: MapPoint { x: -1.5, z: -2.0 },
                max: MapPoint { x: 1.0, z: 2.0 },
            },
            level: 0,
        });
    }
    round_trip(crate::View::Map);
    round_trip(TileReply::Partial {
        image: ImageData {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        },
        coverage: vec![255],
    });
    assert!(
        !TileReply::Image(ImageData {
            width: 256,
            height: 256,
            rgba: vec![]
        })
        .is_valid()
    );
}
