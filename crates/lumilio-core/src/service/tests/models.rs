use std::io::Write;
use std::sync::Arc;

use lumilio_plugin_api::{API_VERSION, Manifest, Permission, Plugin, PluginError, Words};

use super::world;
use crate::error::ServiceError;
use crate::instances::Loader;
use crate::plugins::PluginHost;
use crate::schematic_assets::{AssetNotice, EntityGeometry, PlannedSource, SourceRole};
use crate::service::LauncherService;

struct Builder;

impl Plugin for Builder {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "test.builder".into(),
            name: Words::new("投影替身", "Schematic double"),
            description: Words::default(),
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

const HOUSE: &str = "schematics/house.litematic";

async fn world_with_schematic() -> (super::World, String) {
    let mut world = world();
    world.service.set_plugin_host(Arc::new(PluginHost::new(
        vec![Arc::new(Builder)],
        Default::default(),
    )));
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
        (
            "assets/minecraft/blockstates/stone.json",
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        ),
        ("assets/minecraft/models/block/stone.json", b"{}"),
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

/// A host that answers without the index (here a page that is not one, as
/// a 404 page would be) is not the network being down: the preview says the
/// server does not have the assets.
#[tokio::test(flavor = "multi_thread")]
async fn a_host_without_the_bundle_is_not_reported_as_offline() {
    let (world, id) = world_with_schematic().await;
    world
        .net
        .answer("block-assets/1/blocks.json", "<html>Not Found</html>");
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(preview.pack.is_none());
    assert_eq!(preview.plan.notice, Some(AssetNotice::Unavailable));
}

/// D23: an installed game's jar is never baked. Offline the preview says
/// so; online it is drawn with the bundle's pack.
#[tokio::test(flavor = "multi_thread")]
async fn an_installed_game_is_drawn_with_the_bundle_never_its_own_jar() {
    let (world, id) = world_with_schematic().await;
    install_game(&world, &id);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(preview.schematic, b"schematic bytes");
    assert!(preview.pack.is_none(), "the installed jar is not baked");
    assert_eq!(preview.plan.notice, Some(AssetNotice::Offline));

    publish_bundle_of(&world, "1.0", 100, &["stone", "dirt"], false);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(
        preview.plan.sources,
        [PlannedSource::Bundle {
            version: "1.0".into()
        }]
    );
    let pack = preview.pack.expect("the bundle has 1.0");
    assert!(
        pack.assets.blockstates.contains_key("minecraft:dirt"),
        "the bundle's pack: the installed jar has no dirt"
    );
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

/// A zip at `path` holding `entries`.
fn zip_at(path: &std::path::Path, entries: &[(String, &[u8])]) -> std::path::PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, bytes) in entries {
        writer
            .start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
    path.to_owned()
}

/// A zip at `path` with a blockstate and a model for each of `blocks`
/// (`namespace:name`).
fn blocks_at(path: &std::path::Path, blocks: &[&str]) -> std::path::PathBuf {
    let mut entries: Vec<(String, &[u8])> = Vec::new();
    for block in blocks {
        let (namespace, name) = block.split_once(':').unwrap();
        entries.push((
            format!("assets/{namespace}/blockstates/{name}.json"),
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        ));
        entries.push((
            format!("assets/{namespace}/models/block/{name}.json"),
            b"{}",
        ));
    }
    zip_at(path, &entries)
}

/// A published bundle holding one `1.0` pack with stone, served by `net`.
fn publish_bundle(world: &super::World) {
    publish_bundle_of(world, "1.0", 100, &["stone"], false);
}

/// A published bundle holding one pack of `version` with `blocks` and, when
/// `entities`, the fixture's entity geometry as that version.
fn publish_bundle_of(
    world: &super::World,
    version: &str,
    data_version: u32,
    blocks: &[&str],
    entities: bool,
) {
    use lumilio_block_assets::{AssetSource, EntityDump, EntitySource, write_entities};
    let dir = tempfile::tempdir().unwrap();
    let vanilla: Vec<String> = blocks
        .iter()
        .map(|block| format!("minecraft:{block}"))
        .collect();
    let vanilla: Vec<&str> = vanilla.iter().map(String::as_str).collect();
    let jar = blocks_at(&dir.path().join("client.jar"), &vanilla);
    let pack = lumilio_block_assets::bake(&[AssetSource::Archive(jar)]).unwrap();
    let bundle = lumilio_block_assets::write_bundle([(version, &pack)]).unwrap();
    let mut index = format!(
        r#"{{"format":1,"file":"blocks.dat.z","size":{},"sha256":"{}","versions":[{{"id":"{version}","data_version":{data_version}}}]"#,
        bundle.len(),
        sha256_of(&bundle)
    );
    if entities {
        let dump = EntityDump::from_json(ENTITY_FIXTURE).unwrap();
        let file = write_entities([(version, EntitySource::Dumped(&dump))]).unwrap();
        index.push_str(&format!(
            r#","entities":{{"format":1,"file":"entities.dat.z","size":{},"sha256":"{}","layers":4,"versions":[{{"id":"{version}"}}]}}"#,
            file.len(),
            sha256_of(&file)
        ));
        world.net.answer("block-assets/1/entities.dat.z", file);
    }
    index.push('}');
    world.net.answer("block-assets/1/blocks.json", index);
    world.net.answer("block-assets/1/blocks.dat.z", bundle);
}

#[tokio::test]
async fn a_game_that_is_not_installed_previews_with_the_bundle_fetched_as_a_task() {
    let (world, id) = world_with_schematic().await;
    publish_bundle(&world);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    let pack = preview.pack.expect("the bundle has 1.0");
    assert!(pack.assets.blockstates.contains_key("minecraft:stone"));
    assert_eq!(
        preview.plan.sources,
        [crate::schematic_assets::PlannedSource::Bundle {
            version: "1.0".into()
        }]
    );
    let downloads = |world: &super::World| {
        world
            .service
            .activity(10)
            .finished
            .iter()
            .filter(|task| {
                task.label
                    == crate::activity::TaskLabel::Typed {
                        action: crate::activity::TaskAction::PrepareBlockAssets,
                        subject: "1.0".into(),
                    }
                    && task.outcome == crate::activity::TaskOutcome::Succeeded
            })
            .count()
    };
    assert_eq!(downloads(&world), 1);

    // Kept: the next preview needs no network and starts no task.
    world.net.forget_all();
    let again = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    assert!(again.pack.is_some());
    assert_eq!(downloads(&world), 1);
}

#[tokio::test]
async fn out_of_reach_the_preview_says_so_calmly() {
    let (world, id) = world_with_schematic().await;
    let preview = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    assert!(preview.pack.is_none());
    assert!(preview.plan.sources.is_empty());
    assert_eq!(preview.plan.notice, Some(AssetNotice::Offline));

    publish_bundle(&world);
    world
        .service
        .set_auto_download_block_assets(false)
        .await
        .unwrap();
    let preview = world
        .service
        .plugin_model(&id, "test.builder", "schematics/house.litematic")
        .await
        .unwrap();
    assert!(preview.pack.is_none());
    assert_eq!(preview.plan.notice, Some(AssetNotice::DownloadDisabled));
}

/// The dump `tools/entity-dump` wrote for four 26.3 layers.
const ENTITY_FIXTURE: &[u8] =
    include_bytes!("../../../../lumilio-block-assets/src/entity/fixture_26_3.json");

/// An installed 26.3: its version json asks for Java 25 and one library,
/// and its jar is unobfuscated, all a runtime dump once needed.
async fn install_26_3(world: &super::World) -> String {
    let record = world
        .service
        .create_instance("Newest", Some("26.3"), Loader::Vanilla, None)
        .await
        .unwrap();
    let game = world.service.layout.game(&record.id);
    std::fs::create_dir_all(game.join("schematics")).unwrap();
    std::fs::write(game.join(HOUSE), b"schematic bytes").unwrap();
    let directories = world.service.layout.launch_directories(&record);
    let library = directories.libraries().join("org/example/lib/1/lib-1.jar");
    std::fs::create_dir_all(library.parent().unwrap()).unwrap();
    std::fs::write(&library, b"library").unwrap();
    let dir = directories.versions().join("26.3");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("26.3.json"),
        r#"{"id":"26.3","mainClass":"net.minecraft.client.main.Main",
            "javaVersion":{"component":"java-runtime-epsilon","majorVersion":25},
            "arguments":{"game":["--username","${auth_player_name}"],"jvm":["-cp","${classpath}"]},
            "libraries":[{"name":"org.example:lib:1","downloads":{"artifact":{
              "path":"org/example/lib/1/lib-1.jar","url":"https://example.invalid/lib-1.jar",
              "sha1":"0000000000000000000000000000000000000000","size":7}}}]}"#,
    )
    .unwrap();
    zip_at(
        &dir.join("26.3.jar"),
        &[
            (
                lumilio_block_assets::LAYER_DEFINITIONS_CLASS.to_owned(),
                b"code".as_slice(),
            ),
            (
                "assets/minecraft/blockstates/stone.json".to_owned(),
                br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
            ),
            ("assets/minecraft/models/block/stone.json".to_owned(), b"{}"),
        ],
    );
    record.id
}

