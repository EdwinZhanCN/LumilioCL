use super::ping::{parse_address, probe_at, read_status};
use super::*;
use image::ImageEncoder as _;
use image::codecs::png::PngEncoder;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A base64 PNG, as both `servers.dat` and a status `favicon` carry one.
fn png_b64(width: u32, height: u32) -> String {
    let raw = [1_u8, 2, 3, 255].repeat((width * height) as usize);
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&raw, width, height, image::ExtendedColorType::Rgba8)
        .unwrap();
    base64::engine::general_purpose::STANDARD.encode(&bytes)
}

fn game() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    fs::create_dir_all(&game).unwrap();
    (dir, game)
}

fn server(name: &str, address: &str) -> ServerEntry {
    ServerEntry {
        name: name.into(),
        address: address.into(),
        packs: PackPolicy::Ask,
        icon: None,
    }
}

#[test]
fn a_stored_icon_is_read_but_never_lost_by_an_edit() {
    let (_dir, game) = game();
    let mut raw = Raw::new();
    raw.insert("name".into(), Tag::String("Home".into()));
    raw.insert("ip".into(), Tag::String("a.example.com".into()));
    raw.insert("icon".into(), Tag::String(png_b64(64, 64)));
    store(&game, Raw::new(), vec![raw]).unwrap();

    let listed = list(&game).unwrap();
    let icon = listed[0].icon.as_ref().expect("the icon is read");
    assert_eq!((icon.width, icon.height), (64, 64));

    // Renaming keeps the game's own icon on disk.
    let renamed = server("New", "a.example.com");
    update(&game, 0, &listed[0], &renamed).unwrap();
    let again = list(&game).unwrap();
    assert_eq!(again[0].name, "New");
    assert!(again[0].icon.is_some());
}

#[test]
fn an_unusable_stored_icon_is_just_no_icon() {
    let (_dir, game) = game();
    let mut raw = Raw::new();
    raw.insert("name".into(), Tag::String("Home".into()));
    raw.insert("ip".into(), Tag::String("a".into()));
    raw.insert("icon".into(), Tag::String("bm90IGEgcG5n".into()));
    store(&game, Raw::new(), vec![raw]).unwrap();
    assert!(list(&game).unwrap()[0].icon.is_none());
}

#[test]
fn a_missing_list_is_empty_and_the_first_add_creates_it() {
    let (_dir, game) = game();
    assert!(list(&game).unwrap().is_empty());
    add(&game, &server("Home", "play.example.com")).unwrap();
    assert_eq!(list(&game).unwrap(), [server("Home", "play.example.com")]);
}

#[test]
fn edits_keep_fields_the_launcher_does_not_manage() {
    let (_dir, game) = game();
    let mut raw = Raw::new();
    raw.insert("name".into(), Tag::String("Old".into()));
    raw.insert("ip".into(), Tag::String("a.example.com".into()));
    raw.insert("icon".into(), Tag::String("bm90IGFuIGljb24=".into()));
    raw.insert("hidden".into(), Tag::Byte(1));
    store(&game, Raw::new(), vec![raw]).unwrap();

    let mut renamed = server("New", "b.example.com:25570");
    renamed.packs = PackPolicy::Allow;
    update(&game, 0, &server("Old", "a.example.com"), &renamed).unwrap();

    let (_, servers) = load(&game).unwrap();
    assert_eq!(servers[0]["icon"], Tag::String("bm90IGFuIGljb24=".into()));
    assert_eq!(servers[0]["hidden"], Tag::Byte(1));
    assert_eq!(entry_of(&servers[0]), renamed);
    // The game's own habit: the previous list stays beside the new one.
    assert!(game.join("servers.dat_old").is_file());
}

