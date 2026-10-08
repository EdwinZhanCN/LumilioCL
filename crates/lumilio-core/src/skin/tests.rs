use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use rsa::RsaPublicKey;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::pkcs8::DecodePublicKey;
use rsa::signature::Verifier as _;
use serde_json::{Value, json};
use sha1_legacy::Sha1;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::*;
use crate::account::ProfileId;
use crate::transfer::{TransportError, TransportFuture, TransportResponse};
use std::time::Duration;

const ID: &str = "123e4567e89b12d3a456426614174000";

fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, image::Rgba(color));
    let mut out = Vec::new();
    image
        .write_to(&mut io::Cursor::new(&mut out), ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn a_texture_is_named_by_its_pixels_and_transparent_colour_does_not_count() {
    let red = Texture::from_picture(&png(64, 64, [255, 0, 0, 255])).unwrap();
    let again = Texture::from_picture(&png(64, 64, [255, 0, 0, 255])).unwrap();
    let blue = Texture::from_picture(&png(64, 64, [0, 0, 255, 255])).unwrap();
    let small = Texture::from_picture(&png(64, 32, [255, 0, 0, 255])).unwrap();
    assert_eq!(red.hash, again.hash);
    assert_eq!(red.hash.len(), 64);
    assert_ne!(red.hash, blue.hash);
    assert_ne!(red.hash, small.hash);
    // Invisible pixels are the same whatever colour they hide.
    let a = Texture::from_picture(&png(64, 64, [1, 2, 3, 0])).unwrap();
    let b = Texture::from_picture(&png(64, 64, [9, 9, 9, 0])).unwrap();
    assert_eq!(a.hash, b.hash);
    // What is served is a PNG of those pixels.
    assert_eq!(image::load_from_memory(&red.png).unwrap().width(), 64);
}

#[test]
fn what_is_not_a_skin_sized_picture_is_refused() {
    assert!(matches!(
        Texture::from_picture(b"not an image"),
        Err(SkinError::Picture(_))
    ));
    assert!(matches!(
        Texture::from_picture(&png(5000, 2, [0, 0, 0, 255])),
        Err(SkinError::Picture(_))
    ));
    assert!(matches!(
        Texture::from_picture(&vec![0_u8; 3 * 1024 * 1024]),
        Err(SkinError::Picture(_))
    ));
}

#[test]
fn csl_addresses_become_https_without_a_trailing_slash() {
    assert_eq!(
        normalize_api("skin.example/csl/").unwrap(),
        "https://skin.example/csl"
    );
    assert_eq!(
        normalize_api(" http://localhost:8080/api ").unwrap(),
        "http://localhost:8080/api"
    );
    assert!(matches!(
        normalize_api("ftp://x"),
        Err(SkinError::InvalidApi(_))
    ));
    assert!(matches!(normalize_api(""), Err(SkinError::InvalidApi(_))));
}

#[tokio::test]
async fn local_files_load_with_their_model_and_a_bad_file_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (skin, cape) = (dir.path().join("skin.png"), dir.path().join("cape.png"));
    fs::write(&skin, png(64, 64, [10, 20, 30, 255])).unwrap();
    fs::write(&cape, png(64, 32, [200, 0, 0, 255])).unwrap();
    let choice = SkinChoice::Local {
        model: SkinModel::Slim,
        skin: Some(skin),
        cape: Some(cape),
    };
    assert!(check(&choice).is_ok());
    let loaded = load(&Down, &choice, "Edwin").await.unwrap();
    assert_eq!(loaded.model, SkinModel::Slim);
    assert!(loaded.skin.is_some() && loaded.cape.is_some());

    let broken = dir.path().join("broken.png");
    fs::write(&broken, b"nope").unwrap();
    let bad = SkinChoice::Local {
        model: SkinModel::Wide,
        skin: Some(broken),
        cape: None,
    };
    assert!(matches!(check(&bad), Err(SkinError::Picture(_))));
    assert!(load(&Down, &bad, "Edwin").await.is_err());
    let missing = SkinChoice::Local {
        model: SkinModel::Wide,
        skin: Some(dir.path().join("gone.png")),
        cape: None,
    };
    assert!(matches!(check(&missing), Err(SkinError::Io(_))));
}

