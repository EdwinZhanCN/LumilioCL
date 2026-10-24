use super::*;
use crate::instances::{InstanceSettings, Loader, NewInstance};

fn request(name: &str) -> NewInstance {
    NewInstance {
        name: name.to_owned(),
        game_version: "1.21.1".to_owned(),
        loader: Loader::Vanilla,
        loader_version: None,
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    layout: Layout,
    store: InstanceStore,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let layout = Layout::new(dir.path());
    let store = InstanceStore::open(dir.path()).unwrap();
    Setup {
        _dir: dir,
        layout,
        store,
    }
}

fn stage(setup: &Setup, id: &str, ready: bool) -> Staged {
    let staged = Staged::begin(&setup.layout, "copy", id).unwrap();
    fs::create_dir_all(staged.game_dir().join("saves/W")).unwrap();
    fs::write(staged.game_dir().join("saves/W/level.dat"), b"world").unwrap();
    if ready {
        staged.mark_ready().unwrap();
    }
    staged
}

#[test]
fn publishing_moves_the_whole_folder_and_leaves_no_operation_behind() {
    let mut s = setup();
    let staged = stage(&s, "fresh", true);
    s.store
        .create_as(
            "fresh",
            request("Fresh"),
            InstanceSettings::default(),
            false,
            1,
        )
        .unwrap();
    staged.publish(&s.layout).unwrap();
    assert_eq!(
        fs::read(s.layout.game("fresh").join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(!s.layout.operations().join("copy-fresh").exists());
}

#[test]
fn staging_refuses_unknown_kinds_odd_ids_and_an_unsettled_earlier_attempt() {
    let s = setup();
    assert!(Staged::begin(&s.layout, "weird", "a").is_err());
    assert!(Staged::begin(&s.layout, "copy", "../x").is_err());
    let first = Staged::begin(&s.layout, "copy", "a").unwrap();
    assert!(Staged::begin(&s.layout, "copy", "a").is_err());
    first.discard();
    assert!(!s.layout.operations().join("copy-a").exists());
}

#[test]
fn an_id_being_staged_is_passed_over_until_it_is_given_up() {
    let mut s = setup();
    let first = Staged::claim(&s.store, "import", "26.3").unwrap();
    let second = Staged::claim(&s.store, "copy", "红石生电优化26.3").unwrap();
    assert_eq!((first.id(), second.id()), ("26-3", "26-3-2"));
    // A game made by hand meanwhile does not take an id an import will publish as.
    let made = s.store.create(request("26 3"), 1).unwrap().id.clone();
    assert_eq!(made, "26-3-3");
    first.discard();
    assert_eq!(s.store.suggest_id("26.3").unwrap(), "26-3");
    assert!(matches!(
        Staged::claim(&s.store, "import", " "),
        Err(StoreError::InvalidName)
    ));
}

#[test]
fn recovery_settles_every_state_a_crash_can_leave() {
    let mut s = setup();
    // Never marked ready: nothing was published, silently removed.
    stage(&s, "raw", false);
    // Ready but the record was never created: discarded and reported.
    stage(&s, "norecord", true);
    // Ready and the record exists but the rename never happened: finished.
    stage(&s, "unfinished", true);
    s.store
        .create_as(
            "unfinished",
            request("Unfinished"),
            InstanceSettings::default(),
            false,
            1,
        )
        .unwrap();
    // Ready, record and folder both exist: only garbage remains.
    stage(&s, "done", true);
    s.store
        .create_as(
            "done",
            request("Done"),
            InstanceSettings::default(),
            false,
            1,
        )
        .unwrap();
    fs::create_dir_all(s.layout.game("done")).unwrap();

    let notes = recover(&s.layout, &s.store);
    assert_eq!(
        notes,
        [
            RecoveryNote::PublishDiscarded {
                instance_id: "norecord".into()
            },
            RecoveryNote::PublishCompleted {
                instance_id: "unfinished".into()
            },
        ]
    );
    assert_eq!(
        fs::read(s.layout.game("unfinished").join("saves/W/level.dat")).unwrap(),
        b"world"
    );
    assert!(
        fs::read_dir(s.layout.operations())
            .unwrap()
            .next()
            .is_none()
    );
    assert!(recover(&s.layout, &s.store).is_empty());
}