/// A Java 25 whose `java` notes each start in the returned file.
#[cfg(unix)]
fn java_25(world: &super::World) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let jdk = world.service.layout().runtimes().join("jdk-25");
    std::fs::create_dir_all(jdk.join("bin")).unwrap();
    std::fs::write(jdk.join("release"), "JAVA_VERSION=\"25.0.1\"\n").unwrap();
    let runs = jdk.join("runs");
    let java = jdk.join("bin/java");
    std::fs::write(
        &java,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", runs.display()),
    )
    .unwrap();
    std::fs::set_permissions(&java, std::fs::Permissions::from_mode(0o755)).unwrap();
    runs
}

fn fixture_layers() -> lumilio_block_assets::EntityLayers {
    lumilio_block_assets::EntityDump::from_json(ENTITY_FIXTURE)
        .unwrap()
        .into_layers()
}

fn with_builder(world: &mut super::World) {
    world.service.set_plugin_host(Arc::new(PluginHost::new(
        vec![Arc::new(Builder)],
        Default::default(),
    )));
}

/// D23: a 26.x client installed here, with a Java that could dump it, is
/// drawn with the bundle's 26.x pack and entity geometry; no Java starts for
/// a preview. Out of reach it is a notice, not a bake of the jar.
#[cfg(unix)]
#[tokio::test(flavor = "multi_thread")]
async fn an_installed_26_x_client_takes_the_bundle_and_is_never_dumped() {
    let mut world = world();
    with_builder(&mut world);
    let id = install_26_3(&world).await;
    let runs = java_25(&world);
    publish_bundle_of(&world, "26.3", 5023, &["stone", "dirt"], true);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(
        preview.plan.sources,
        [PlannedSource::Bundle {
            version: "26.3".into()
        }]
    );
    let pack = preview.pack.expect("the bundle has 26.3");
    assert!(
        pack.assets.blockstates.contains_key("minecraft:dirt"),
        "the bundle's pack: the installed jar has no dirt"
    );
    assert_eq!(
        preview.plan.entities,
        EntityGeometry::Bundle {
            version: "26.3".into()
        }
    );
    assert_eq!(preview.entities, fixture_layers());
    assert!(!runs.exists(), "no Java was started");

    let mut world = super::world();
    with_builder(&mut world);
    let id = install_26_3(&world).await;
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(preview.pack.is_none());
    assert_eq!(preview.plan.notice, Some(AssetNotice::Offline));
    assert_eq!(preview.plan.entities, EntityGeometry::None);
    assert!(preview.entities.is_empty());
}