struct Down;

impl Transport for Down {
    fn get<'a>(&'a self, _: &'a str) -> TransportFuture<'a> {
        Box::pin(async { Err(TransportError::transient("offline")) })
    }
}

#[derive(Default)]
struct Site {
    documents: Mutex<BTreeMap<String, Vec<u8>>>,
    asked: Mutex<Vec<String>>,
}

impl Site {
    fn serve(&self, url: &str, bytes: Vec<u8>) {
        self.documents.lock().unwrap().insert(url.to_owned(), bytes);
    }
}

impl Transport for Site {
    fn get<'a>(&'a self, source: &'a str) -> TransportFuture<'a> {
        Box::pin(async move {
            self.asked.lock().unwrap().push(source.to_owned());
            let found = self.documents.lock().unwrap().get(source).cloned();
            Ok(TransportResponse::from_bytes(
                if found.is_some() { 200 } else { 404 },
                found.unwrap_or_default(),
            ))
        })
    }
}

#[tokio::test]
async fn a_csl_profile_gives_the_model_of_the_skin_it_has_and_its_cape() {
    let site = Site::default();
    let api = "https://skin.example/csl";
    site.serve(
        &format!("{api}/Edwin.json"),
        json!({"username": "Edwin", "textures": {"slim": "aaa", "default": "bbb", "cape": "ccc"}})
            .to_string()
            .into_bytes(),
    );
    site.serve(&format!("{api}/textures/aaa"), png(64, 64, [1, 1, 1, 255]));
    site.serve(&format!("{api}/textures/bbb"), png(64, 64, [2, 2, 2, 255]));
    site.serve(&format!("{api}/textures/ccc"), png(64, 32, [3, 3, 3, 255]));
    let choice = SkinChoice::Csl {
        api: format!("{api}/"),
    };
    let loaded = load(&site, &choice, "Edwin").await.unwrap();
    // Slim wins when the profile has both, as in HMCL.
    assert_eq!(loaded.model, SkinModel::Slim);
    assert_eq!(
        loaded.skin.unwrap().hash,
        Texture::from_picture(&png(64, 64, [1, 1, 1, 255]))
            .unwrap()
            .hash
    );
    assert!(loaded.cape.is_some());
    assert!(
        !site
            .asked
            .lock()
            .unwrap()
            .iter()
            .any(|url| url.ends_with("/bbb"))
    );
}

#[tokio::test]
async fn csl_profiles_without_a_name_or_with_odd_answers_are_handled() {
    let site = Site::default();
    let api = "https://skin.example/csl";
    // No such player: the site answers a profile with no name.
    site.serve(
        &format!("{api}/Nobody.json"),
        br#"{"username":""}"#.to_vec(),
    );
    let choice = SkinChoice::Csl {
        api: api.to_owned(),
    };
    assert_eq!(
        load(&site, &choice, "Nobody").await.unwrap(),
        LoadedSkin::default()
    );

    // An older profile that only has the flat `skin` field is a classic skin.
    site.serve(
        &format!("{api}/Old.json"),
        br#"{"username":"Old","skin":"abc"}"#.to_vec(),
    );
    site.serve(&format!("{api}/textures/abc"), png(64, 32, [5, 5, 5, 255]));
    let old = load(&site, &choice, "Old").await.unwrap();
    assert_eq!(old.model, SkinModel::Wide);
    assert!(old.skin.is_some() && old.cape.is_none());

    // A texture name that is not a plain name could climb out of /textures/.
    site.serve(
        &format!("{api}/Evil.json"),
        br#"{"username":"Evil","textures":{"default":"../../x"}}"#.to_vec(),
    );
    assert!(matches!(
        load(&site, &choice, "Evil").await,
        Err(SkinError::Malformed(_))
    ));
    site.serve(&format!("{api}/Junk.json"), b"<html>".to_vec());
    assert!(matches!(
        load(&site, &choice, "Junk").await,
        Err(SkinError::Malformed(_))
    ));
    assert!(matches!(
        load(&Down, &choice, "Edwin").await,
        Err(SkinError::Network(_))
    ));
}

