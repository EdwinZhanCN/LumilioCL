use super::*;
use lumilio_plugin_api::map::{TileKey, WorldContext};

fn request(tx: i32, tz: i32) -> TileRequest {
    let world = WorldId::Save {
        instance: "test".into(),
        folder: "World".into(),
    };
    TileRequest {
        context: WorldContext {
            world: world.clone(),
            version: Some("1.21.4".into()),
            data_version: None,
            seed: None,
            dimension: Dimension::Overworld,
            sources: vec![SourceLink::XaeroWorldMap("World".into())],
        },
        key: TileKey {
            provider: crate::ID.into(),
            base_map: BASE.into(),
            world,
            dimension: Dimension::Overworld,
            level: 0,
            tx,
            tz,
        },
        pixels: TILE_PIXELS,
    }
}

#[test]
fn legacy_region_renders_covered_pixels_and_leaves_unexplored_pixels_empty() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../vendor/xaerotools/crates/xaero-core/tests/fixtures/legacy/v0.4_-1000_998.zip"
    );
    let bytes = std::fs::read(fixture).unwrap();
    let stream = xaero_core::read_region_container(&bytes).unwrap();
    let region = xaero_core::decode_region(&stream).unwrap();
    assert_eq!((region.version.major, region.version.minor), (0, 4));
    assert!(!region.truncated);
    let mut seen = 0usize;
    let mut empty = 0usize;
    for tz in 1996..1998 {
        for tx in -2000..-1998 {
            match render_quarter(&region, &request(tx, tz)) {
                TileReply::Partial {
                    image, coverage, ..
                } => {
                    seen += coverage.iter().filter(|&&value| value != 0).count();
                    empty += coverage.iter().filter(|&&value| value == 0).count();
                    assert_eq!(image.rgba.len(), (TILE_PIXELS * TILE_PIXELS * 4) as usize);
                }
                TileReply::Empty => empty += (TILE_PIXELS * TILE_PIXELS) as usize,
                other => panic!("unexpected tile: {other:?}"),
            }
        }
    }
    assert!(seen > 0);
    assert!(empty > 0);
}

#[test]
fn codec_rejects_a_future_region_version() {
    let stream = [0xff, 0, 8, 0, 0];
    assert!(matches!(
        xaero_core::decode_region(&stream),
        Err(xaero_core::codec::CodecError::UnsupportedVersion { .. })
    ));
}

#[test]
fn multiworld_ids_require_an_explicit_cache_key_and_reject_unlinked_ids() {
    let mut tile = request(0, 0);
    tile.context.sources = vec![
        SourceLink::XaeroWorldMap("World/null/mw$default".into()),
        SourceLink::XaeroWorldMap("World/null/mw$default_1".into()),
        SourceLink::XaeroWorldMap("World/DIM-1/mw$default".into()),
    ];
    assert!(
        matches!(region_dir(&tile), Err(PluginError::Unavailable(message)) if message == "map-xaero-choose-map")
    );
    tile.key.base_map = "xaero@World/null/mw$default_1".into();
    assert_eq!(
        region_dir(&tile).unwrap().as_deref(),
        Some("xaero/world-map/World/null/mw$default_1")
    );
    tile.key.base_map = "xaero@World/null/not-linked".into();
    assert!(matches!(
        region_dir(&tile),
        Err(PluginError::InvalidInput(_))
    ));
}

#[test]
#[ignore = "set LUMILIO_XAERO_REGION_SAMPLE to a local Xaero 7.8 region ZIP"]
fn local_modern_region_decodes_without_truncation() {
    let path = std::env::var("LUMILIO_XAERO_REGION_SAMPLE").expect("sample path");
    let bytes = std::fs::read(path).unwrap();
    let stream = xaero_core::read_region_container(&bytes).unwrap();
    let region = xaero_core::decode_region(&stream).unwrap();
    assert_eq!((region.version.major, region.version.minor), (7, 8));
    assert!(!region.truncated);
    assert_eq!(region.trailing, 0);
    assert!(!region.region.chunks.is_empty());
}
