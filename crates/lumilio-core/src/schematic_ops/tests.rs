use super::*;
use crate::plugins::write::CONFLICT;

const FILE: &str = "schematics/house.litematic";

struct Fixture {
    _dir: tempfile::TempDir,
    folder: PathBuf,
    target: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("profile/schematic-ops");
    let target = dir.path().join("profile/game").join(FILE);
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"original").unwrap();
    Fixture {
        _dir: dir,
        folder,
        target,
    }
}

fn replace() -> SchematicOperation {
    SchematicOperation {
        kind: "replace".into(),
        summary: "minecraft:stone -> minecraft:andesite".into(),
    }
}

#[test]
fn the_original_is_backed_up_before_the_file_is_written() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    // The new content cannot be written: its temporary file's name is taken.
    let blocked = fx.target.with_file_name("house.litematic.lumilio-tmp");
    fs::create_dir(&blocked).unwrap();
    assert!(
        apply(
            &fx.folder,
            &fx.target,
            FILE,
            b"edited",
            Some(&info),
            &replace(),
            1
        )
        .is_err()
    );
    let copies: Vec<_> = fs::read_dir(
        fx.folder
            .join(BACKUP_DIR)
            .join("schematics__house.litematic"),
    )
    .unwrap()
    .flatten()
    .map(|entry| fs::read(entry.path()).unwrap())
    .collect();
    assert_eq!(copies, vec![b"original".to_vec()], "the copy came first");
    assert_eq!(fs::read(&fx.target).unwrap(), b"original");
    assert!(
        history(&fx.folder).unwrap().is_empty(),
        "a failed write is not logged"
    );

    fs::remove_dir(&blocked).unwrap();
    let entry = apply(
        &fx.folder,
        &fx.target,
        FILE,
        b"edited",
        Some(&info),
        &replace(),
        2,
    )
    .unwrap();
    assert_eq!(fs::read(&fx.target).unwrap(), b"edited");
    let backup = fx.folder.join(entry.backup.as_deref().unwrap());
    assert_eq!(fs::read(backup).unwrap(), b"original");
}

#[test]
fn a_restore_puts_back_the_same_bytes_and_is_logged_and_undoable() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    let first = apply(
        &fx.folder,
        &fx.target,
        FILE,
        b"edited",
        Some(&info),
        &replace(),
        10,
    )
    .unwrap();
    assert_eq!(first.index, 1);

    let restored = restore(&fx.folder, &fx.target, &first, 20).unwrap();
    assert_eq!(fs::read(&fx.target).unwrap(), b"original");
    assert_eq!(restored.index, 2);
    assert_eq!(restored.operation, RESTORE);
    assert_eq!(restored.restores, Some(1));

    // The edited file was kept too, so the restore can be undone.
    let undone = restore(&fx.folder, &fx.target, &restored, 30).unwrap();
    assert_eq!(fs::read(&fx.target).unwrap(), b"edited");
    let listed: Vec<_> = history(&fx.folder)
        .unwrap()
        .iter()
        .map(|entry| (entry.index, entry.operation.clone()))
        .collect();
    assert_eq!(
        listed,
        vec![
            (3, RESTORE.into()),
            (2, RESTORE.into()),
            (1, "replace".into())
        ]
    );
    assert_eq!(undone.restores, Some(2));
}

#[test]
fn a_file_changed_since_it_was_read_is_not_written_backed_up_or_logged() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    fs::write(&fx.target, b"changed in the game").unwrap();
    assert_eq!(
        apply(
            &fx.folder,
            &fx.target,
            FILE,
            b"edited",
            Some(&info),
            &replace(),
            1
        ),
        Err(PluginError::Unavailable(CONFLICT.into()))
    );
    assert_eq!(fs::read(&fx.target).unwrap(), b"changed in the game");
    assert!(!fx.folder.join(BACKUP_DIR).exists());
    assert!(history(&fx.folder).unwrap().is_empty());
}

#[test]
fn each_operation_is_one_json_line() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    let entry = apply(
        &fx.folder,
        &fx.target,
        FILE,
        b"edited",
        Some(&info),
        &replace(),
        1_760_000_000,
    )
    .unwrap();
    let log = fs::read_to_string(fx.folder.join(LOG_FILE)).unwrap();
    let backup = entry.backup.unwrap();
    assert!(backup.starts_with("backups/schematics__house.litematic/"));
    assert_eq!(
        log,
        format!(
            "{{\"time\":1760000000,\"operation\":\"replace\",\"file\":\"{FILE}\",\
             \"backup\":\"{backup}\",\"summary\":\"minecraft:stone -> minecraft:andesite\"}}\n"
        )
    );
}

#[test]
fn a_line_cut_short_is_skipped_but_keeps_its_number() {
    let fx = fixture();
    fs::create_dir_all(&fx.folder).unwrap();
    fs::write(fx.folder.join(LOG_FILE), "{\"time\":1,\"opera").unwrap();
    let (_, info) = describe(&fx.target).unwrap();
    let entry = apply(
        &fx.folder,
        &fx.target,
        FILE,
        b"edited",
        Some(&info),
        &replace(),
        2,
    )
    .unwrap();
    assert_eq!(entry.index, 2);
    let listed = history(&fx.folder).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], entry);
}

