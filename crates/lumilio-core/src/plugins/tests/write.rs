use super::*;
use crate::world_map::write::{KEEP_BACKUPS, describe, write_checked};
use lumilio_plugin_api::{API_VERSION, FileInfo, Manifest, Permission, PluginState, Words};

fn manifest() -> Manifest {
    Manifest {
        id: "test.write".into(),
        name: Words::new("W", "W"),
        description: Words::default(),
        version: "1".into(),
        api: API_VERSION,
        default_enabled: true,
        permissions: vec![
            Permission::ReadGameFiles {
                under: "xaero".into(),
            },
            Permission::WriteGameFiles {
                under: "xaero/minimap".into(),
                names: "mw$*.txt".into(),
            },
        ],
        settings: vec![],
    }
}

fn game() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let world = dir.path().join("xaero/minimap/World/dim%0");
    std::fs::create_dir_all(&world).unwrap();
    std::fs::write(world.join("mw$default_1.txt"), "old\n").unwrap();
    std::fs::write(dir.path().join("options.txt"), "keep").unwrap();
    std::fs::write(dir.path().join("xaero/other.txt"), "readable only").unwrap();
    dir
}

const FILE: &str = "xaero/minimap/World/dim%0/mw$default_1.txt";

#[test]
fn only_granted_directories_and_file_names_can_be_written() {
    let dir = game();
    let root = dir.path();
    let manifest = manifest();
    assert!(access::resolve_write(root, &manifest, FILE).is_ok());
    assert!(access::resolve_write(root, &manifest, "xaero/minimap/New/dim%0/mw$set_2.txt").is_ok());
    for bad in [
        "options.txt",
        "xaero/other.txt",
        "xaero/minimap",
        "xaero/minimap/World/dim%0/notes.txt",
        "xaero/minimap/World/dim%0/mw$default_1.json",
        "xaero/minimap/../../options.txt",
        "xaero/minimap/World/../../../options.txt",
        "/etc/mw$x.txt",
        "../mw$x.txt",
    ] {
        assert_eq!(
            access::resolve_write(root, &manifest, bad),
            Err(PluginError::PermissionDenied),
            "{bad}"
        );
    }
    // A read grant alone never allows a write.
    let mut read_only = manifest.clone();
    read_only
        .permissions
        .retain(|permission| matches!(permission, Permission::ReadGameFiles { .. }));
    assert_eq!(
        access::resolve_write(root, &read_only, FILE),
        Err(PluginError::PermissionDenied)
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            root.join("options.txt"),
            root.join("xaero/minimap/World/dim%0/mw$link_1.txt"),
        )
        .unwrap();
        assert_eq!(
            access::resolve_write(root, &manifest, "xaero/minimap/World/dim%0/mw$link_1.txt"),
            Err(PluginError::PermissionDenied)
        );
        std::os::unix::fs::symlink(root, root.join("xaero/minimap/Out")).unwrap();
        assert_eq!(
            access::resolve_write(root, &manifest, "xaero/minimap/Out/mw$x_1.txt"),
            Err(PluginError::PermissionDenied)
        );
    }
    assert_eq!(
        std::fs::read_to_string(root.join("options.txt")).unwrap(),
        "keep"
    );
    assert!(!root.join("mw$x.txt").exists());
}

#[test]
fn a_context_writes_only_while_the_host_holds_the_instance() {
    let dir = game();
    let backups = tempfile::tempdir().unwrap();
    let manifest = manifest();
    let mut context = context::Context::new(
        &manifest,
        &PluginState::default(),
        Some(dir.path().to_owned()),
        None,
        "en".into(),
    );
    let (_, info) = context.read_file_info(FILE).unwrap();
    // No lease, no write: the file and the backups stay untouched.
    assert_eq!(
        context.write_file(FILE, b"new\n", Some(&info)),
        Err(PluginError::PermissionDenied)
    );
    assert_eq!(std::fs::read(dir.path().join(FILE)).unwrap(), b"old\n");
    assert_eq!(std::fs::read_dir(backups.path()).unwrap().count(), 0);
    context.writer = Some(backups.path().to_owned());
    context.write_file(FILE, b"new\n", Some(&info)).unwrap();
    assert_eq!(std::fs::read(dir.path().join(FILE)).unwrap(), b"new\n");
    // A path outside the grant is refused even with the lease.
    assert_eq!(
        context.write_file("options.txt", b"x", None),
        Err(PluginError::PermissionDenied)
    );
}

