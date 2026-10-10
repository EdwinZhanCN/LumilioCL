use std::io::Write as _;

use super::super::error::ServiceError;
use super::{ECHO_ARGS, World, fake_java, publish_identity_release, world};
use crate::instance::Loader;
use crate::skin::{SkinChoice, SkinError, SkinModel};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

fn picture(color: [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(64, 64, image::Rgba(color));
    let mut out = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn jar() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut out);
        writer
            .start_file::<_, ()>(
                "META-INF/MANIFEST.MF",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        write!(
            writer,
            "Manifest-Version: 1.0\r\nImplementation-Title: authlib-injector\r\nImplementation-Version: 1.2.54\r\nBuild-Number: 54\r\n"
        )
        .unwrap();
        writer.finish().unwrap();
    }
    out.into_inner()
}

fn publish_injector(world: &World) {
    let bytes = jar();
    let sha: String = Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    world.net.answer(
        "authlib-injector.yushi.moe/artifact/latest.json",
        serde_json::json!({
            "build_number": 54, "version": "1.2.54",
            "download_url": "https://download.example/authlib-injector-1.2.54.jar",
            "checksums": {"sha256": sha},
        })
        .to_string(),
    );
    world
        .net
        .answer("download.example/authlib-injector-1.2.54.jar", bytes);
}

fn skin_world() -> World {
    let mut world = world();
    world.service.skin_key_bits = 1024;
    world
}

/// `Steve` (the world's offline account) wears LittleSkin.
fn publish_littleskin(world: &World) {
    world.net.answer(
        "littleskin.cn/csl/Steve.json",
        r#"{"username":"Steve","textures":{"slim":"abc123","cape":"cape99"}}"#,
    );
    world.net.answer(
        "littleskin.cn/csl/textures/abc123",
        picture([10, 20, 30, 255]),
    );
    world.net.answer(
        "littleskin.cn/csl/textures/cape99",
        picture([200, 0, 0, 255]),
    );
}

