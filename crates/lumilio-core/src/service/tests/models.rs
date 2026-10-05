use super::world;
use crate::instance::Loader;
use crate::plugins::PluginHost;
use crate::service::ServiceError;
use lumilio_plugin_api::{API_VERSION, Manifest, Permission, Plugin, PluginError};
use std::io::Write;
use std::sync::Arc;

struct Builder;

impl Plugin for Builder {
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
    let mut world = world();
    world.service.plugins = PluginHost::new(vec![Arc::new(Builder)], Default::default());
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