#[test]
fn a_write_needs_the_file_to_be_what_was_read_and_backs_up_the_old_one() {
    let dir = game();
    let backups = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    let (_, info) = describe(&path).unwrap();
    assert_eq!(info.len, 4);
    assert_eq!(info.sha256.len(), 64);

    write_checked(&path, FILE, b"first\n", Some(&info), backups.path()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"first\n");
    let folder = backups.path().join(FILE.replace('/', "__"));
    let copies: Vec<_> = std::fs::read_dir(&folder).unwrap().flatten().collect();
    assert_eq!(copies.len(), 1);
    assert_eq!(std::fs::read(copies[0].path()).unwrap(), b"old\n");
    assert!(
        !dir.path()
            .join("xaero/minimap/World/dim%0/mw$default_1.txt.lumilio-tmp")
            .exists()
    );

    // The stale info no longer matches: nothing changes, nothing is backed up.
    assert_eq!(
        write_checked(&path, FILE, b"second\n", Some(&info), backups.path()),
        Err(PluginError::Unavailable("map-edit-conflict".into()))
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"first\n");
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 1);

    // Same length and hash but another modification time is also a conflict.
    let (_, now) = describe(&path).unwrap();
    let touched = FileInfo {
        modified_ms: now.modified_ms + 1,
        ..now.clone()
    };
    assert!(write_checked(&path, FILE, b"x", Some(&touched), backups.path()).is_err());
    let different = FileInfo {
        sha256: "0".repeat(64),
        ..now.clone()
    };
    assert!(write_checked(&path, FILE, b"x", Some(&different), backups.path()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"first\n");

    // A new file must really be new; an existing one with `None` is refused.
    let fresh = dir
        .path()
        .join("xaero/minimap/Fresh/dim%0/mw$default_1.txt");
    write_checked(&fresh, "fresh", b"hello\n", None, backups.path()).unwrap();
    assert_eq!(std::fs::read(&fresh).unwrap(), b"hello\n");
    assert!(write_checked(&fresh, "fresh", b"again", None, backups.path()).is_err());
    // Expecting a file that is gone is a conflict too.
    let gone = dir.path().join("xaero/minimap/Gone/mw$x_1.txt");
    assert!(write_checked(&gone, "gone", b"x", Some(&now), backups.path()).is_err());
    assert!(!gone.exists());
}

#[test]
fn only_the_latest_backups_of_a_file_are_kept() {
    let dir = game();
    let backups = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    for round in 0..KEEP_BACKUPS + 4 {
        let (_, info) = describe(&path).unwrap();
        write_checked(
            &path,
            FILE,
            format!("round {round}\n").as_bytes(),
            Some(&info),
            backups.path(),
        )
        .unwrap();
    }
    let folder = backups.path().join(FILE.replace('/', "__"));
    let mut kept: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .flatten()
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .collect();
    kept.sort();
    assert_eq!(kept.len(), KEEP_BACKUPS);
    assert!(
        kept.contains(&format!("round {}\n", KEEP_BACKUPS + 2)),
        "newest copy kept"
    );
    assert!(!kept.contains(&"old\n".to_owned()), "oldest copy dropped");
}