#[tokio::test]
async fn littleskin_uses_its_own_csl_address() {
    let site = Site::default();
    site.serve(
        "https://littleskin.cn/csl/Edwin.json",
        br#"{"username":"Edwin","textures":{"default":"abc"}}"#.to_vec(),
    );
    site.serve(
        "https://littleskin.cn/csl/textures/abc",
        png(64, 64, [7, 7, 7, 255]),
    );
    let loaded = load(&site, &SkinChoice::LittleSkin, "Edwin").await.unwrap();
    assert!(loaded.skin.is_some());
}

#[test]
fn a_choice_survives_the_settings_file() {
    for choice in [
        SkinChoice::LittleSkin,
        SkinChoice::Csl {
            api: "https://a/csl".into(),
        },
        SkinChoice::Local {
            model: SkinModel::Slim,
            skin: Some("/s.png".into()),
            cape: None,
        },
    ] {
        let text = serde_json::to_string(&choice).unwrap();
        assert_eq!(serde_json::from_str::<SkinChoice>(&text).unwrap(), choice);
    }
    assert_eq!(
        serde_json::to_string(&SkinChoice::LittleSkin).unwrap(),
        r#"{"type":"little_skin"}"#
    );
}

// ---- the local server, over real TCP ----

fn signer() -> Arc<Signer> {
    // A small key keeps the tests quick; the real one is 2048 bits.
    Arc::new(Signer::generate(1024).unwrap())
}

async fn started(model: SkinModel, cape: bool) -> (LocalSkinServer, Arc<Signer>, LoadedSkin) {
    let loaded = LoadedSkin {
        model,
        skin: Some(Texture::from_picture(&png(64, 64, [10, 20, 30, 255])).unwrap()),
        cape: cape.then(|| Texture::from_picture(&png(64, 32, [200, 0, 0, 255])).unwrap()),
    };
    let signer = signer();
    let server = LocalSkinServer::start(
        Character {
            id: ProfileId::parse(ID).unwrap(),
            name: "Edwin".into(),
            skin: loaded.clone(),
        },
        signer.clone(),
    )
    .await
    .unwrap();
    (server, signer, loaded)
}