#[test]
fn a_logged_backup_outside_the_backup_folder_is_refused() {
    let fx = fixture();
    let mut entry = SchematicLogEntry {
        index: 1,
        time: 1,
        operation: "replace".into(),
        file: FILE.into(),
        backup: Some("backups/../../game/schematics/house.litematic".into()),
        summary: String::new(),
        restores: None,
    };
    assert!(restore(&fx.folder, &fx.target, &entry, 2).is_err());
    entry.backup = Some("log.jsonl".into());
    assert!(restore(&fx.folder, &fx.target, &entry, 2).is_err());
    assert_eq!(
        fs::read(&fx.target).unwrap(),
        b"original",
        "nothing was touched"
    );
}

#[test]
fn removing_moves_the_file_into_the_backups_and_a_restore_brings_it_back() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    let dedupe = SchematicOperation {
        kind: "dedupe".into(),
        summary: "keep other".into(),
    };
    let entry = remove(&fx.folder, &fx.target, FILE, &info, &dedupe, 1).unwrap();
    assert!(!fx.target.exists(), "the file left the game directory");
    let backup = fx.folder.join(entry.backup.as_deref().unwrap());
    assert_eq!(
        fs::read(&backup).unwrap(),
        b"original",
        "nothing was deleted"
    );
    assert_eq!(history(&fx.folder).unwrap(), vec![entry.clone()]);
    assert_eq!(entry.operation, "dedupe");

    restore(&fx.folder, &fx.target, &entry, 2).unwrap();
    assert_eq!(fs::read(&fx.target).unwrap(), b"original");
}

#[test]
fn a_file_changed_since_it_was_read_is_not_removed() {
    let fx = fixture();
    let (_, info) = describe(&fx.target).unwrap();
    fs::write(&fx.target, b"changed by the game").unwrap();
    let error = remove(&fx.folder, &fx.target, FILE, &info, &replace(), 1).unwrap_err();
    assert_eq!(error, PluginError::Unavailable(CONFLICT.into()));
    assert_eq!(fs::read(&fx.target).unwrap(), b"changed by the game");
    assert!(!fx.folder.join(BACKUP_DIR).exists());
    assert!(history(&fx.folder).unwrap().is_empty());
}

/// T23: a conversion saved beside its original writes a new file, refuses
/// to replace one already there, and leaves the original alone; restoring
/// its line takes the new file into the backups, as a deletion does.
#[test]
fn a_new_file_is_written_beside_and_restoring_takes_it_out() {
    let fx = fixture();
    let beside = fx.target.with_file_name("house (1.20.1).litematic");
    let name = "schematics/house (1.20.1).litematic";
    let convert = SchematicOperation {
        kind: "convert".into(),
        summary: "26.3 -> 1.20.1".into(),
    };
    let entry = apply(&fx.folder, &beside, name, b"converted", None, &convert, 1).unwrap();
    assert_eq!(fs::read(&beside).unwrap(), b"converted");
    assert_eq!(
        fs::read(&fx.target).unwrap(),
        b"original",
        "the original stays"
    );
    assert_eq!(entry.backup, None, "nothing was there to keep");
    let refused = apply(&fx.folder, &beside, name, b"again", None, &convert, 2);
    assert!(refused.is_err(), "a file is there now");
    assert_eq!(fs::read(&beside).unwrap(), b"converted");

    let restored = restore(&fx.folder, &beside, &entry, 3).unwrap();
    assert!(!beside.exists(), "restored to no file");
    let kept = fx.folder.join(restored.backup.as_deref().unwrap());
    assert_eq!(fs::read(kept).unwrap(), b"converted", "kept, not deleted");
    assert_eq!(restored.restores, Some(entry.index));
}

/// T24: tables are kept by name and read back; recent targets come latest
/// first, once each, and only so many; no file reads as none.
#[test]
fn replace_schemes_are_kept_and_recent_targets_come_first() {
    use super::schemes::{RECENT, ReplaceSchemes, SchemeRow, read, write};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schematic-replace.json");
    assert_eq!(read(&path), ReplaceSchemes::default());
    let mut schemes = ReplaceSchemes::default();
    schemes.schemes.insert(
        "spruce".into(),
        vec![SchemeRow {
            source: "minecraft:oak_stairs".into(),
            properties: vec![("half".into(), "top".into())],
            target: "minecraft:spruce_stairs".into(),
        }],
    );
    schemes.used([
        "minecraft:spruce_stairs".to_owned(),
        "minecraft:glass".to_owned(),
    ]);
    schemes.used(["minecraft:glass".to_owned()]);
    assert_eq!(
        schemes.recent,
        ["minecraft:glass", "minecraft:spruce_stairs"]
    );
    schemes.used((0..20).map(|n| format!("minecraft:block_{n}")));
    assert_eq!(schemes.recent.len(), RECENT);
    assert_eq!(schemes.recent[0], "minecraft:block_0");
    write(&path, &schemes).unwrap();
    assert_eq!(read(&path), schemes);
}
