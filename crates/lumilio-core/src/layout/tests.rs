use super::*;

#[test]
fn shared_files_live_in_meta_and_instance_files_in_profiles() {
    let layout = Layout::new("/r");
    assert_eq!(layout.versions(), Path::new("/r/meta/versions"));
    assert_eq!(layout.libraries(), Path::new("/r/meta/libraries"));
    assert_eq!(layout.assets(), Path::new("/r/meta/assets"));
    assert_eq!(layout.natives("1.21"), Path::new("/r/meta/natives/1.21"));
    assert_eq!(layout.game("a"), Path::new("/r/profiles/a/game"));
    assert_eq!(
        layout.history("a"),
        Path::new("/r/profiles/a/history.jsonl")
    );
    assert_eq!(layout.snapshots("a"), Path::new("/r/profiles/a/snapshots"));
    assert_eq!(layout.database(), Path::new("/r/launcher.db"));
}

#[test]
fn two_instances_never_share_a_profile() {
    let layout = Layout::new("/r");
    assert_ne!(layout.game("a"), layout.game("b"));
    assert_ne!(layout.profile("a"), layout.profile("b"));
}
