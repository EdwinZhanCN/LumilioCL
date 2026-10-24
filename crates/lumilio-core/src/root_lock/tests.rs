use super::*;

#[test]
fn dropping_ownership_unlocks_even_if_an_open_file_description_is_retained() {
    let dir = tempfile::tempdir().unwrap();
    let owner = RootLock::acquire(dir.path()).unwrap();
    let retained = owner._file.try_clone().unwrap();
    drop(owner);
    let replacement = RootLock::acquire(dir.path()).unwrap();
    drop(retained);
    assert!(matches!(
        RootLock::acquire(dir.path()),
        Err(ServiceError::RootBusy(_))
    ));
    drop(replacement);
    RootLock::acquire(dir.path()).unwrap();
}
