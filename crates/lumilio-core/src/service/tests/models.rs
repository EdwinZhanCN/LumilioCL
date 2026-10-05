use super::world;
use crate::instance::Loader;
use crate::plugins::PluginHost;
use crate::service::ServiceError;
use lumilio_plugin_api::{
    API_VERSION, ActionId, Effect, GameFacts, HostContext, InstanceTab, Manifest, Permission,
    Plugin, PluginError, TabState, View,
};
use std::io::Write;
use std::sync::Arc;

/// A plugin whose `model` can be told to fail or to panic.
struct Builder {
    mode: Mode,
}

#[derive(Clone, Copy)]
enum Mode {
    Plain,
    Error,
    Panic,
}

impl InstanceTab for Builder {
    fn title(&self) -> String {
        "投影".into()
    }
    fn appears(&self, _: &GameFacts, _: &dyn HostContext) -> bool {
        true
    }
    fn view(&self, _: &dyn HostContext, _: &TabState) -> Result<View, PluginError> {
        Ok(View::Tags(Vec::new()))
    }
    fn update(
        &self,
        _: &dyn HostContext,
        state: TabState,
        _: ActionId,
    ) -> Result<(TabState, Vec<Effect>), PluginError> {
        Ok((state, Vec::new()))
    }
    fn model(&self, ctx: &dyn HostContext, file: &str) -> Result<Vec<u8>, PluginError> {
        match self.mode {
            Mode::Plain => ctx.read_file(file),
            Mode::Error => Err(PluginError::Unavailable("this file is broken".into())),
            Mode::Panic => panic!("boom"),
        }
    }
}

impl Plugin for Builder {
    fn instance_tab(&self) -> Option<&dyn InstanceTab> {
        Some(self)
    }
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.builder".into(),
            name: "投影替身".into(),
            description: String::new(),
            version: "1".into(),
            api: API_VERSION,
            default_enabled: true,
            permissions: vec![Permission::ReadGameFiles {
                under: "schematics".into(),
            }],
            settings: Vec::new(),
        }
    }
}

async fn world_with_schematic() -> (super::World, String) {
    world_with(Mode::Plain).await
}

async fn world_with(mode: Mode) -> (super::World, String) {
    let mut world = world();
    world.service.plugins = PluginHost::new(vec![Arc::new(Builder { mode })], Default::default());
    let record = world
        .service
        .create_instance("Builder", Some("1.0"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("schematics")).unwrap();
    std::fs::write(game.join("schematics/house.litematic"), b"schematic bytes").unwrap();
    std::fs::write(game.join("options.txt"), b"secret").unwrap();
    (world, record.id)
}

/// What an installed game leaves: the release json and the client jar.
fn install_game(world: &super::World, id: &str) {
    let record = futures_block(world.service.instance(id)).unwrap();
    let versions = world
        .service
        .layout
        .launch_directories(&record)
        .versions()
        .to_path_buf();
    let dir = versions.join("1.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("1.0.json"), r#"{"id":"1.0"}"#).unwrap();
    let mut jar = zip::ZipWriter::new(std::fs::File::create(dir.join("1.0.jar")).unwrap());
    for (name, bytes) in [
        ("net/minecraft/Main.class", b"code".as_slice()),
        ("assets/minecraft/blockstates/stone.json", b"{}"),
        ("assets/minecraft/textures/block/stone.png", b"png"),
    ] {
        jar.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        jar.write_all(bytes).unwrap();
    }
    jar.finish().unwrap();
}

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_preview_has_the_schematic_and_a_pack_from_the_games_own_jar() {
    let (world, id) = world_with_schematic().await;
    install_game(&world, &id);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    assert_eq!(preview.schematic, b"schematic bytes");
    let pack = preview.pack.expect("the jar is there");
    assert!(pack.id.starts_with("jar-1.0-"), "{}", pack.id);
    let archive = zip::ZipArchive::new(std::io::Cursor::new(pack.bytes)).unwrap();
    let mut names: Vec<_> = archive.file_names().map(str::to_owned).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "assets/minecraft/blockstates/stone.json",
            "assets/minecraft/textures/block/stone.png"
        ],
        "the code is not in the pack"
    );
}

#[tokio::test]
async fn a_game_that_is_not_installed_still_previews_without_a_pack() {
    let (world, id) = world_with_schematic().await;
    let preview = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    assert_eq!(preview.schematic, b"schematic bytes");
    assert!(preview.pack.is_none());
}

#[tokio::test]
async fn files_outside_the_grant_and_switched_off_plugins_are_refused() {
    let (world, id) = world_with_schematic().await;
    for file in ["options.txt", "schematics/../options.txt", "/etc/passwd"] {
        let result = world.service.plugin_model(&id, "test.builder", file).await;
        assert!(
            matches!(
                result,
                Err(ServiceError::Plugin(PluginError::PermissionDenied))
            ),
            "{file}: {result:?}"
        );
    }
    assert!(matches!(
        world
            .service
            .plugin_model(&id, "no.such", "schematics/house.litematic")
            .await,
        Err(ServiceError::Plugin(PluginError::InvalidInput(_)))
    ));
    world
        .service
        .set_plugin_enabled("test.builder", false)
        .await
        .unwrap();
    assert!(matches!(
        world
            .service
            .plugin_model(&id, "test.builder", "schematics/house.litematic")
            .await,
        Err(ServiceError::Plugin(PluginError::Unavailable(_)))
    ));
}

#[tokio::test]
async fn a_file_the_plugin_cannot_prepare_fails_that_preview_not_the_plugin() {
    let (world, id) = world_with(Mode::Error).await;
    let result = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await;
    assert!(
        matches!(
            result,
            Err(ServiceError::Plugin(PluginError::Unavailable(_)))
        ),
        "{result:?}"
    );
    // The plugin is still on: its tab shows and the next preview is asked for.
    assert_eq!(world.service.plugin_tabs(&id).await.unwrap().len(), 1);
    assert!(
        world
            .service
            .plugin_model(&id, "test.builder", "schematics/house.litematic")
            .await
            .is_err()
    );
    assert_eq!(world.service.plugin_tabs(&id).await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_plugin_that_panics_while_preparing_is_switched_off_for_the_run() {
    let (world, id) = world_with(Mode::Panic).await;
    assert!(
        world
            .service
            .plugin_model(&id, "test.builder", "schematics/house.litematic")
            .await
            .is_err()
    );
    assert!(world.service.plugin_tabs(&id).await.unwrap().is_empty());
}
