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