#[test]
fn remove_and_move_refuse_a_row_that_is_no_longer_there() {
    let (_dir, game) = game();
    for (name, address) in [("A", "a"), ("B", "b"), ("C", "c")] {
        add(&game, &server(name, address)).unwrap();
    }
    move_to(&game, 2, &server("C", "c"), 0).unwrap();
    let names = || {
        list(&game)
            .unwrap()
            .into_iter()
            .map(|entry| entry.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(), ["C", "A", "B"]);
    // The caller still thinks "B" is at position 1.
    assert!(matches!(
        remove(&game, 1, &server("B", "b")),
        Err(ServerError::Changed)
    ));
    assert!(matches!(
        remove(&game, 9, &server("B", "b")),
        Err(ServerError::NotFound(9))
    ));
    remove(&game, 2, &server("B", "b")).unwrap();
    assert_eq!(names(), ["C", "A"]);
}

#[test]
fn invalid_fields_are_refused_and_a_damaged_file_is_not_overwritten() {
    let (_dir, game) = game();
    assert!(matches!(
        add(&game, &server(" ", "a")),
        Err(ServerError::InvalidField("name"))
    ));
    assert!(matches!(
        add(&game, &server("A", "has space")),
        Err(ServerError::InvalidField("address"))
    ));
    fs::write(game.join("servers.dat"), b"not nbt").unwrap();
    assert!(matches!(list(&game), Err(ServerError::Damaged(_))));
    assert!(add(&game, &server("A", "a")).is_err());
    assert_eq!(fs::read(game.join("servers.dat")).unwrap(), b"not nbt");
}

#[test]
fn a_status_without_players_or_version_still_reads() {
    let status = read_status(br#"{"description":"Hi"}"#).unwrap();
    assert_eq!(
        (status.motd.as_str(), status.online, status.version),
        ("Hi", None, None)
    );
    assert!(status.favicon.is_none());
}

#[test]
fn a_status_favicon_is_the_servers_icon_when_it_is_a_png() {
    let ok = format!(
        r#"{{"description":"x","favicon":"data:image/png;base64,{}"}}"#,
        png_b64(64, 64)
    );
    let status = read_status(ok.as_bytes()).unwrap();
    assert_eq!(
        status.favicon.as_ref().map(|p| (p.width, p.height)),
        Some((64, 64))
    );
    // A data URI that is not a PNG is no icon, and neither is a missing one.
    let odd = read_status(br#"{"description":"x","favicon":"data:image/png;base64,bm90IGEgcG5n"}"#)
        .unwrap();
    assert!(odd.favicon.is_none());
}

fn varint(out: &mut Vec<u8>, value: usize) {
    let mut value = value;
    while value >= 0x80 {
        out.push((value & 0x7f) as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// A server that answers one status request with `json` and echoes the ping.
async fn scripted(json: &'static str, echo: bool) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        // The handshake packet (its length is one byte here), then the
        // two-byte status request, each read whole so nothing is left over.
        let mut length = [0_u8; 1];
        stream.read_exact(&mut length).await.unwrap();
        let mut handshake = vec![0_u8; usize::from(length[0])];
        stream.read_exact(&mut handshake).await.unwrap();
        let mut request = [0_u8; 2];
        stream.read_exact(&mut request).await.unwrap();
        let mut body = vec![0x00];
        varint(&mut body, json.len());
        body.extend(json.as_bytes());
        let mut packet = Vec::new();
        varint(&mut packet, body.len());
        packet.extend(body);
        stream.write_all(&packet).await.unwrap();
        if echo {
            let mut ping = [0_u8; 10];
            if stream.read_exact(&mut ping).await.is_ok() {
                stream.write_all(&ping).await.unwrap();
            }
        }
    });
    port
}

#[tokio::test]
async fn probing_reads_the_status_and_the_latency() {
    let port = scripted(
        r#"{"description":{"text":"Welcome"},"players":{"online":2,"max":10},"version":{"name":"1.21.1"}}"#,
        true,
    )
    .await;
    let status = probe_at("127.0.0.1", port, None).await.unwrap();
    assert_eq!(status.motd, "Welcome");
    assert_eq!((status.online, status.max), (Some(2), Some(10)));
    assert!(status.latency_ms.is_some());
}

#[tokio::test]
async fn a_server_that_skips_the_ping_still_has_a_status() {
    let port = scripted(r#"{"description":"x"}"#, false).await;
    assert_eq!(
        probe_at("127.0.0.1", port, None).await.unwrap().latency_ms,
        None
    );
}

#[tokio::test]
async fn an_absurd_length_is_refused_before_anything_is_reserved() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut sink = [0_u8; 64];
        let _ = stream.read(&mut sink).await;
        // A packet claiming two billion bytes.
        let _ = stream.write_all(&[0xff, 0xff, 0xff, 0xff, 0x07]).await;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    });
    assert!(matches!(
        probe_at("127.0.0.1", port, None).await,
        Err(PingError::Protocol(_))
    ));
}

#[tokio::test]
async fn nothing_listening_is_an_error_not_a_hang() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap().port()
    };
    assert!(matches!(
        probe_at("127.0.0.1", port, None).await,
        Err(PingError::Io(_))
    ));
}