/// T21: an instance's mod is listed but off until switched on for the
/// studio folder. The switch is kept in launcher data, so a launcher started
/// again (and any studio window opened again) finds it on.
#[tokio::test(flavor = "multi_thread")]
async fn a_mod_is_off_until_switched_on_and_stays_on_after_reopening() {
    let (world, id) = world_with_schematic().await;
    publish_bundle(&world);
    let gems = blocks_at(
        &world.service.layout.game(&id).join("mods/gems.jar"),
        &["gems:ruby_ore"],
    );
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(preview.plan.overlays().is_empty(), "off by default");
    let pack = preview.pack.as_ref().unwrap();
    assert!(!pack.assets.blockstates.contains_key("gems:ruby_ore"));
    let row = preview
        .plan
        .provided
        .iter()
        .find(|source| source.role == SourceRole::Mod(gems.clone()))
        .expect("listed while off");
    assert_eq!((row.blocks, row.enabled), (1, false));

    world
        .service
        .schematic_source_set(&id, "schematics", gems.clone(), true)
        .await
        .unwrap();
    let on = |preview: &crate::worlds::ModelPreview| {
        preview
            .pack
            .as_ref()
            .is_some_and(|pack| pack.assets.blockstates.contains_key("gems:ruby_ore"))
    };
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(preview.plan.overlays(), [gems.as_path()]);
    assert!(on(&preview));

    // The launcher started again over the same data: the first lets go of
    // the data folder before the second opens it.
    let super::World {
        _dir,
        service,
        net,
        server,
        secrets,
    } = world;
    let root = service.layout().root().to_path_buf();
    drop(service);
    let mut world = super::World {
        _dir,
        service: LauncherService::open(&root, net.clone(), Vec::new()).unwrap(),
        net,
        server,
        secrets,
    };
    with_builder(&mut world);
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(on(&preview), "the switch was kept");

    world
        .service
        .schematic_source_set(&id, "schematics", gems, false)
        .await
        .unwrap();
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(!on(&preview));
}

