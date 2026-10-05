use super::*;

fn request(pack: Option<&str>) -> ModelRequest {
    ModelRequest {
        title: "house".into(),
        schematic: vec![1, 2, 3],
        pack: pack.map(|id| ModelPack {
            id: id.into(),
            bytes: vec![9, 9],
        }),
    }
}

#[test]
fn the_viewer_page_and_the_schematic_are_served_and_the_pack_only_when_there_is_one() {
    let with = request(Some("jar-26.2-1-2"));
    let page = serve("/viewer.html", &with);
    assert_eq!((page.status, page.content_type), (200, "text/html"));
    assert!(String::from_utf8(page.body).unwrap().contains("<canvas"));
    assert_eq!(serve("/", &with).status, 200);
    assert_eq!(serve("/schematic", &with).body, [1, 2, 3]);
    assert_eq!(serve("/pack.zip", &with).body, [9, 9]);
    assert_eq!(serve("/pack.zip", &request(None)).status, 404);
}

#[test]
fn every_vendored_file_decompresses_and_is_the_right_kind() {
    let request = request(None);
    for path in assets::paths() {
        let served = serve(path, &request);
        assert_eq!(served.status, 200, "{path}");
        assert!(served.body.len() > 100, "{path} is empty");
        if path.ends_with(".wasm") {
            assert_eq!(&served.body[..4], b"\0asm", "{path}");
            assert_eq!(served.content_type, "application/wasm");
        } else {
            assert_eq!(served.content_type, "text/javascript", "{path}");
        }
    }
}

#[test]
fn nothing_else_is_served_and_mojangs_pack_is_not_among_the_files() {
    let request = request(None);
    for path in [
        "/sr/pack.zip",
        "/sr/../../Cargo.toml",
        "/etc/passwd",
        "/sr/Inspector-a3b87e9e.mjs",
        "/sr/schematic-renderer.umd.js",
    ] {
        assert_eq!(serve(path, &request).status, 404, "{path}");
    }
    assert!(assets::paths().all(|path| !path.ends_with(".zip")));
}

#[test]
fn the_page_address_carries_the_pack_name() {
    assert_eq!(
        page_url("lumilio://localhost", &request(Some("jar-1.21-a-b"))),
        "lumilio://localhost/viewer.html?pack=jar-1.21-a-b"
    );
    assert_eq!(
        page_url("lumilio://localhost", &request(None)),
        "lumilio://localhost/viewer.html"
    );
}

#[test]
fn the_viewer_page_names_no_network_address() {
    let page = String::from_utf8(assets::VIEWER.to_vec()).unwrap();
    assert!(
        !page.contains("http://") && !page.contains("https://"),
        "the viewer works offline"
    );
    assert!(page.contains("/pack.zip") && page.contains("/schematic"));
}