#[test]
fn the_protocol_table_knows_legacy_and_modern_versions() {
    assert_eq!(
        protocol::OLD_PROTOCOL_VERSIONS.get("1.6.4"),
        Some(&ProtocolVersion::legacy(78))
    );
    assert_eq!(
        protocol::OLD_PROTOCOL_VERSIONS.get("1.13.2"),
        Some(&ProtocolVersion::modern(404))
    );
    // A version the table knows needs no jar.
    assert_eq!(
        protocol_version("1.6.2", std::path::Path::new("/nonexistent")),
        Some(ProtocolVersion::legacy(74))
    );
    // One it does not know falls to the jar, which is not here: no answer.
    assert!(protocol_version("1.20.1", std::path::Path::new("/nonexistent")).is_none());
}

/// A server that answers one request with a legacy `0xFF` UTF-16 status.
async fn scripted_legacy(answer: &str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let units: Vec<u16> = answer.encode_utf16().collect();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = [0_u8; 512];
        let _ = stream.read(&mut request).await;
        let mut body = vec![0xff_u8];
        body.extend_from_slice(&u16::try_from(units.len()).unwrap().to_be_bytes());
        for unit in units {
            body.extend_from_slice(&unit.to_be_bytes());
        }
        stream.write_all(&body).await.unwrap();
    });
    port
}

#[tokio::test]
async fn a_legacy_server_is_asked_the_old_way() {
    let port = scripted_legacy("§1\u{0}78\u{0}1.6.4\u{0}A classic\u{0}3\u{0}20").await;
    let status = probe_at("127.0.0.1", port, Some(ProtocolVersion::legacy(78)))
        .await
        .unwrap();
    assert_eq!(status.version.as_deref(), Some("1.6.4"));
    assert_eq!(status.motd, "A classic");
    assert_eq!((status.online, status.max), (Some(3), Some(20)));
    // The legacy ping carries no round trip.
    assert!(status.latency_ms.is_none());
}

#[tokio::test]
async fn an_ancient_legacy_answer_still_reads_its_motd_and_counts() {
    // No "§1" marker: `§`-separated, motd first.
    let port = scripted_legacy("Old world§5§30").await;
    let status = probe_at("127.0.0.1", port, Some(ProtocolVersion::legacy(39)))
        .await
        .unwrap();
    assert_eq!(status.motd, "Old world");
    assert_eq!((status.online, status.max), (Some(5), Some(30)));
    assert!(status.version.is_none());
}

#[test]
fn addresses_split_into_host_and_port() {
    let parse = |text| parse_address(text).unwrap();
    assert_eq!(parse("mc.example.com"), ("mc.example.com".into(), 25565));
    assert_eq!(
        parse("mc.example.com:25570"),
        ("mc.example.com".into(), 25570)
    );
    assert_eq!(parse("[::1]:2000"), ("::1".into(), 2000));
    assert_eq!(parse("::1"), ("::1".into(), 25565));
    assert!(parse_address(":25565").is_err());
    assert!(parse_address("host:notaport").is_err());
    assert!(parse_address("").is_err());
}