struct Reply {
    status: u16,
    headers: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn ask(port: u16, method: &str, target: &str, body: &str) -> Reply {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!(
        "{method} {target} HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let status = head.split(' ').nth(1).unwrap().parse().unwrap();
    Reply {
        status,
        headers: head,
        body: raw[split + 4..].to_vec(),
    }
}

#[tokio::test]
async fn the_root_publishes_the_key_and_the_domains_the_game_may_load_skins_from() {
    let (server, signer, _) = started(SkinModel::Wide, false).await;
    let reply = ask(server.port(), "GET", "/", "").await;
    assert_eq!(reply.status, 200);
    let root = reply.json();
    assert_eq!(root["signaturePublickey"], signer.public_key_pem());
    assert!(
        root["signaturePublickey"]
            .as_str()
            .unwrap()
            .starts_with("-----BEGIN PUBLIC KEY-----")
    );
    assert_eq!(root["skinDomains"], json!(["127.0.0.1", "localhost"]));
    assert_eq!(root["meta"]["feature.non_email_login"], true);
    assert_eq!(
        ask(server.port(), "GET", "/status", "").await.json()["user.count"],
        1
    );
    assert_eq!(ask(server.port(), "GET", "/nothing", "").await.status, 404);
}

#[tokio::test]
async fn the_character_comes_with_signed_textures_the_game_can_fetch() {
    let (server, signer, loaded) = started(SkinModel::Slim, true).await;
    let port = server.port();
    let reply = ask(
        port,
        "GET",
        "/sessionserver/session/minecraft/hasJoined?username=Edwin&serverId=x",
        "",
    )
    .await;
    assert_eq!(reply.status, 200);
    let profile = reply.json();
    assert_eq!(
        (
            profile["id"].as_str().unwrap(),
            profile["name"].as_str().unwrap()
        ),
        (ID, "Edwin")
    );
    let property = &profile["properties"][0];
    assert_eq!(property["name"], "textures");

    // The signature checks out against the published key (SHA1withRSA).
    let public = RsaPublicKey::from_public_key_pem(signer.public_key_pem()).unwrap();
    let signature = base64::engine::general_purpose::STANDARD
        .decode(property["signature"].as_str().unwrap())
        .unwrap();
    VerifyingKey::<Sha1>::new(public)
        .verify(
            property["value"].as_str().unwrap().as_bytes(),
            &Signature::try_from(signature.as_slice()).unwrap(),
        )
        .expect("the textures property is signed with the published key");

    let textures: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(property["value"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(textures["profileId"], ID);
    assert_eq!(textures["profileName"], "Edwin");
    let skin = &loaded.skin.as_ref().unwrap().hash;
    assert_eq!(
        textures["textures"]["SKIN"]["url"],
        format!("http://localhost:{port}/textures/{skin}")
    );
    assert_eq!(textures["textures"]["SKIN"]["metadata"]["model"], "slim");
    assert!(
        textures["textures"]["CAPE"]["url"]
            .as_str()
            .unwrap()
            .contains("/textures/")
    );

    // The picture itself, with the headers that let the game cache it.
    let picture = ask(port, "GET", &format!("/textures/{skin}"), "").await;
    assert_eq!(picture.status, 200);
    assert!(picture.headers.contains("image/png"));
    assert!(picture.headers.contains(&format!("Etag: \"{skin}\"")));
    assert!(picture.headers.contains("max-age=2592000"));
    assert_eq!(picture.body, loaded.skin.unwrap().png);
    assert_eq!(
        ask(port, "GET", &format!("/textures/{}", "0".repeat(64)), "")
            .await
            .status,
        404
    );
}

#[tokio::test]
async fn a_wide_skin_names_no_model_and_a_skinless_player_still_resolves() {
    let (server, _, _) = started(SkinModel::Wide, false).await;
    let profile = ask(
        server.port(),
        "GET",
        &format!("/sessionserver/session/minecraft/profile/{ID}"),
        "",
    )
    .await
    .json();
    let value = profile["properties"][0]["value"].as_str().unwrap();
    let textures: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(value)
            .unwrap(),
    )
    .unwrap();
    assert!(textures["textures"]["SKIN"].get("metadata").is_none());
    assert!(textures["textures"].get("CAPE").is_none());

    let bare = LocalSkinServer::start(
        Character {
            id: ProfileId::parse(ID).unwrap(),
            name: "Edwin".into(),
            skin: LoadedSkin::default(),
        },
        signer(),
    )
    .await
    .unwrap();
    let profile = ask(
        bare.port(),
        "GET",
        &format!("/sessionserver/session/minecraft/profile/{ID}"),
        "",
    )
    .await
    .json();
    let value = profile["properties"][0]["value"].as_str().unwrap();
    let textures: Value = serde_json::from_slice(
        &base64::engine::general_purpose::STANDARD
            .decode(value)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(textures["textures"], json!({}));
}

#[tokio::test]
async fn only_the_one_player_is_known_and_odd_requests_are_refused() {
    let (server, _, _) = started(SkinModel::Wide, false).await;
    let port = server.port();
    assert_eq!(
        ask(
            port,
            "GET",
            "/sessionserver/session/minecraft/hasJoined?username=Someone",
            ""
        )
        .await
        .status,
        204
    );
    assert_eq!(
        ask(
            port,
            "GET",
            "/sessionserver/session/minecraft/hasJoined",
            ""
        )
        .await
        .status,
        400
    );
    let other = "223e4567e89b12d3a456426614174000";
    assert_eq!(
        ask(
            port,
            "GET",
            &format!("/sessionserver/session/minecraft/profile/{other}"),
            ""
        )
        .await
        .status,
        204
    );
    assert_eq!(
        ask(port, "POST", "/sessionserver/session/minecraft/join", "{}")
            .await
            .status,
        204
    );

    let found = ask(
        port,
        "POST",
        "/api/profiles/minecraft",
        r#"["Edwin","Nobody"]"#,
    )
    .await
    .json();
    assert_eq!(found, json!([{"id": ID, "name": "Edwin"}]));
    assert_eq!(
        ask(port, "POST", "/api/profiles/minecraft", r#"["Nobody"]"#)
            .await
            .json(),
        json!([])
    );
    assert_eq!(
        ask(port, "POST", "/api/profiles/minecraft", "not json")
            .await
            .status,
        400
    );

    // A request that claims a huge body is refused, not waited for.
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream
        .write_all(b"POST /api/profiles/minecraft HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n")
        .await
        .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).await.unwrap();
    assert!(String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 400"));
}

#[tokio::test]
async fn dropping_the_server_stops_it() {
    let (server, _, _) = started(SkinModel::Wide, false).await;
    let port = server.port();
    assert_eq!(ask(port, "GET", "/status", "").await.status, 200);
    drop(server);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}

#[test]
fn the_signing_key_is_made_once_and_kept_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keys/skin-server.pem");
    let first = Signer::load_or_create(&path, 1024).unwrap();
    let second = Signer::load_or_create(&path, 1024).unwrap();
    assert_eq!(first.public_key_pem(), second.public_key_pem());
    assert_eq!(first.sign("data"), second.sign("data"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    // A damaged file is replaced rather than trusted.
    fs::write(&path, "garbage").unwrap();
    assert!(Signer::load_or_create(&path, 1024).is_ok());
}

fn picture(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(paint(x, y)));
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn texel(pixels: &Pixels, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * pixels.width + x) * 4) as usize;
    pixels.rgba[at..at + 4].try_into().unwrap()
}

#[test]
fn a_legacy_skin_gets_mirrored_left_limbs_and_a_cleared_hat() {
    // Old skins: every texel opaque; the right arm's front is one colour
    // with a marker in its left column.
    let old = picture(64, 32, |x, y| match (x, y) {
        (44, 20..32) => [255, 0, 0, 255],
        (45..48, 20..32) => [0, 255, 0, 255],
        _ => [10, 10, 10, 255],
    });
    let skin = skin_pixels(&old).unwrap();
    assert_eq!((skin.width, skin.height), (64, 64));
    // The left arm's front is the right arm's front, mirrored.
    assert_eq!(texel(&skin, 39, 52), [255, 0, 0, 255]);
    assert_eq!(texel(&skin, 36, 52), [0, 255, 0, 255]);
    // A hat area with no transparency was filler.
    assert_eq!(texel(&skin, 40, 8)[3], 0);
    assert!(!looks_slim(&skin));
}

#[test]
fn modern_skins_pass_through_and_odd_shapes_are_refused() {
    let modern = picture(64, 64, |x, _| {
        if (54..56).contains(&x) {
            [0; 4]
        } else {
            [1, 2, 3, 255]
        }
    });
    let skin = skin_pixels(&modern).unwrap();
    assert_eq!(texel(&skin, 0, 0), [1, 2, 3, 255]);
    assert!(looks_slim(&skin));
    assert!(skin_pixels(&picture(64, 48, |_, _| [0; 4])).is_err());
    assert!(skin_pixels(&picture(100, 100, |_, _| [0; 4])).is_err());
    assert!(skin_pixels(b"not a picture").is_err());
}

#[test]
fn capes_are_two_to_one_and_early_ones_are_placed_on_a_canvas() {
    let cape = cape_pixels(&picture(128, 64, |_, _| [9, 9, 9, 255])).unwrap();
    assert_eq!((cape.width, cape.height), (128, 64));
    let early = cape_pixels(&picture(22, 17, |_, _| [5, 5, 5, 255])).unwrap();
    assert_eq!((early.width, early.height), (64, 32));
    assert_eq!(texel(&early, 21, 16), [5, 5, 5, 255]);
    assert_eq!(texel(&early, 30, 20)[3], 0);
    assert!(cape_pixels(&picture(64, 64, |_, _| [0; 4])).is_err());
}