/// A pack added in the studio is on in the folder it was added from; taken
/// off again, it is off everywhere.
#[tokio::test(flavor = "multi_thread")]
async fn an_added_pack_is_on_where_it_was_added_and_off_once_taken_off() {
    let (world, id) = world_with_schematic().await;
    publish_bundle(&world);
    let pack = blocks_at(
        &world.service.layout().root().join("elsewhere/Granite.zip"),
        &["minecraft:granite"],
    );
    world
        .service
        .schematic_pack_add(&id, "schematics", pack.clone())
        .await
        .unwrap();
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(preview.plan.overlays(), [pack.as_path()]);
    world
        .service
        .schematic_pack_remove(&id, pack.clone())
        .await
        .unwrap();
    assert!(
        crate::schematic_assets::switched_on(&world.service.layout.schematic_sources(&id), HOUSE)
            .is_empty()
    );
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert!(preview.plan.overlays().is_empty());
}

/// A backward conversion is checked against the target's bundle slice
/// (plan schematic-studio, T14); a version the bundle lacks is an error.
#[tokio::test]
async fn schematic_catalogs_come_from_the_bundle_slice_of_the_target() {
    let world = world();
    publish_bundle(&world);
    let (target, source) = world.service.schematic_catalogs("1.0", 4790).await.unwrap();
    assert!(target.has_block("minecraft:stone"));
    assert!(!target.has_block("minecraft:cherry_log"));
    assert!(source.is_none(), "the nearest version is the target itself");
    assert!(world.service.schematic_catalogs("9.9", 4790).await.is_err());
}

