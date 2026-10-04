use super::ping::{parse_address, probe_at, read_status};
use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

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
    }
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
    raw.insert("icon".into(), Tag::String("iVBORw0KGgo=".into()));
    raw.insert("hidden".into(), Tag::Byte(1));
    store(&game, Raw::new(), vec![raw]).unwrap();

    let mut renamed = server("New", "b.example.com:25570");
    renamed.packs = PackPolicy::Allow;
    update(&game, 0, &server("Old", "a.example.com"), &renamed).unwrap();

    let (_, servers) = load(&game).unwrap();
    assert_eq!(servers[0]["icon"], Tag::String("iVBORw0KGgo=".into()));
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
    let status = probe_at("127.0.0.1", port).await.unwrap();
    assert_eq!(status.motd, "Welcome");
    assert_eq!((status.online, status.max), (Some(2), Some(10)));
    assert!(status.latency_ms.is_some());
}

#[tokio::test]
async fn a_server_that_skips_the_ping_still_has_a_status() {
    let port = scripted(r#"{"description":"x"}"#, false).await;
    assert_eq!(probe_at("127.0.0.1", port).await.unwrap().latency_ms, None);
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
        probe_at("127.0.0.1", port).await,
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
        probe_at("127.0.0.1", port).await,
        Err(PingError::Io(_))
    ));
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
