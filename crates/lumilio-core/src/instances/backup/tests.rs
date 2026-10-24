use super::*;

fn meta() -> BackupMeta {
    BackupMeta {
        format: FORMAT,
        name: "生存".into(),
        game_version: "1.21.1".into(),
        loader: Loader::Fabric,
        loader_version: Some("0.16.0".into()),
        created: 5,
        settings: InstanceSettings {
            max_memory_mb: Some(4096),
            ..InstanceSettings::default()
        },
    }
}

fn game() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("game");
    for (path, body) in [
        ("mods/a.jar", "mod"),
        ("config/x.toml", "cfg"),
        ("options.txt", "fov"),
        ("saves/W/level.dat", "world"),
        ("logs/latest.log", "log"),
        ("crash-reports/c.txt", "crash"),
    ] {
        let full = game.join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    (dir, game)
}

#[test]
fn a_backup_holds_everything_but_logs_and_restores_it_whole() {
    let (dir, game) = game();
    let archive = dir.path().join("b.zip");
    let size = write(&game, &meta(), &archive, &CancellationToken::new()).unwrap();
    assert!(size > 0 && !dir.path().join("b.lumilio-backup.part").exists());
    assert_eq!(read_meta(&archive).unwrap(), meta());

    let restored = dir.path().join("restored");
    let count = restore(&archive, &restored, &CancellationToken::new()).unwrap();
    assert_eq!(count, 4);
    assert_eq!(fs::read(restored.join("mods/a.jar")).unwrap(), b"mod");
    assert_eq!(
        fs::read(restored.join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!restored.join("logs").exists() && !restored.join("crash-reports").exists());
}

#[test]
fn a_cancelled_backup_leaves_nothing_and_a_foreign_zip_is_not_a_backup() {
    let (dir, game) = game();
    let archive = dir.path().join("never.zip");
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(matches!(
        write(&game, &meta(), &archive, &cancel),
        Err(BackupError::Cancelled)
    ));
    assert!(!archive.exists() && !dir.path().join("never.lumilio-backup.part").exists());

    let other = dir.path().join("other.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&other).unwrap());
    writer
        .start_file("readme.txt", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(b"hi").unwrap();
    writer.finish().unwrap();
    assert!(matches!(read_meta(&other), Err(BackupError::NotABackup)));
    let text = dir.path().join("text.zip");
    fs::write(&text, b"not a zip").unwrap();
    assert!(matches!(read_meta(&text), Err(BackupError::NotABackup)));
}

#[test]
fn restoring_skips_entries_that_would_escape_and_a_newer_format_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("evil.zip");
    let mut writer = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    writer.start_file(META, options).unwrap();
    writer
        .write_all(serde_json::to_string(&meta()).unwrap().as_bytes())
        .unwrap();
    for name in [
        "game/ok.txt",
        "game/../../escape.txt",
        "/abs.txt",
        "other/x.txt",
    ] {
        writer.start_file(name, options).unwrap();
        writer.write_all(b"x").unwrap();
    }
    writer.finish().unwrap();
    let into = dir.path().join("into");
    assert_eq!(
        restore(&archive, &into, &CancellationToken::new()).unwrap(),
        1
    );
    assert!(into.join("ok.txt").is_file());
    assert!(!dir.path().join("escape.txt").exists() && !into.join("other").exists());

    let future = dir.path().join("future.zip");
    let mut newer = meta();
    newer.format = FORMAT + 1;
    let mut writer = zip::ZipWriter::new(fs::File::create(&future).unwrap());
    writer.start_file(META, options).unwrap();
    writer
        .write_all(serde_json::to_string(&newer).unwrap().as_bytes())
        .unwrap();
    writer.finish().unwrap();
    assert!(matches!(read_meta(&future), Err(BackupError::NotABackup)));
}