fn sha256_of(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn stone_names() -> lumilio_block_assets::BlockNames {
    let mut names = lumilio_block_assets::BlockNames::default();
    names.add(
        "zh_cn",
        [("block.minecraft.stone".to_owned(), "石头".to_owned())].into(),
    );
    names
}

/// The game's names come from the bundle's `names.dat.z`, fetched without
/// an Activity task and kept (plan schematic-studio, T26).
#[tokio::test]
async fn names_come_from_the_bundle_quietly_and_are_kept() {
    let (world, id) = world_with_schematic().await;
    let names = stone_names().to_bytes().unwrap();
    let index = format!(
        r#"{{"format":1,"file":"blocks.dat.z","size":1,"sha256":"{}","versions":[{{"id":"1.0","data_version":100}}],"names":{{"file":"names.dat.z","size":{},"sha256":"{}"}}}}"#,
        sha256_of(b"x"),
        names.len(),
        sha256_of(&names)
    );
    world.net.answer("block-assets/1/blocks.json", index);
    world.net.answer("block-assets/1/names.dat.z", names);
    let got = world.service.schematic_names(&id).await.unwrap();
    assert_eq!(got.block("minecraft:stone", "zh_cn"), Some("石头"));
    assert!(world.service.activity(10).finished.is_empty());

    world.net.forget_all();
    let again = world.service.schematic_names(&id).await.unwrap();
    assert_eq!(again.block("minecraft:stone", "zh_cn"), Some("石头"));
}

/// A names file that is not the one its index names is not used.
#[tokio::test]
async fn a_names_file_that_fails_its_check_is_not_kept() {
    let (world, id) = world_with_schematic().await;
    let index = format!(
        r#"{{"format":1,"file":"blocks.dat.z","size":1,"sha256":"{}","versions":[{{"id":"1.0","data_version":100}}],"names":{{"file":"names.dat.z","size":3,"sha256":"{}"}}}}"#,
        sha256_of(b"x"),
        sha256_of(b"abc")
    );
    world.net.answer("block-assets/1/blocks.json", index);
    world.net.answer(
        "block-assets/1/names.dat.z",
        stone_names().to_bytes().unwrap(),
    );
    let got = world.service.schematic_names(&id).await.unwrap();
    assert!(got.is_empty());
    let kept = world
        .service
        .layout
        .model_cache()
        .join("bundle/1/names.dat.z");
    assert!(!kept.exists());
}

/// Without the bundle, the names come from the installed game: English
/// from its jar, Chinese from the asset objects its index names.
#[tokio::test(flavor = "multi_thread")]
async fn without_the_bundle_names_come_from_the_installed_game() {
    let (world, id) = world_with_schematic().await;
    let record = futures_block(world.service.instance(&id)).unwrap();
    let directories = world.service.layout.launch_directories(&record);
    let dir = directories.versions().join("1.0");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("1.0.json"),
        r#"{"id":"1.0","assetIndex":{"id":"7","url":"https://example.invalid/7.json"}}"#,
    )
    .unwrap();
    let mut jar = zip::ZipWriter::new(std::fs::File::create(dir.join("1.0.jar")).unwrap());
    jar.start_file(
        "assets/minecraft/lang/en_us.json",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    jar.write_all(br#"{"block.minecraft.stone":"Stone","gui.done":"Done"}"#)
        .unwrap();
    jar.finish().unwrap();
    let chinese = r#"{"block.minecraft.stone":"石头"}"#;
    let hash = "3f786850e387550fdab836ed7e6dc881de23001b";
    let assets = directories.assets();
    std::fs::create_dir_all(assets.join("indexes")).unwrap();
    std::fs::write(
        assets.join("indexes/7.json"),
        format!(r#"{{"objects":{{"minecraft/lang/zh_cn.json":{{"hash":"{hash}","size":1}}}}}}"#),
    )
    .unwrap();
    std::fs::create_dir_all(assets.join("objects/3f")).unwrap();
    std::fs::write(assets.join("objects/3f").join(hash), chinese).unwrap();

    let names = world.service.schematic_names(&id).await.unwrap();
    assert_eq!(names.block("minecraft:stone", "en_us"), Some("Stone"));
    assert_eq!(names.block("minecraft:stone", "zh_cn"), Some("石头"));
    assert_eq!(names.len("en_us"), 1);
}

/// A drawing step that counts its calls and answers `png`, after checking
/// it is handed the file's preview.
fn drawing(
    drawn: &Arc<std::sync::atomic::AtomicUsize>,
    png: &'static [u8],
) -> impl FnOnce(crate::worlds::ModelPreview) -> Result<Vec<u8>, String> + Send + 'static {
    let drawn = drawn.clone();
    move |preview| {
        drawn.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(preview.schematic, b"schematic bytes");
        Ok(png.to_vec())
    }
}

/// A thumbnail is drawn once under its content key and read back after; a
/// failed drawing keeps nothing; a key that is not plain hex reaches no
/// file and draws nothing (T28).
#[tokio::test(flavor = "multi_thread")]
async fn thumbnails_are_drawn_once_by_their_key() {
    use std::sync::atomic::Ordering::SeqCst;
    let (world, id) = world_with_schematic().await;
    let (service, key) = (&world.service, "ab".repeat(32));
    let drawn = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let thumbnail = |key: String, png: &'static [u8]| {
        let draw = drawing(&drawn, png);
        let id = id.clone();
        async move {
            service
                .schematic_thumbnail(&id, "test.builder", HOUSE, &key, draw)
                .await
        }
    };
    assert_eq!(thumbnail(key.clone(), b"one").await.unwrap(), b"one");
    assert_eq!(
        thumbnail(key.clone(), b"two").await.unwrap(),
        b"one",
        "kept"
    );
    assert_eq!(drawn.load(SeqCst), 1);

    let other = "ef".repeat(32);
    let failed = service
        .schematic_thumbnail(&id, "test.builder", HOUSE, &other, |_| {
            Err("no block assets".to_owned())
        })
        .await;
    assert!(failed.is_err());
    assert_eq!(
        thumbnail(other, b"three").await.unwrap(),
        b"three",
        "drawn again"
    );

    for bad in ["../../settings", "", "ab/cd"] {
        assert!(thumbnail(bad.to_owned(), b"x").await.is_err());
    }
    assert_eq!(drawn.load(SeqCst), 2, "a bad key draws nothing");
}

/// T21: a thumbnail is kept per look of the sources switched on for its
/// folder, and drawn with exactly those. Switching a mod on draws the file
/// again with it; switching it off finds the picture drawn without it; a
/// file in a folder whose switches did not change keeps its picture.
#[tokio::test(flavor = "multi_thread")]
async fn a_switched_source_gives_thumbnails_a_new_look() {
    use std::sync::atomic::Ordering::SeqCst;
    let (world, id) = world_with_schematic().await;
    publish_bundle(&world);
    let gems = blocks_at(
        &world.service.layout.game(&id).join("mods/gems.jar"),
        &["gems:ruby_ore"],
    );
    let game = world.service.layout.game(&id);
    std::fs::create_dir_all(game.join("schematics/farms")).unwrap();
    std::fs::write(
        game.join("schematics/farms/a.litematic"),
        b"schematic bytes",
    )
    .unwrap();
    let (service, key) = (&world.service, "cd".repeat(32));
    let farm = "schematics/farms/a.litematic";
    let drawn = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let thumbnail = |file: &'static str, png: &'static [u8]| {
        let draw = drawing(&drawn, png);
        let (id, key) = (id.clone(), key.clone());
        async move {
            service
                .schematic_thumbnail(&id, "test.builder", file, &key, draw)
                .await
                .unwrap()
        }
    };
    assert_eq!(thumbnail(HOUSE, b"plain").await, b"plain");
    assert_eq!(thumbnail(farm, b"plain").await, b"plain");
    service
        .schematic_source_set(&id, "schematics/farms", gems.clone(), true)
        .await
        .unwrap();
    assert_eq!(
        thumbnail(HOUSE, b"gems").await,
        b"plain",
        "this folder's switches did not change"
    );
    let with_gems = service
        .schematic_thumbnail(&id, "test.builder", farm, &key, |preview| {
            let pack = preview.pack.expect("the bundle");
            assert!(
                pack.assets.blockstates.contains_key("gems:ruby_ore"),
                "drawn with the switched-on mod"
            );
            Ok(b"gems".to_vec())
        })
        .await
        .unwrap();
    assert_eq!(with_gems, b"gems", "drawn again with the mod");
    service
        .schematic_source_set(&id, "schematics/farms", gems, false)
        .await
        .unwrap();
    assert_eq!(
        thumbnail(farm, b"again").await,
        b"plain",
        "without the mod, the first picture again"
    );
    assert_eq!(
        drawn.load(SeqCst),
        1,
        "the same blocks under the same look are one picture"
    );
}

/// With neither, there are no names and no error: the studio writes IDs.
#[tokio::test]
async fn without_bundle_or_game_there_are_no_names() {
    let (world, id) = world_with_schematic().await;
    let names = world.service.schematic_names(&id).await.unwrap();
    assert!(names.is_empty());
}

/// T36: moving a pack past another changes the order the preview layers
/// them in, and the order is kept; a pack added later goes on top.
#[tokio::test(flavor = "multi_thread")]
async fn moving_a_pack_reorders_the_stack_and_is_kept() {
    let (world, id) = world_with_schematic().await;
    publish_bundle(&world);
    let elsewhere = world.service.layout().root().join("elsewhere");
    let granite = blocks_at(&elsewhere.join("Granite.zip"), &["minecraft:granite"]);
    let redstone = blocks_at(
        &elsewhere.join("Redstone.zip"),
        &["minecraft:redstone_wire"],
    );
    for pack in [&granite, &redstone] {
        world
            .service
            .schematic_pack_add(&id, "schematics", pack.clone())
            .await
            .unwrap();
    }
    let overlays = |preview: &crate::worlds::ModelPreview| {
        preview
            .plan
            .overlays()
            .into_iter()
            .map(std::path::Path::to_path_buf)
            .collect::<Vec<_>>()
    };
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(overlays(&preview), [granite.clone(), redstone.clone()]);

    assert!(
        world
            .service
            .schematic_source_move(&id, granite.clone(), redstone.clone())
            .await
            .unwrap()
    );
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(overlays(&preview), [redstone.clone(), granite.clone()]);

    let third = blocks_at(&elsewhere.join("Third.zip"), &["minecraft:stone"]);
    world
        .service
        .schematic_pack_add(&id, "schematics", third.clone())
        .await
        .unwrap();
    let preview = world
        .service
        .plugin_model(&id, "test.builder", HOUSE)
        .await
        .unwrap();
    assert_eq!(overlays(&preview), [redstone, granite, third]);
}