async fn get(port: u16, target: &str) -> serde_json::Value {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(format!("GET {target} HTTP/1.1\r\nHost: localhost\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    serde_json::from_slice(&raw[split + 4..]).unwrap_or(serde_json::Value::Null)
}

fn port_of(api_root: &str) -> u16 {
    api_root.rsplit(':').next().unwrap().parse().unwrap()
}

#[tokio::test]
async fn a_skin_is_chosen_checked_kept_and_cleared_for_offline_accounts_only() {
    let world = skin_world();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("me.png");
    std::fs::write(&file, picture([1, 2, 3, 255])).unwrap();
    let choice = SkinChoice::Local {
        model: SkinModel::Slim,
        skin: Some(file),
        cape: None,
    };
    world
        .service
        .set_account_skin("Steve", Some(choice.clone()))
        .await
        .unwrap();
    let settings = world.service.settings().await;
    assert_eq!(settings.accounts[0].skin, Some(choice));
    let saved =
        std::fs::read_to_string(world.service.layout().root().join("settings.json")).unwrap();
    assert!(saved.contains("\"local\""), "{saved}");

    // A file that is not a picture, and an address that is not one, never get in.
    let broken = dir.path().join("broken.png");
    std::fs::write(&broken, b"nope").unwrap();
    let bad = SkinChoice::Local {
        model: SkinModel::Wide,
        skin: Some(broken),
        cape: None,
    };
    assert!(matches!(
        world.service.set_account_skin("Steve", Some(bad)).await,
        Err(ServiceError::Skin(SkinError::Picture(_)))
    ));
    assert!(matches!(
        world
            .service
            .set_account_skin(
                "Steve",
                Some(SkinChoice::Csl {
                    api: "ftp://x".into()
                })
            )
            .await,
        Err(ServiceError::Skin(SkinError::InvalidApi(_)))
    ));
    assert!(world.service.settings().await.accounts[0].skin.is_some());

    // Not for accounts that are not there (or are not offline).
    assert!(
        world
            .service
            .set_account_skin("Nobody", Some(SkinChoice::LittleSkin))
            .await
            .is_err()
    );
    world.service.set_account_skin("Steve", None).await.unwrap();
    assert_eq!(world.service.settings().await.accounts[0].skin, None);
}

#[tokio::test]
async fn an_account_face_is_the_head_of_its_skin_and_none_without_one() {
    let world = skin_world();
    // No chosen skin and no installed client to fall back to: no face.
    assert!(world.service.account_face("Steve").await.unwrap().is_none());

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("me.png");
    std::fs::write(&file, picture([1, 2, 3, 255])).unwrap();
    world
        .service
        .set_account_skin(
            "Steve",
            Some(SkinChoice::Local {
                model: SkinModel::Wide,
                skin: Some(file),
                cape: None,
            }),
        )
        .await
        .unwrap();
    let face = world
        .service
        .account_face("Steve")
        .await
        .unwrap()
        .expect("a face");
    assert_eq!((face.width, face.height), (64, 64));
    assert_eq!(face.rgba.len(), 64 * 64 * 4);
}

#[tokio::test]
async fn an_offline_player_without_a_skin_needs_no_agent_and_no_download() {
    let world = skin_world();
    let settings = world.service.settings().await;
    let session = world.service.session_for(&settings).await.unwrap();
    assert!(session.injection().is_none());
    assert!(session.jvm_arguments().is_empty());
    assert!(world.net.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn a_littleskin_player_gets_a_local_server_that_serves_their_skin_for_as_long_as_the_session_lives()
 {
    let world = skin_world();
    publish_injector(&world);
    publish_littleskin(&world);
    world
        .service
        .set_account_skin("Steve", Some(SkinChoice::LittleSkin))
        .await
        .unwrap();

    let settings = world.service.settings().await;
    let session = world.service.session_for(&settings).await.unwrap();
    let injection = session.injection().expect("the agent is loaded");
    assert!(injection.api_root.starts_with("http://localhost:"));
    assert!(injection.prefetched.is_none());
    assert_eq!(
        session.jvm_arguments().len(),
        2,
        "no prefetched metadata for the local server"
    );
    assert!(session.notes().is_empty());

    let port = port_of(&injection.api_root);
    let root = get(port, "/").await;
    assert!(root["signaturePublickey"].as_str().is_some());
    let profile = get(
        port,
        "/sessionserver/session/minecraft/hasJoined?username=Steve",
    )
    .await;
    assert_eq!(profile["name"], "Steve");
    let textures = profile["properties"][0]["value"].as_str().unwrap();
    use base64::Engine as _;
    let decoded = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(textures)
            .unwrap(),
    )
    .unwrap();
    assert!(decoded.contains("\"slim\""), "{decoded}");
    assert!(decoded.contains("CAPE"), "{decoded}");

    // The key was made once and kept for the next launch.
    assert!(
        world
            .service
            .layout()
            .root()
            .join("skin-server.pem")
            .is_file()
    );

    // When the game is over and the session is gone, so is the server.
    drop(session);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}

#[tokio::test]
async fn a_skin_that_cannot_be_loaded_does_not_stop_the_game_but_is_told() {
    let world = skin_world();
    publish_injector(&world);
    // No route to the skin site.
    world
        .service
        .set_account_skin("Steve", Some(SkinChoice::LittleSkin))
        .await
        .unwrap();
    let settings = world.service.settings().await;
    let session = world.service.session_for(&settings).await.unwrap();
    assert!(session.injection().is_some());
    assert_eq!(session.notes().len(), 1);
    assert!(session.notes()[0].contains("skin"), "{:?}", session.notes());
}

#[tokio::test]
async fn without_the_agent_a_skin_cannot_work_so_the_launch_stops_with_that_reason() {
    let world = skin_world();
    publish_littleskin(&world);
    world
        .service
        .set_account_skin("Steve", Some(SkinChoice::LittleSkin))
        .await
        .unwrap();
    let settings = world.service.settings().await;
    let error = world.service.session_for(&settings).await.unwrap_err();
    assert!(matches!(error, ServiceError::Injector(_)), "{error}");
}

#[tokio::test]
async fn a_launch_loads_the_agent_for_a_skinned_player_and_tells_a_missing_skin_in_the_log() {
    let world = skin_world();
    publish_identity_release(&world);
    fake_java(&world, ECHO_ARGS);
    publish_injector(&world);
    world
        .service
        .set_account_skin("Steve", Some(SkinChoice::LittleSkin))
        .await
        .unwrap();
    let record = world
        .service
        .create_instance("Run", None, Loader::Vanilla, None)
        .await
        .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    world
        .service
        .launch(&record.id, tx, crate::activity::CancellationToken::new())
        .await
        .unwrap();
    let args =
        std::fs::read_to_string(world.service.layout().game(&record.id).join("args.txt")).unwrap();
    let agent = args
        .split_whitespace()
        .find(|a| a.starts_with("-javaagent:"))
        .expect("the agent");
    assert!(
        agent.contains("authlib-injector.jar=http://localhost:"),
        "{agent}"
    );
    assert!(args.contains("-Dauthlibinjector.side=client"));
    assert!(args.contains("--username Steve"), "{args}");
    let mut told = false;
    while let Ok(update) = rx.try_recv() {
        if let crate::launcher::LaunchUpdate::Log { text, .. } = update
            && text.contains("the skin was not loaded")
        {
            told = true;
        }
    }
    assert!(told, "the missing skin is reported in the launch log");
}