#[test]
fn ranges_stats_and_pages_obey_the_read_grant_and_do_not_need_the_whole_file() {
    let dir = game();
    let root = dir.path();
    let manifest = manifest();
    let big = root.join("xaero/big.bin");
    let file = std::fs::File::create(&big).unwrap();
    // Larger than a whole-file read allows, but a range of it is fine.
    file.set_len(access::MAX_FILE_BYTES + 4096).unwrap();
    std::fs::write(root.join("xaero/small.txt"), "0123456789").unwrap();
    assert!(access::read(root, &manifest, "xaero/big.bin").is_err());
    let tail =
        access::read_range(root, &manifest, "xaero/big.bin", access::MAX_FILE_BYTES, 16).unwrap();
    assert_eq!(tail, vec![0; 16]);
    assert_eq!(
        access::read_range(root, &manifest, "xaero/small.txt", 4, 100).unwrap(),
        b"456789",
        "a range past the end is cut short"
    );
    assert!(
        access::read_range(root, &manifest, "xaero/small.txt", 99, 4)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        access::read_range(
            root,
            &manifest,
            "xaero/small.txt",
            0,
            lumilio_plugin_api::MAX_RANGE + 1
        ),
        Err(PluginError::InvalidInput(_))
    ));
    for bad in ["options.txt", "../x", "/etc/passwd", "xaero/../options.txt"] {
        assert_eq!(
            access::read_range(root, &manifest, bad, 0, 1),
            Err(PluginError::PermissionDenied),
            "{bad}"
        );
        assert_eq!(
            access::stat(root, &manifest, bad),
            Err(PluginError::PermissionDenied)
        );
        assert_eq!(
            access::list_dir(root, &manifest, bad, None, 10),
            Err(PluginError::PermissionDenied)
        );
    }
    let stat = access::stat(root, &manifest, "xaero/small.txt")
        .unwrap()
        .unwrap();
    assert_eq!(stat.len, 10);
    assert!(stat.modified_ms > 0);
    assert_eq!(
        access::stat(root, &manifest, "xaero/nothing").unwrap(),
        None
    );
    assert_eq!(
        access::stat(root, &manifest, "xaero/minimap").unwrap(),
        None,
        "a directory is not a file"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("options.txt"), root.join("xaero/link")).unwrap();
        assert_eq!(
            access::read_range(root, &manifest, "xaero/link", 0, 1),
            Err(PluginError::PermissionDenied)
        );
        assert_eq!(
            access::stat(root, &manifest, "xaero/link"),
            Err(PluginError::PermissionDenied)
        );
    }

    // Pages: sorted, resumable with `after`, links left out, a missing directory is empty.
    let many = root.join("xaero/many");
    std::fs::create_dir_all(&many).unwrap();
    for n in 0..25 {
        std::fs::write(many.join(format!("r.{n:02}.mca")), "x").unwrap();
    }
    std::fs::create_dir_all(many.join("sub")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(root.join("options.txt"), many.join("aaa-link")).unwrap();
    let mut seen = Vec::new();
    let mut after: Option<String> = None;
    let mut pages = 0;
    loop {
        let page = access::list_dir(root, &manifest, "xaero/many", after.as_deref(), 10).unwrap();
        assert!(page.entries.len() <= 10);
        seen.extend(
            page.entries
                .iter()
                .map(|entry| (entry.name.clone(), entry.is_dir)),
        );
        pages += 1;
        match page.next {
            Some(next) => after = Some(next),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(seen.len(), 26, "25 files and one directory, no link");
    assert!(
        seen.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "sorted, no repeats"
    );
    assert!(seen.contains(&("sub".to_owned(), true)));
    assert!(!seen.iter().any(|(name, _)| name == "aaa-link"));
    assert_eq!(
        access::list_dir(root, &manifest, "xaero/absent", None, 10).unwrap(),
        lumilio_plugin_api::DirPage::default()
    );
}

#[test]
fn ranges_and_pages_refuse_links_anywhere_on_the_path() {
    let dir = game();
    let root = dir.path();
    let manifest = manifest();
    std::fs::create_dir_all(root.join("secret")).unwrap();
    std::fs::write(root.join("secret/r.0.0.mca"), "outside").unwrap();
    std::fs::write(root.join("xaero/r.0.0.mca"), "inside").unwrap();
    assert!(matches!(
        access::read_range(root, &manifest, "xaero/minimap", 0, 1),
        Err(PluginError::InvalidInput(_))
    ));
    assert_eq!(
        access::read_range(root, &manifest, "xaero/r.0.0.mca", u64::MAX, 4).unwrap(),
        Vec::<u8>::new(),
        "an offset far past the end is an empty range, not an error"
    );
    #[cfg(unix)]
    {
        // A granted-looking path that walks through a linked directory.
        std::os::unix::fs::symlink(root.join("secret"), root.join("xaero/region")).unwrap();
        assert_eq!(
            access::read_range(root, &manifest, "xaero/region/r.0.0.mca", 0, 4),
            Err(PluginError::PermissionDenied)
        );
        assert_eq!(
            access::stat(root, &manifest, "xaero/region/r.0.0.mca"),
            Err(PluginError::PermissionDenied)
        );
        assert_eq!(
            access::list_dir(root, &manifest, "xaero/region", None, 10),
            Err(PluginError::PermissionDenied)
        );
        let page = access::list_dir(root, &manifest, "xaero", None, 10).unwrap();
        assert!(
            !page.entries.iter().any(|entry| entry.name == "region"),
            "a linked directory is not listed"
        );
    }
}

#[test]
fn file_access_needs_a_game_directory() {
    let context = context::Context::new(
        &manifest(),
        &PluginState::default(),
        None,
        None,
        "en".into(),
    );
    assert_eq!(
        context.read_range("xaero/r.0.0.mca", 0, 1),
        Err(PluginError::PermissionDenied)
    );
    assert_eq!(
        context.file_stat("xaero/r.0.0.mca"),
        Err(PluginError::PermissionDenied)
    );
    assert_eq!(
        context.list_dir("xaero", None, 1),
        Err(PluginError::PermissionDenied)
    );
}
